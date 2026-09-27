mod summary;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{CellAlignment, Table};
use rent_or_sell_engine::export::{self, Cell};
use rent_or_sell_engine::headline::Headline;
use rent_or_sell_engine::{
    AltModel, BreakEven, Evaluation, KeepResult, Nok, OptionKind, SaleModel, Scenario, TaxRuleSet,
    TaxRules, WealthModel, YearMonth, YearRow,
};

#[derive(Parser)]
#[command(
    name = "rent-or-sell",
    version,
    about = "Rent out vs. sell evaluator (Norway)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Simulate renting out the property and compare selling at different times.
    Run {
        /// Scenario TOML file (see crates/engine/scenarios/basic.toml).
        scenario: PathBuf,
        /// Also write the table shown (yearly, or monthly with --monthly) to this CSV
        /// file, and the sale options to <name>-options.csv next to it.
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Show all tables, notes, tax rules and assumptions instead of the summary.
        #[arg(long)]
        full: bool,
        /// Show month-by-month detail (implies --full).
        #[arg(long)]
        monthly: bool,
        #[command(flatten)]
        rules: RulesArg,
    },
    /// Re-run with one input bumped at a time (rates, price growth, rent,
    /// vacancy, costs) and show how keep vs sell moves.
    Sensitivity {
        scenario: PathBuf,
        /// Also write the table to this CSV file.
        #[arg(long)]
        csv: Option<PathBuf>,
        #[command(flatten)]
        rules: RulesArg,
    },
    /// Show step by step how the wealth of one sale was derived.
    Explain {
        scenario: PathBuf,
        /// Sale month (YYYY-MM) within the horizon.
        #[arg(long, conflicts_with = "option")]
        at: Option<String>,
        /// A standard option instead of a month (default: horizon).
        #[arg(long, value_enum)]
        option: Option<OptionArg>,
        /// Explain plain keep's wealth tax at the end of this year instead.
        #[arg(long, conflicts_with_all = ["at", "option", "compare"])]
        wealth: Option<i32>,
        /// Explain the keep-vs-sell comparison row (investment account included)
        /// instead of the sale itself. Implied by --option rent / prepay.
        #[arg(long)]
        compare: bool,
        #[command(flatten)]
        rules: RulesArg,
    },
}

#[derive(clap::Args)]
struct RulesArg {
    /// Tax rule set TOML to use instead of the built-in ones (repeatable, one per income year).
    #[arg(long = "rules")]
    paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
enum OptionArg {
    /// Sell now.
    Now,
    /// Sell in the last tax-free month.
    TaxFree,
    /// Sell at the horizon (= keep to horizon).
    Horizon,
    /// Rent, with savings invested (comparison row).
    #[value(alias = "keep")]
    Rent,
    /// Rent, prepaying the loan with savings (comparison row).
    Prepay,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Run {
            scenario,
            csv,
            full,
            monthly,
            rules,
        } => run(
            &scenario,
            csv.as_deref(),
            full || monthly,
            monthly,
            &rules.paths,
        ),
        Command::Sensitivity {
            scenario,
            csv,
            rules,
        } => sensitivity(&scenario, csv.as_deref(), &rules.paths),
        Command::Explain {
            scenario,
            at,
            option,
            wealth,
            compare,
            rules,
        } => match wealth {
            Some(year) => explain_wealth(&scenario, year, &rules.paths),
            None => explain(&scenario, at.as_deref(), option, compare, &rules.paths),
        },
    }
}

fn load(scenario_path: &Path, rule_paths: &[PathBuf]) -> Result<(Scenario, TaxRules)> {
    let text = std::fs::read_to_string(scenario_path)
        .with_context(|| format!("reading {}", scenario_path.display()))?;
    let scenario = Scenario::from_toml_str(&text)
        .with_context(|| format!("in scenario {}", scenario_path.display()))?;
    Ok((scenario, load_rules(rule_paths)?))
}

fn run(
    scenario_path: &Path,
    csv: Option<&Path>,
    full: bool,
    monthly: bool,
    rule_paths: &[PathBuf],
) -> Result<()> {
    let (scenario, rules) = load(scenario_path, rule_paths)?;
    let evaluation = rent_or_sell_engine::evaluate(&scenario, &rules)?;
    let result = &evaluation.keep;
    let headline = rent_or_sell_engine::headline(&scenario, &evaluation);
    if !full {
        print!("{}", summary::render(&headline, summary::Style::detect()));
        return write_run_csv(csv, monthly, &evaluation);
    }

    let end = scenario
        .start
        .add_months(scenario.horizon_months() as i64 - 1);
    println!(
        "{} — rent out, {} to {}\n",
        result.scenario_name, scenario.start, end
    );
    if monthly {
        println!("{}", monthly_view(result, Show::of(&scenario)));
    } else {
        println!("{}", yearly_view(&evaluation, Show::of(&scenario)));
        println!(
            "Out-of-pocket/mo: average monthly top-up before tax. After-tax/mo: the year's after-tax cash flow spread per month.\nWealth if sold: equity after selling at the end of that year + cumulative cash."
        );
    }

    println!("\nSale options (wealth = equity after sale + cumulative cash from renting):");
    println!("{}", options_view(&evaluation, Show::of(&scenario)));

    let horizon = scenario.start.add_months(scenario.horizon_months() as i64);
    println!("\nKeep vs sell at {horizon} (the same out-of-pocket cash flows in every option):");
    println!(
        "{}",
        comparison_view(&evaluation, Show::of(&scenario).wealth)
    );
    println!(
        "Break-even: after-tax annual return the sale proceeds must earn to match renting (savings excluded);\nfor \"prepay\", the return the savings must earn to beat prepaying the loan."
    );

    if !headline.by_sale.is_empty() {
        println!(
            "\nRent, then sell: wealth at {horizon} by sale month (savings excluded; the investment at its configured return):"
        );
        println!("{}", by_sale_view(&headline));
        let basis = if headline.returns_before_tax {
            "before tax, taxed like the investment"
        } else {
            "after tax"
        };
        println!("\nWealth at {horizon} at other yearly returns ({basis}; savings excluded):");
        println!("{}", by_return_view(&headline));
    }

    println!("\nTax rules used:");
    for set in rules.sets() {
        println!("  {} — {}", set.income_year, describe_rules(set));
    }
    if !evaluation.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &evaluation.warnings {
            println!("  ⚠ {w}");
        }
    }
    println!("\nNotes:");
    for n in &evaluation.notes {
        println!("  • {n}");
    }
    println!("\nAssumptions:");
    for g in &headline.assumptions {
        println!("  {}", g.title);
        for a in &g.items {
            println!("    • {a}");
        }
    }
    println!("\nSee how a number was derived: rent-or-sell explain <scenario> --at YYYY-MM");
    write_run_csv(csv, monthly, &evaluation)
}

/// Writes the yearly (or monthly), options and comparison tables.
fn write_run_csv(csv: Option<&Path>, monthly: bool, evaluation: &Evaluation) -> Result<()> {
    let Some(path) = csv else {
        return Ok(());
    };
    let table = if monthly {
        export::monthly_table(&evaluation.keep)
    } else {
        export::yearly_table(evaluation)
    };
    write_csv(path, &table)?;
    let options_path = sibling(path, "options");
    write_csv(&options_path, &export::options_table(evaluation))?;
    let comparison_path = sibling(path, "comparison");
    write_csv(&comparison_path, &export::comparison_table(evaluation))?;
    println!(
        "\nWrote {}, {} and {}",
        path.display(),
        options_path.display(),
        comparison_path.display()
    );
    Ok(())
}

fn explain(
    scenario_path: &Path,
    at: Option<&str>,
    option: Option<OptionArg>,
    compare: bool,
    rule_paths: &[PathBuf],
) -> Result<()> {
    let (scenario, rules) = load(scenario_path, rule_paths)?;
    let evaluation = rent_or_sell_engine::evaluate(&scenario, &rules)?;
    let model = SaleModel::new(&scenario, &rules, &evaluation.keep);
    let n = scenario.horizon_months();

    let offset = match (at, option) {
        (Some(at), _) => {
            let month: YearMonth = at.parse()?;
            let Some(offset) = model.offset_of(month) else {
                bail!(
                    "{month} is outside the horizon ({} to {})",
                    scenario.start,
                    scenario.start.add_months(n as i64)
                );
            };
            Some(offset)
        }
        (None, Some(OptionArg::Rent | OptionArg::Prepay)) => None,
        (None, option) => {
            let kind = match option.unwrap_or(OptionArg::Horizon) {
                OptionArg::Now => OptionKind::SellNow,
                OptionArg::TaxFree => OptionKind::SellLastTaxFree,
                _ => OptionKind::KeepToHorizon,
            };
            match evaluation.options.iter().find(|o| o.kind == kind) {
                Some(o) => Some(o.outcome.offset),
                None => bail!("no tax-free sale is possible within the horizon"),
            }
        }
    };
    println!("{} — {}\n", scenario.name, scenario.start);

    let row_mode = compare || matches!(option, Some(OptionArg::Rent | OptionArg::Prepay));
    if !row_mode {
        print!(
            "{}",
            model.explain(&model.outcome(offset.unwrap())).render()
        );
        return Ok(());
    }
    let alt = AltModel::new(&scenario, &rules, &evaluation.keep, &model)?;
    let row = match (option, offset) {
        (Some(OptionArg::Prepay), _) => match alt.keep_prepay_row(n) {
            Some(row) => row,
            None => bail!("rent-and-prepay needs [savings], a [loan] and [alternative.invest]"),
        },
        (_, Some(k)) if k < n => {
            let label = format!("Sell {} → invest", scenario.start.add_months(k as i64));
            alt.sell_row(label, k, n)
        }
        _ => alt.keep_row(n),
    };
    print!("{}", alt.explain(&row).render());
    Ok(())
}

fn sensitivity(scenario_path: &Path, csv: Option<&Path>, rule_paths: &[PathBuf]) -> Result<()> {
    let (scenario, rules) = load(scenario_path, rule_paths)?;
    let rows = rent_or_sell_engine::sensitivity(&scenario, &rules)?;
    let horizon = scenario.start.add_months(scenario.horizon_months() as i64);
    println!(
        "{} — sensitivity at {horizon}, one change at a time\n",
        scenario.name
    );
    let mut t = new_table(&[
        "Change",
        "Rent: wealth",
        "vs base",
        "Sell now vs rent",
        "Break-even (sell now)",
    ]);
    let base = rows[0].keep_net_position;
    let opt = |n: Option<Nok>| n.map_or("—".to_string(), |n| n.to_string());
    for r in &rows {
        t.add_row(vec![
            r.label.clone(),
            r.keep_net_position.to_string(),
            (r.keep_net_position - base).to_string(),
            opt(r.sell_now_vs_keep),
            r.sell_now_break_even.map_or(String::new(), fmt_break_even),
        ]);
    }
    right_align(&mut t, 1);
    println!("{t}");
    if let Some(base) = rows[0].sell_now_vs_keep {
        let st = summary::Style::detect();
        let sells = |d: Nok| d.0 > 0.0;
        let flips: Vec<&str> = rows[1..]
            .iter()
            .filter(|r| r.sell_now_vs_keep.is_some_and(|d| sells(d) != sells(base)))
            .map(|r| r.label.as_str())
            .collect();
        let (now, other) = if sells(base) {
            ("selling now", "renting")
        } else {
            ("renting", "selling now")
        };
        let line = if flips.is_empty() {
            format!("Base case: {now} wins, and no single change flips it.")
        } else {
            format!(
                "Base case: {now} wins; {other} wins instead with: {}.",
                flips.join(", ")
            )
        };
        println!("{}", st.bold(&line));
    }
    println!(
        "Sell now vs rent: positive means selling now and investing beats renting.\nRates shift the loan (and fellesgjeld); costs scale all running costs; vacancy adds a month a year."
    );
    if let Some(path) = csv {
        write_csv(path, &export::sensitivity_table(&rows))?;
        println!("\nWrote {}", path.display());
    }
    Ok(())
}

fn explain_wealth(scenario_path: &Path, year: i32, rule_paths: &[PathBuf]) -> Result<()> {
    let (scenario, rules) = load(scenario_path, rule_paths)?;
    let Some(model) = WealthModel::new(&scenario, &rules) else {
        bail!("the scenario has no [wealth] section, so wealth tax is not modelled");
    };
    let keep = rent_or_sell_engine::run_keep(&scenario, &rules)?;
    let Some(m) = keep
        .months
        .iter()
        .position(|r| r.month.year() == year && r.month.month() == 12)
    else {
        bail!("no December {year} within the horizon");
    };
    let a = model.assess(m, &model.property(m, keep.months[m].loan_balance));
    println!("{} — rent out\n", scenario.name);
    print!("{}", model.explain(&a).render());
    Ok(())
}

/// `dir/name.csv` → `dir/name-<suffix>.csv`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    path.with_file_name(format!("{stem}-{suffix}.csv"))
}

fn load_rules(paths: &[PathBuf]) -> Result<TaxRules> {
    if paths.is_empty() {
        return Ok(TaxRules::builtin());
    }
    let sets = paths
        .iter()
        .map(|p| {
            let text =
                std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
            TaxRuleSet::from_toml_str(&text)
                .with_context(|| format!("in tax rules {}", p.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(TaxRules::from_sets(sets)?)
}

fn describe_rules(set: &TaxRuleSet) -> String {
    format!(
        "rate {} (verified {}), tax-free rent up to {} kr/yr, loan fees deductible: {}, \
         tax-free sale: owned > {} months and lived {} of the last {} months; \
         wealth tax {} above {} per person, secondary home at {}, shares at {}",
        set.capital_income_rate.value,
        set.capital_income_rate.verified_on,
        set.rental_tax_free_threshold.value,
        set.loan_fees_deductible.value,
        set.sale_min_ownership_months.value,
        set.sale_occupancy_months.value,
        set.sale_occupancy_window_months.value,
        set.wealth_rate.value,
        set.wealth_threshold.value,
        set.secondary_home_rate.value,
        set.share_valuation_rate.value,
    )
}

fn new_table(headers: &[&str]) -> Table {
    let mut t = Table::new();
    t.load_style(UTF8_FULL_CONDENSED)
        .set_header(headers.to_vec());
    t
}

fn right_align(t: &mut Table, from_column: usize) {
    let n = t.column_count();
    for i in from_column..n {
        if let Some(c) = t.column_mut(i) {
            c.set_cell_alignment(CellAlignment::Right);
        }
    }
}

/// Which optional columns the views show.
#[derive(Clone, Copy)]
struct Show {
    wealth: bool,
    fellesgjeld: bool,
}

impl Show {
    fn of(scenario: &Scenario) -> Show {
        Show {
            wealth: scenario.wealth.is_some(),
            fellesgjeld: scenario.fellesgjeld.is_some(),
        }
    }
}

type YearColumn = (&'static str, fn(&YearRow) -> String);

fn yearly_view(evaluation: &Evaluation, show: Show) -> Table {
    let years = &evaluation.keep.years;
    let cols: Vec<Option<YearColumn>> = vec![
        Some(("Rent collected", |y| y.rent_collected.to_string())),
        Some(("Op. costs", |y| y.operating_costs.to_string())),
        Some(("Interest+fees", |y| (y.interest + y.loan_fees).to_string())),
        Some(("Principal", |y| y.principal.to_string())),
        show.fellesgjeld.then_some(("Fellesgjeld (int+princ)", |y| {
            (y.fellesgjeld_interest + y.fellesgjeld_principal).to_string()
        })),
        Some(("Tax", |y| y.tax.to_string())),
        show.wealth
            .then_some(("Wealth tax", |y| y.wealth_tax.to_string())),
        Some(("After-tax CF", |y| y.after_tax_cash_flow.to_string())),
        Some(("Out-of-pocket/mo", |y| {
            y.avg_monthly_out_of_pocket.to_string()
        })),
        Some(("After-tax/mo", |y| y.after_tax_monthly.to_string())),
        Some(("Loan balance", |y| y.loan_balance_end.to_string())),
        show.fellesgjeld.then_some(("Fellesgjeld balance", |y| {
            y.fellesgjeld_balance_end.to_string()
        })),
    ];
    let cols: Vec<YearColumn> = cols.into_iter().flatten().collect();
    let mut headers = vec!["Year", "Mo"];
    headers.extend(cols.iter().map(|c| c.0));
    headers.extend(["Market value", "Wealth if sold", "Break-even (sell now)"]);
    let mut t = new_table(&headers);
    let row = |label: String, months: String, y: &YearRow| -> Vec<String> {
        let mut r = vec![label, months];
        r.extend(cols.iter().map(|c| (c.1)(y)));
        r
    };
    for ((y, sale), be) in years
        .iter()
        .zip(&evaluation.year_end)
        .zip(&evaluation.year_end_break_even)
    {
        let mut r = row(y.year.to_string(), y.months.to_string(), y);
        r.push(sale.price.to_string());
        r.push(sale.net_position.to_string());
        r.push(fmt_break_even(*be));
        t.add_row(r);
    }
    let total = totals(years);
    let mut total_row = row("Total".into(), total.months.to_string(), &total);
    for (cell, col) in total_row[2..].iter_mut().zip(&cols) {
        if col.0.ends_with("/mo") {
            cell.clear();
        }
    }
    t.add_row(total_row);
    right_align(&mut t, 1);
    t
}

fn options_view(evaluation: &Evaluation, show: Show) -> Table {
    let mut headers = vec![
        "Option",
        "Sale month",
        "Price",
        "Sale costs",
        "Debt repaid",
        "Gain",
        "Tax-free",
        "Gain tax",
        "Equity",
        "Cum. cash",
        "Wealth",
    ];
    if show.fellesgjeld {
        headers.insert(5, "Fellesgjeld");
    }
    let mut t = new_table(&headers);
    for o in &evaluation.options {
        let s = &o.outcome;
        let mut row = vec![
            o.label.clone(),
            s.month.to_string(),
            s.price.to_string(),
            s.sale_costs.to_string(),
            s.debt_repaid.to_string(),
            s.gain.to_string(),
            if s.tax_free.tax_free { "yes" } else { "no" }.to_string(),
            s.gain_tax.to_string(),
            s.equity.to_string(),
            s.cumulative_cash.to_string(),
            s.net_position.to_string(),
        ];
        if show.fellesgjeld {
            row.insert(5, s.fellesgjeld.to_string());
        }
        t.add_row(row);
    }
    right_align(&mut t, 2);
    t
}

fn fmt_break_even(b: BreakEven) -> String {
    match b {
        BreakEven::Rate(r) => format!("{:.2}%", r * 100.0),
        BreakEven::Below(r) => format!("< {:.0}%", r * 100.0),
        BreakEven::Above(r) => format!("> {:.0}%", r * 100.0),
    }
}

fn comparison_view(evaluation: &Evaluation, wealth: bool) -> Table {
    let mut headers = vec![
        "Option",
        "Sold",
        "Property equity / proceeds",
        "Investment",
        "Cum. cash",
        "Wealth",
        "vs rent",
        "Break-even",
    ];
    if wealth {
        headers.insert(5, "Wealth tax paid");
    }
    let mut t = new_table(&headers);
    let opt = |n: Option<Nok>| n.map_or("—".to_string(), |n| n.to_string());
    for r in &evaluation.comparison {
        let mut row = vec![
            r.label.clone(),
            r.sale_month.map_or(String::new(), |m| m.to_string()),
            r.property_equity.to_string(),
            opt(r.account.as_ref().map(|a| a.value)),
            r.cumulative_cash.to_string(),
            opt(r.net_position),
            opt(r.vs_keep),
            r.break_even.map_or(String::new(), fmt_break_even),
        ];
        if wealth {
            row.insert(5, opt(r.wealth_tax));
        }
        t.add_row(row);
    }
    right_align(&mut t, 2);
    t
}

/// Sale now, each strategy's month, every January and the horizon.
fn by_sale_view(h: &Headline) -> Table {
    let mut t = new_table(&["Sale month", "Tax-free", "Wealth at horizon", "vs sell now"]);
    let named: Vec<&str> = h.strategies.iter().map(|s| s.sale_month.as_str()).collect();
    let last = h.by_sale.len() - 1;
    for (i, p) in h.by_sale.iter().enumerate() {
        // The first month a sale is taxed: where the value drops.
        let cliff = i > 0 && h.by_sale[i - 1].tax_free && !p.tax_free;
        let keep = i == 0 || i == last || cliff || p.month.ends_with("-01");
        if !(keep || named.contains(&p.month.as_str())) {
            continue;
        }
        let label = if i == last {
            format!("{} (rent to horizon)", p.month)
        } else if i == 0 {
            format!("{} (now)", p.month)
        } else {
            p.month.clone()
        };
        let tax_free = if i == last {
            String::new()
        } else if p.tax_free {
            "yes".into()
        } else {
            "no".into()
        };
        t.add_row(vec![
            label,
            tax_free,
            Nok(p.value).to_string(),
            Nok(p.vs_sell_now).to_string(),
        ]);
    }
    right_align(&mut t, 2);
    t
}

/// The strategies at every whole-percent return.
fn by_return_view(h: &Headline) -> Table {
    let mut headers = vec!["Return".to_string()];
    headers.extend(h.strategies.iter().map(summary::strategy_label));
    let refs: Vec<&str> = headers.iter().map(String::as_str).collect();
    let mut t = new_table(&refs);
    for p in h
        .by_return
        .iter()
        .filter(|p| ((p.rate * 100.0).round() - p.rate * 100.0).abs() < 1e-9)
    {
        let mut row = vec![format!("{:.0}%", p.rate * 100.0)];
        row.extend(p.values.iter().map(|v| Nok(*v).to_string()));
        t.add_row(row);
    }
    right_align(&mut t, 1);
    t
}

/// Sums of the flow columns; balance is the final one.
fn totals(years: &[YearRow]) -> YearRow {
    let sum = |f: fn(&YearRow) -> Nok| years.iter().map(f).sum::<Nok>();
    let mut total = years[0].clone();
    total.months = years.iter().map(|y| y.months).sum();
    total.rent_collected = sum(|y| y.rent_collected);
    total.operating_costs = sum(|y| y.operating_costs);
    total.interest = sum(|y| y.interest);
    total.loan_fees = sum(|y| y.loan_fees);
    total.principal = sum(|y| y.principal);
    total.fellesgjeld_interest = sum(|y| y.fellesgjeld_interest);
    total.fellesgjeld_principal = sum(|y| y.fellesgjeld_principal);
    total.pre_tax_cash_flow = sum(|y| y.pre_tax_cash_flow);
    total.tax = sum(|y| y.tax);
    total.wealth_tax = sum(|y| y.wealth_tax);
    total.after_tax_cash_flow = sum(|y| y.after_tax_cash_flow);
    total.loan_balance_end = years[years.len() - 1].loan_balance_end;
    total.fellesgjeld_balance_end = years[years.len() - 1].fellesgjeld_balance_end;
    total
}

fn monthly_view(result: &KeepResult, show: Show) -> Table {
    let mut headers = vec![
        "Month",
        "Rent collected",
        "Op. costs",
        "Interest",
        "Fee",
        "Principal",
        "Pre-tax CF",
        "Loan balance",
    ];
    if show.fellesgjeld {
        headers.splice(6..6, ["FG interest", "FG principal"]);
        headers.push("FG balance");
    }
    let mut t = new_table(&headers);
    for m in &result.months {
        let mut row = vec![
            m.month.to_string(),
            m.rent_collected.to_string(),
            m.operating_costs.to_string(),
            m.interest.to_string(),
            m.loan_fee.to_string(),
            m.principal.to_string(),
            m.pre_tax_cash_flow.to_string(),
            m.loan_balance.to_string(),
        ];
        if show.fellesgjeld {
            row.splice(
                6..6,
                [
                    m.fellesgjeld_interest.to_string(),
                    m.fellesgjeld_principal.to_string(),
                ],
            );
            row.push(m.fellesgjeld_balance.to_string());
        }
        t.add_row(row);
    }
    right_align(&mut t, 1);
    t
}

fn write_csv(path: &Path, table: &export::Table) -> Result<()> {
    let mut w =
        csv::Writer::from_path(path).with_context(|| format!("creating {}", path.display()))?;
    w.write_record(&table.headers)?;
    for row in &table.rows {
        w.write_record(row.iter().map(Cell::to_plain))?;
    }
    w.flush()?;
    Ok(())
}
