//! Rent-or-sell calculation engine.
//!
//! Pure library: no file or network I/O, so it backs both the CLI and the WASM
//! build. `evaluate` is the main entry point: renting's cash flow, sale
//! outcomes, the rent-vs-sell comparison and the strategies; `headline` turns
//! its result into the shared view model. See `docs/architecture.md` and
//! `docs/model.md`.

pub mod alternative;
pub mod assumptions;
pub mod error;
pub mod examples;
pub mod explain;
pub mod export;
pub mod headline;
pub mod keep;
pub mod ledger;
pub mod loan;
pub mod money;
pub mod sale;
pub mod scenario;
pub mod sensitivity;
pub mod strategies;
pub mod tax_rules;
pub mod time;
pub mod timepath;
pub mod wealth;

use serde::Serialize;

pub use alternative::{AltModel, BreakEven, ComparisonRow, RowKind};
pub use assumptions::{AssumptionGroup, assumptions};
pub use error::EngineError;
pub use explain::Explained;
pub use headline::{Headline, headline};
pub use keep::MonthRow;
pub use ledger::YearRow;
pub use money::{Nok, Rate};
pub use sale::{SaleModel, SaleOutcome, TaxFreeWindow};
pub use scenario::{CostLine, Scenario};
pub use sensitivity::{SensitivityRow, sensitivity};
pub use strategies::{Strategies, Strategy, StrategyKind};
pub use tax_rules::{TaxRuleSet, TaxRules};
pub use time::YearMonth;
pub use timepath::TimePath;
pub use wealth::WealthModel;

#[derive(Clone, Debug, Serialize)]
pub struct KeepResult {
    pub scenario_name: String,
    pub cost_lines: Vec<CostLine>,
    pub months: Vec<MonthRow>,
    pub years: Vec<YearRow>,
    /// Scenario-specific remarks (e.g. which tax rule years were applied).
    pub notes: Vec<String>,
}

/// Simulates keeping and renting out the property over the scenario horizon.
pub fn run_keep(scenario: &Scenario, rules: &TaxRules) -> Result<KeepResult, EngineError> {
    run_keep_with(scenario, rules, None)
}

/// Like [`run_keep`], with an optional lump-sum prepayment of the loan.
pub fn run_keep_with(
    scenario: &Scenario,
    rules: &TaxRules,
    prepay: Option<loan::Prepayment>,
) -> Result<KeepResult, EngineError> {
    scenario.validate()?;
    let (cost_lines, mut months) = keep::simulate_months_with(scenario, prepay)?;
    if let Some(w) = WealthModel::new(scenario, rules) {
        for (m, row) in months.iter_mut().enumerate() {
            if row.month.month() == 12 {
                let debt = row.loan_balance + row.fellesgjeld_balance;
                let a = w.assess(m, &w.property(m, debt));
                row.taxable_wealth = a.valuation.net;
                row.wealth_tax = a.tax;
            }
        }
    }
    let years = ledger::aggregate(&cost_lines, &months, rules);
    let notes = notes(&years);
    Ok(KeepResult {
        scenario_name: scenario.name.clone(),
        cost_lines,
        months,
        years,
        notes,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    SellNow,
    SellLastTaxFree,
    SellAt,
    KeepToHorizon,
}

#[derive(Clone, Debug, Serialize)]
pub struct SaleOption {
    pub kind: OptionKind,
    pub label: String,
    pub outcome: SaleOutcome,
}

#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub keep: KeepResult,
    /// Outcome of selling right after each year in `keep.years` (aligned by index).
    pub year_end: Vec<SaleOutcome>,
    pub options: Vec<SaleOption>,
    pub tax_free_window: TaxFreeWindow,
    /// Keep vs sell at the horizon: keep, keep-and-prepay, then each sale option → invest.
    pub comparison: Vec<ComparisonRow>,
    /// After-tax return that selling now must earn to match keeping until each year's end.
    pub year_end_break_even: Vec<BreakEven>,
    /// Rent until a month, then sell, all valued at the horizon; `None`
    /// without an investment.
    pub strategies: Option<Strategies>,
    /// Constant after-tax annual return equivalent to `[alternative.invest]`
    /// over the horizon (comparable with the break-even returns).
    pub invest_after_tax_return: Option<f64>,
    /// Remarks that may change how to read or trust the result.
    pub warnings: Vec<String>,
    /// Other scenario-specific remarks, including `keep.notes`.
    pub notes: Vec<String>,
}

/// Keeps and rents out over the horizon, and evaluates selling at the
/// standard options (now, last tax-free month, `sale.sell_at`, horizon) and
/// at the end of each year.
pub fn evaluate(scenario: &Scenario, rules: &TaxRules) -> Result<Evaluation, EngineError> {
    let keep = run_keep(scenario, rules)?;
    let (year_end, options, window, comparison, year_end_break_even, strategies) = {
        let model = SaleModel::new(scenario, rules, &keep);
        let mut elapsed = 0;
        let year_end = keep
            .years
            .iter()
            .map(|y| {
                elapsed += y.months as usize;
                model.outcome(elapsed)
            })
            .collect();

        let window = sale::tax_free_window(scenario, rules);
        let mut options = vec![SaleOption {
            kind: OptionKind::SellNow,
            label: "Sell now".into(),
            outcome: model.outcome(0),
        }];
        if let TaxFreeWindow::Open(m) = window
            && let Some(k) = model.offset_of(m)
            && k > 0
        {
            options.push(SaleOption {
                kind: OptionKind::SellLastTaxFree,
                label: "Sell last tax-free month".into(),
                outcome: model.outcome(k),
            });
        }
        for &m in &scenario.sale.sell_at {
            if let Some(k) = model.offset_of(m) {
                options.push(SaleOption {
                    kind: OptionKind::SellAt,
                    label: format!("Sell {m}"),
                    outcome: model.outcome(k),
                });
            }
        }
        options.push(SaleOption {
            kind: OptionKind::KeepToHorizon,
            label: "Rent to horizon".into(),
            outcome: model.outcome(scenario.horizon_months()),
        });

        let n = scenario.horizon_months();
        let alt = AltModel::new(scenario, rules, &keep, &model)?;
        let mut comparison = vec![alt.keep_row(n)];
        comparison.extend(alt.keep_prepay_row(n));
        for o in options.iter().filter(|o| o.outcome.offset < n) {
            comparison.push(alt.sell_row(format!("{} → invest", o.label), o.outcome.offset, n));
        }
        let mut elapsed = 0;
        let mut year_end_break_even = Vec::new();
        for y in &keep.years {
            elapsed += y.months as usize;
            year_end_break_even.push(alt.break_even(0, elapsed));
        }
        let strategies = strategies::strategies(scenario, &model, &alt, window);
        (
            year_end,
            options,
            window,
            comparison,
            year_end_break_even,
            strategies,
        )
    };

    let mut notes = keep.notes.clone();
    notes.push(match window {
        TaxFreeWindow::Open(m) => format!(
            "A tax-free sale is possible until {m} (approximate: sell with a margin, the rule counts days)."
        ),
        TaxFreeWindow::Closed(m) => {
            format!("The tax-free sale window closed after {m}; any gain is taxable.")
        }
        TaxFreeWindow::Never => {
            "A tax-free sale is not possible with this ownership and occupancy history.".into()
        }
    });
    if comparison
        .iter()
        .any(|r| r.account.as_ref().is_some_and(|a| a.went_negative))
    {
        notes.push(
            "In some options the investment balance goes negative (withdrawals exceed it); that is treated as borrowing at the same return."
                .into(),
        );
    }
    let mut warnings = borettslag_notes(scenario);
    let payment = keep::fellesgjeld_payment(scenario);
    if let Some(line) = felleskost_line(&keep.cost_lines)
        && payment.0 > 0.0
    {
        notes.push(format!(
            "Felleskost invoice split: {}/month fellesgjeld (interest + principal, your share) + {}/month operating costs (whole unit).",
            payment, line.monthly_base
        ));
    }
    if let Some(w) = &scenario.wealth {
        if w.assessed_market_value.is_none() {
            warnings.push(
                "No assessed value from the tax return is set, so wealth tax uses the market value; the tax-assessed value is often lower."
                    .into(),
            );
        }
        if w.other_net_wealth == TimePath::Constant(0.0) {
            notes.push(
                "wealth.other_net_wealth is 0 (or not given), so the wealth-tax threshold is spent on this property and investment alone."
                    .into(),
            );
        }
    }
    if scenario.alternative.invest.is_none() {
        warnings.push("No investment is set, so only break-even returns are shown.".into());
    }
    Ok(Evaluation {
        keep,
        year_end,
        options,
        tax_free_window: window,
        comparison,
        year_end_break_even,
        strategies,
        invest_after_tax_return: alternative::after_tax_equivalent(scenario, rules),
        warnings,
        notes,
    })
}

/// Borettslagsloven §§ 5-5 and 5-6: renting out the whole unit needs board
/// approval and is limited to 3 years, and you must have lived there at least
/// one of the last two years. Warned about, not enforced.
fn borettslag_notes(scenario: &Scenario) -> Vec<String> {
    let mut notes = Vec::new();
    if scenario.property.kind != scenario::PropertyKind::Borettslag {
        return notes;
    }
    if scenario.horizon_months() > 36 {
        notes.push(format!(
            "Borettslag: renting out the whole unit needs board approval and is limited to 3 years (to {}) unless there are special grounds such as temporary work or study elsewhere (borettslagsloven §§ 5-5, 5-6).",
            scenario.start.add_months(35)
        ));
    }
    let lived = sale::lived_months(
        scenario,
        scenario.start.add_months(-24),
        scenario.start.add_months(-1),
    );
    if lived < 12 {
        notes.push(format!(
            "Borettslag: you (or close family) must have lived there at least 12 of the 24 months before renting out; the occupancy dates show {lived} (borettslagsloven § 5-5)."
        ));
    }
    notes
}

fn felleskost_line(lines: &[CostLine]) -> Option<&CostLine> {
    lines.iter().find(|l| l.name == "felleskostnader")
}

fn notes(years: &[YearRow]) -> Vec<String> {
    let mut notes = Vec::new();
    let borrowed: Vec<&YearRow> = years.iter().filter(|y| y.year != y.rules_year).collect();
    if let (Some(first), Some(last)) = (borrowed.first(), borrowed.last()) {
        notes.push(format!(
            "No tax rules for {}–{}; used the {} rules (the closest available).",
            first.year, last.year, first.rules_year
        ));
    }
    for y in years.iter().filter(|y| !y.rental_income_taxable) {
        notes.push(format!(
            "{}: rent collected is within the tax-free threshold, so rental income is untaxed and its costs are not deducted.",
            y.year
        ));
    }
    notes
}
