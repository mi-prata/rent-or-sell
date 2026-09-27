//! Money outside the property: the investment account, cash-flow parity,
//! keeping and prepaying the loan with savings, and break-even returns.
//!
//! **Parity:** in every option your own pocket sees exactly the cash flows of
//! plain "keep and rent out". After a sale, what keeping would have cost each
//! month is put into the investment instead (a surplus is taken out), and
//! keeping's rental tax is mirrored at each calendar year's end. In "keep and
//! prepay", the difference between its cash flows and plain keep's goes to the
//! investment. The savings lump sum exists in every option: it prepays the loan
//! in "keep and prepay" and is invested otherwise. So every row's net position
//! = property equity after sale + investment account + the same cumulative cash.
//!
//! **Wealth tax** (with `[wealth]`): each option pays its own. Your pocket pays
//! plain keep's (part of its after-tax cash flow); at each year end the account
//! pays the difference between the option's wealth tax and plain keep's.

use serde::Serialize;

use crate::explain::Explained;
use crate::loan::Prepayment;
use crate::money::Nok;
use crate::sale::SaleModel;
use crate::scenario::{Invest, InvestTax, Scenario};
use crate::tax_rules::TaxRules;
use crate::time::YearMonth;
use crate::wealth::WealthModel;
use crate::{EngineError, KeepResult, ledger, run_keep_with};

/// Monthly growth factors and tax treatment of the investment.
#[derive(Clone, Debug)]
pub struct InvestSpec {
    monthly_factor: Vec<f64>,
    tax: InvestTax,
}

impl InvestSpec {
    pub fn from_config(invest: &Invest, start: YearMonth, months: usize) -> InvestSpec {
        InvestSpec {
            monthly_factor: invest
                .annual_return
                .resolve(start, months)
                .into_iter()
                .map(|r| (1.0 + r).powf(1.0 / 12.0))
                .collect(),
            tax: invest.tax,
        }
    }

    /// A constant annual return that is already after tax.
    pub fn after_tax_constant(annual: f64, months: usize) -> InvestSpec {
        InvestSpec::constant(annual, InvestTax::AfterTax, months)
    }

    /// A constant annual return, taxed as `tax` says.
    pub fn constant(annual: f64, tax: InvestTax, months: usize) -> InvestSpec {
        InvestSpec {
            monthly_factor: vec![(1.0 + annual).powf(1.0 / 12.0); months],
            tax,
        }
    }

    pub fn tax(&self) -> InvestTax {
        self.tax
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AccountYear {
    pub year: i32,
    /// Lump sums paid in (sale proceeds, savings).
    pub deposits: Nok,
    /// Net monthly parity flows (positive = paid in).
    pub flows: Nok,
    pub growth: Nok,
    /// Tax on the year's return (`yearly` tax only).
    pub tax: Nok,
    /// This option's wealth tax for the year.
    pub wealth_tax: Nok,
    /// Plain keep's wealth tax, already paid from your pocket; the account pays
    /// `wealth_tax − keep_wealth_tax`.
    pub keep_wealth_tax: Nok,
    pub end_value: Nok,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AccountRun {
    pub deposits: Nok,
    pub flows: Nok,
    pub growth: Nok,
    pub yearly_tax: Nok,
    /// This option's wealth tax over the run (see [`AccountYear`]).
    pub wealth_tax: Nok,
    pub keep_wealth_tax: Nok,
    pub value_before_liquidation: Nok,
    /// Tax on the remaining gain when cashed out (`deferred` tax only).
    pub liquidation_tax: Nok,
    pub value: Nok,
    /// The balance went below zero at some point (treated as borrowing at the same return).
    pub went_negative: bool,
    pub years: Vec<AccountYear>,
}

/// Runs the investment account over months `0..t`. `deposits` are paid in at
/// the start of their month (before that month's growth); `flows[m]` at its
/// end. Cashed out at the start of month `t`. No wealth tax.
pub fn run_account(
    scenario: &Scenario,
    rules: &TaxRules,
    spec: &InvestSpec,
    deposits: &[(usize, Nok)],
    flows: &[f64],
    t: usize,
) -> AccountRun {
    let no_wealth_tax = |_: usize, _: Nok| (Nok::ZERO, Nok::ZERO);
    run_account_with(scenario, rules, spec, deposits, flows, t, &no_wealth_tax)
}

/// Like [`run_account`]; at each December `m`, `wealth_tax(m, value)` gives
/// the option's wealth tax and plain keep's, and the account pays the
/// difference (after any `yearly` income tax).
pub fn run_account_with(
    scenario: &Scenario,
    rules: &TaxRules,
    spec: &InvestSpec,
    deposits: &[(usize, Nok)],
    flows: &[f64],
    t: usize,
    wealth_tax: &dyn Fn(usize, Nok) -> (Nok, Nok),
) -> AccountRun {
    let (mut value, mut basis) = (0.0_f64, 0.0_f64);
    let (mut wealth_total, mut keep_wealth_total) = (0.0, 0.0);
    let mut totals = (0.0, 0.0, 0.0, 0.0); // deposits, flows, growth, yearly tax
    let mut year = (0.0, 0.0, 0.0, 0.0);
    let mut went_negative = false;
    let mut years = Vec::new();
    let steps = spec.monthly_factor.iter().zip(flows).enumerate().take(t);
    for (m, (&factor, &flow)) in steps {
        let month = scenario.start.add_months(m as i64);
        let deposit: f64 = deposits.iter().filter(|d| d.0 == m).map(|d| d.1.0).sum();
        value += deposit;
        basis += deposit;
        let growth = value * (factor - 1.0);
        value += growth;
        value += flow;
        // Basis = net money paid in, even below zero: the account is marginal
        // (a negative balance means holding less of the investment), so the
        // whole gain is taxed on cashing out, including gains withdrawn earlier.
        basis += flow;
        went_negative |= value < -1e-6;
        year.0 += deposit;
        year.1 += flow;
        year.2 += growth;

        let year_end = month.month() == 12 || m + 1 == t;
        if year_end {
            if spec.tax == InvestTax::Yearly {
                let tax = year.2 * rules.for_year(month.year()).capital_income_rate.value.0;
                value -= tax;
                year.3 = tax;
            }
            let (wealth, keep_wealth) = if month.month() == 12 {
                wealth_tax(m, Nok(value))
            } else {
                (Nok::ZERO, Nok::ZERO)
            };
            // Paid from the account, so its basis shrinks like any withdrawal.
            value -= (wealth - keep_wealth).0;
            basis -= (wealth - keep_wealth).0;
            went_negative |= value < -1e-6;
            wealth_total += wealth.0;
            keep_wealth_total += keep_wealth.0;
            if year != (0.0, 0.0, 0.0, 0.0) || value.abs() > 1e-9 || wealth.0 != 0.0 {
                years.push(AccountYear {
                    year: month.year(),
                    deposits: Nok(year.0),
                    flows: Nok(year.1),
                    growth: Nok(year.2),
                    tax: Nok(year.3),
                    wealth_tax: wealth,
                    keep_wealth_tax: keep_wealth,
                    end_value: Nok(value),
                });
            }
            totals = (
                totals.0 + year.0,
                totals.1 + year.1,
                totals.2 + year.2,
                totals.3 + year.3,
            );
            year = (0.0, 0.0, 0.0, 0.0);
        }
    }
    let liquidation_tax = if spec.tax == InvestTax::Deferred {
        let set = rules.for_year(scenario.start.add_months(t as i64).year());
        (value - basis) * set.capital_income_rate.value.0 * set.share_income_uplift_factor.value
    } else {
        0.0
    };
    AccountRun {
        deposits: Nok(totals.0),
        flows: Nok(totals.1),
        growth: Nok(totals.2),
        yearly_tax: Nok(totals.3),
        wealth_tax: Nok(wealth_total),
        keep_wealth_tax: Nok(keep_wealth_total),
        value_before_liquidation: Nok(value),
        liquidation_tax: Nok(liquidation_tax),
        value: Nok(value - liquidation_tax),
        went_negative,
        years,
    }
}

/// The constant after-tax annual return that the configured investment
/// amounts to over the horizon: 1 kr invested at `start` and cashed out at
/// the horizon, after its income tax (no wealth tax).
pub fn after_tax_equivalent(scenario: &Scenario, rules: &TaxRules) -> Option<f64> {
    let invest = scenario.alternative.invest.as_ref()?;
    let n = scenario.horizon_months();
    let spec = InvestSpec::from_config(invest, scenario.start, n);
    let run = run_account(scenario, rules, &spec, &[(0, Nok(1.0))], &vec![0.0; n], n);
    Some(run.value.0.max(0.0).powf(12.0 / n as f64) - 1.0)
}

/// Monthly flows into the investment after a sale at `s` that give your
/// pocket the same cash flows as keeping (see module docs).
pub fn parity_flows(keep: &KeepResult, rules: &TaxRules, s: usize) -> Vec<f64> {
    let mut flows = vec![0.0; keep.months.len()];
    let mut a = 0;
    for y in &keep.years {
        let b = a + y.months as usize;
        if b > s {
            let after = a.max(s)..b;
            for (flow, month) in flows[after.clone()].iter_mut().zip(&keep.months[after]) {
                *flow = -month.pre_tax_cash_flow.0;
            }
            // Tax for the part of the sale year before `s` is already in the cash before the sale.
            let before = if a < s {
                ledger::aggregate(&keep.cost_lines, &keep.months[a..s], rules)[0]
                    .tax
                    .0
            } else {
                0.0
            };
            flows[b - 1] += y.tax.0 - before;
        }
        a = b;
    }
    flows
}

/// Flows moving the difference between `variant`'s after-tax cash flows and
/// `base`'s into the investment, so your pocket sees `base`'s.
pub fn difference_flows(base: &KeepResult, variant: &KeepResult) -> Vec<f64> {
    let mut flows: Vec<f64> = base
        .months
        .iter()
        .zip(&variant.months)
        .map(|(b, v)| (v.pre_tax_cash_flow - b.pre_tax_cash_flow).0)
        .collect();
    let mut end = 0;
    for (b, v) in base.years.iter().zip(&variant.years) {
        end += b.months as usize;
        flows[end - 1] -= (v.tax - b.tax).0;
    }
    flows
}

/// The after-tax annual return at which two options are equal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "rate", rename_all = "snake_case")]
pub enum BreakEven {
    Rate(f64),
    /// Even this (the lowest return searched) is enough.
    Below(f64),
    /// Even this (the highest return searched) is not enough.
    Above(f64),
}

const SEARCH: (f64, f64) = (-0.5, 1.0);

/// Bisection for the root of an increasing `f` over [`SEARCH`].
fn solve(f: impl Fn(f64) -> f64) -> BreakEven {
    let (mut lo, mut hi) = SEARCH;
    if f(lo) >= 0.0 {
        return BreakEven::Below(lo);
    }
    if f(hi) <= 0.0 {
        return BreakEven::Above(hi);
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    BreakEven::Rate(0.5 * (lo + hi))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Keep,
    KeepPrepay,
    Sell,
}

/// One option in the keep-vs-sell comparison, valued at `target`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ComparisonRow {
    pub label: String,
    pub kind: RowKind,
    pub target: YearMonth,
    pub sale_month: Option<YearMonth>,
    /// Keep rows: equity after selling at `target`. Sell rows: proceeds (equity after the sale).
    pub property_equity: Nok,
    /// `None` when no `[alternative.invest]` is configured (and nothing needs investing).
    pub account: Option<AccountRun>,
    /// Plain keep's cumulative after-tax cash up to `target`; identical in every row.
    pub cumulative_cash: Nok,
    /// Wealth tax this option pays up to `target` (plain keep's from your
    /// pocket plus the difference from the account). `None` when unknown
    /// (a sale without an investment configured).
    pub wealth_tax: Option<Nok>,
    pub net_position: Option<Nok>,
    pub vs_keep: Option<Nok>,
    /// Sell rows: after-tax return the proceeds need to match plain keep.
    /// Keep-and-prepay: return the savings need to beat prepaying.
    pub break_even: Option<BreakEven>,
}

/// What an option holds besides its investment account, for wealth tax.
#[derive(Clone, Copy)]
enum Held<'b> {
    /// The property, with this simulation's loan balance.
    Keep(&'b KeepResult),
    /// The property (plain keep's loan) until it is sold at this offset.
    SoldAt(usize),
}

/// Evaluates the comparison rows and break-even returns.
pub struct AltModel<'a> {
    scenario: &'a Scenario,
    rules: &'a TaxRules,
    keep: &'a KeepResult,
    sale: &'a SaleModel<'a>,
    spec: Option<InvestSpec>,
    prepay: Option<(Prepayment, KeepResult)>,
    wealth: Option<WealthModel<'a>>,
}

impl<'a> AltModel<'a> {
    pub fn new(
        scenario: &'a Scenario,
        rules: &'a TaxRules,
        keep: &'a KeepResult,
        sale: &'a SaleModel<'a>,
    ) -> Result<Self, EngineError> {
        let n = scenario.horizon_months();
        let spec = scenario
            .alternative
            .invest
            .as_ref()
            .map(|i| InvestSpec::from_config(i, scenario.start, n));
        let prepay = match (&scenario.savings, &scenario.loan) {
            (Some(savings), Some(_)) => {
                let p = Prepayment {
                    offset: scenario.savings_month().months_since(scenario.start) as usize,
                    amount: savings.amount,
                };
                Some((p, run_keep_with(scenario, rules, Some(p))?))
            }
            _ => None,
        };
        Ok(AltModel {
            scenario,
            rules,
            keep,
            sale,
            spec,
            prepay,
            wealth: WealthModel::new(scenario, rules),
        })
    }

    fn n(&self) -> usize {
        self.scenario.horizon_months()
    }

    fn savings_deposit(&self) -> Option<(usize, Nok)> {
        self.scenario.savings.as_ref().map(|s| {
            let offset = self
                .scenario
                .savings_month()
                .months_since(self.scenario.start);
            (offset as usize, s.amount)
        })
    }

    fn cumulative_cash(&self, t: usize) -> Nok {
        ledger::aggregate(&self.keep.cost_lines, &self.keep.months[..t], self.rules)
            .iter()
            .map(|y| y.after_tax_cash_flow)
            .sum()
    }

    /// Plain keep's wealth tax (paid from your pocket) before month `t`.
    fn keep_wealth_tax(&self, t: usize) -> Nok {
        self.keep.months[..t].iter().map(|m| m.wealth_tax).sum()
    }

    /// An option's wealth tax and plain keep's, at the end of December `m`,
    /// with the account worth `account`.
    fn wealth_tax(&self, held: Held<'_>, m: usize, account: Nok) -> (Nok, Nok) {
        let Some(w) = &self.wealth else {
            return (Nok::ZERO, Nok::ZERO);
        };
        let debt_of = |k: &KeepResult| k.months[m].loan_balance + k.months[m].fellesgjeld_balance;
        let debt = match held {
            Held::Keep(k) => Some(debt_of(k)),
            Held::SoldAt(s) => (m < s).then(|| debt_of(self.keep)),
        };
        let mut holdings = w.account(account);
        if let Some(debt) = debt {
            holdings = holdings + w.property(m, debt);
        }
        (w.assess(m, &holdings).tax, self.keep.months[m].wealth_tax)
    }

    fn target(&self, t: usize) -> YearMonth {
        self.scenario.start.add_months(t as i64)
    }

    /// Plain keep until `t`, with the savings (if any) invested.
    pub fn keep_row(&self, t: usize) -> ComparisonRow {
        let equity = self.sale.outcome(t).equity;
        let account = match (&self.spec, self.savings_deposit()) {
            (Some(spec), Some(d)) => {
                Some(self.run(spec, &[d], &vec![0.0; self.n()], t, Held::Keep(self.keep)))
            }
            _ => None,
        };
        let wealth_tax = account
            .as_ref()
            .map_or(self.keep_wealth_tax(t), |a| a.wealth_tax);
        let cash = self.cumulative_cash(t);
        let net = equity + account.as_ref().map_or(Nok::ZERO, |a| a.value) + cash;
        ComparisonRow {
            label: if account.is_some() {
                "Rent (savings invested)".into()
            } else {
                "Rent".into()
            },
            kind: RowKind::Keep,
            target: self.target(t),
            sale_month: None,
            property_equity: equity,
            account,
            cumulative_cash: cash,
            wealth_tax: Some(wealth_tax),
            net_position: Some(net),
            vs_keep: Some(Nok::ZERO),
            break_even: None,
        }
    }

    /// Keep until `t`, having prepaid the loan with the savings. `None` without
    /// savings, a loan, or an investment for the freed cash.
    pub fn keep_prepay_row(&self, t: usize) -> Option<ComparisonRow> {
        let (prepayment, variant) = self.prepay.as_ref()?;
        let spec = self.spec.as_ref()?;
        let variant_sale = SaleModel::new(self.scenario, self.rules, variant);
        let equity = variant_sale.outcome(t).equity;
        let flows = difference_flows(self.keep, variant);
        let excess = self.excess_prepayment(*prepayment, variant);
        let deposits: Vec<(usize, Nok)> = excess.into_iter().collect();
        let account = self.run(spec, &deposits, &flows, t, Held::Keep(variant));
        let cash = self.cumulative_cash(t);
        let net = equity + account.value + cash;
        let keep_net = self.keep_row(t).net_position.unwrap();

        // Return at which investing the savings instead matches prepaying.
        let savings = self.savings_deposit().unwrap();
        let plain_equity = self.sale.outcome(t).equity;
        let zero_flows = vec![0.0; self.n()];
        let break_even = solve(|r| {
            let s = InvestSpec::after_tax_constant(r, self.n());
            let invested = self
                .run(&s, &[savings], &zero_flows, t, Held::Keep(self.keep))
                .value;
            let prepaid = self
                .run(&s, &deposits, &flows, t, Held::Keep(variant))
                .value;
            (plain_equity + invested - equity - prepaid).0
        });
        Some(ComparisonRow {
            label: "Rent, prepay loan with savings".into(),
            kind: RowKind::KeepPrepay,
            target: self.target(t),
            sale_month: None,
            property_equity: equity,
            wealth_tax: Some(account.wealth_tax),
            account: Some(account),
            cumulative_cash: cash,
            net_position: Some(net),
            vs_keep: Some(net - keep_net),
            break_even: Some(break_even),
        })
    }

    /// Savings beyond what the loan could absorb, invested at the prepayment month.
    fn excess_prepayment(
        &self,
        prepayment: Prepayment,
        variant: &KeepResult,
    ) -> Option<(usize, Nok)> {
        let opening = match prepayment.offset {
            0 => self.scenario.loan.as_ref().map_or(Nok::ZERO, |l| l.balance),
            k => variant.months[k - 1].loan_balance,
        };
        let excess = prepayment.amount - prepayment.amount.min(opening);
        (excess.0 > 0.0).then_some((prepayment.offset, excess))
    }

    /// Sell at `s`, invest the proceeds (and savings), value at `t`.
    pub fn sell_row(&self, label: impl Into<String>, s: usize, t: usize) -> ComparisonRow {
        let proceeds = self.sale.outcome(s).equity;
        let cash = self.cumulative_cash(t);
        let account = self.spec.as_ref().map(|spec| {
            let mut deposits = vec![(s, proceeds)];
            deposits.extend(self.savings_deposit());
            let flows = parity_flows(self.keep, self.rules, s);
            self.run(spec, &deposits, &flows, t, Held::SoldAt(s))
        });
        let keep_net = self.keep_row(t).net_position.unwrap();
        let net = account.as_ref().map(|a| a.value + cash);
        ComparisonRow {
            label: label.into(),
            kind: RowKind::Sell,
            target: self.target(t),
            sale_month: Some(self.target(s)),
            property_equity: proceeds,
            wealth_tax: account.as_ref().map(|a| a.wealth_tax),
            account,
            cumulative_cash: cash,
            net_position: net,
            vs_keep: net.map(|n| n - keep_net),
            break_even: Some(self.break_even(s, t)),
        }
    }

    /// The configured investment, if any.
    pub fn spec(&self) -> Option<&InvestSpec> {
        self.spec.as_ref()
    }

    /// Selling at `s`, the money invested with `spec`, cashed out at `t`:
    /// the investment account minus the savings account plain keep holds,
    /// since the savings are the same in both. So it equals plain keep's
    /// property equity at `t` plus a sell row's `vs_keep`, and with `s >= t`
    /// (never sold before `t`) it is that equity.
    pub fn sold_value(&self, spec: &InvestSpec, s: usize, t: usize) -> Nok {
        if s >= t {
            return self.sale.outcome(t).equity;
        }
        let savings = self.savings_value(spec, t);
        self.sold_value_with(spec, s, t, savings)
    }

    /// [`Self::sold_value`] with the savings account's value at `t` already
    /// known (it is the same for every sale month).
    pub fn sold_value_with(&self, spec: &InvestSpec, s: usize, t: usize, savings: Nok) -> Nok {
        if s >= t {
            return self.sale.outcome(t).equity;
        }
        let mut deposits = vec![(s, self.sale.outcome(s).equity)];
        deposits.extend(self.savings_deposit());
        let flows = parity_flows(self.keep, self.rules, s);
        self.run(spec, &deposits, &flows, t, Held::SoldAt(s)).value - savings
    }

    /// Plain keep's savings account at `t`, invested with `spec`.
    pub fn savings_value(&self, spec: &InvestSpec, t: usize) -> Nok {
        match self.savings_deposit() {
            Some(d) => {
                let zero = vec![0.0; self.n()];
                self.run(spec, &[d], &zero, t, Held::Keep(self.keep)).value
            }
            None => Nok::ZERO,
        }
    }

    /// The constant annual return, taxed like the configured investment,
    /// at which selling now and investing is worth `target` at `t`. `None`
    /// without an investment.
    pub fn equivalent_return(&self, target: Nok, t: usize) -> Option<BreakEven> {
        let tax = self.spec.as_ref()?.tax;
        Some(solve(|r| {
            let spec = InvestSpec::constant(r, tax, self.n());
            (self.sold_value(&spec, 0, t) - target).0
        }))
    }

    /// After-tax annual return the proceeds of a sale at `s` must earn to
    /// match plain keep at `t` (savings left out: they are the same in both).
    pub fn break_even(&self, s: usize, t: usize) -> BreakEven {
        let proceeds = self.sale.outcome(s).equity;
        let target = self.sale.outcome(t).equity;
        let flows = parity_flows(self.keep, self.rules, s);
        solve(|r| {
            let spec = InvestSpec::after_tax_constant(r, self.n());
            let run = self.run(&spec, &[(s, proceeds)], &flows, t, Held::SoldAt(s));
            (run.value - target).0
        })
    }

    fn run(
        &self,
        spec: &InvestSpec,
        deposits: &[(usize, Nok)],
        flows: &[f64],
        t: usize,
        held: Held<'_>,
    ) -> AccountRun {
        let wealth_tax = |m, account| self.wealth_tax(held, m, account);
        run_account_with(
            self.scenario,
            self.rules,
            spec,
            deposits,
            flows,
            t,
            &wealth_tax,
        )
    }

    /// How a comparison row's net position was derived.
    pub fn explain(&self, row: &ComparisonRow) -> Explained {
        let mut parts = Vec::new();
        match row.kind {
            RowKind::Sell => parts.push(
                Explained::value("Sale proceeds invested", row.property_equity).formula(format!(
                    "equity after selling {} (see `explain --at`)",
                    row.sale_month.unwrap()
                )),
            ),
            RowKind::Keep | RowKind::KeepPrepay => parts.push(
                Explained::value("Property equity if sold", row.property_equity)
                    .formula(format!("equity after selling {}", row.target)),
            ),
        }
        if let Some(a) = &row.account {
            let tax = match self.scenario.alternative.invest.as_ref().map(|i| i.tax) {
                Some(InvestTax::Deferred) => "gain × capital income rate × share income factor",
                Some(InvestTax::Yearly) => "each year's return × capital income rate",
                _ => "none (return entered after tax)",
            };
            let mut years: Vec<Explained> = a
                .years
                .iter()
                .map(|y| {
                    let mut formula = format!(
                        "deposits {} + parity flows {} + growth {} − tax {}",
                        y.deposits, y.flows, y.growth, y.tax
                    );
                    if self.wealth.is_some() {
                        formula.push_str(&format!(
                            " − (wealth tax {} − renting's {}, paid from cash)",
                            y.wealth_tax, y.keep_wealth_tax
                        ));
                    }
                    Explained::value(format!("{}", y.year), y.end_value).formula(formula)
                })
                .collect();
            years.push(
                Explained::value("Tax on cashing out", a.liquidation_tax)
                    .formula(tax)
                    .source(format!(
                        "tax-rules {}: capital_income_rate, share_income_uplift_factor",
                        self.rules.for_year(row.target.year()).income_year
                    )),
            );
            parts.push(
                Explained::value("Investment account", a.value)
                    .formula("value before cashing out − tax on cashing out; year-end values below")
                    .parts(years),
            );
        }
        parts.push(
            Explained::value("Cumulative cash from renting (same in every option)", row.cumulative_cash)
                .formula("sum of plain renting's after-tax cash flows; after a sale they flow via the investment"),
        );
        if let (Some(_), Some(wealth_tax)) = (&self.wealth, row.wealth_tax) {
            parts.push(
                Explained::value("Memo: wealth tax this option paid (already included)", wealth_tax)
                    .formula("plain renting's from the cumulative cash + the difference from the investment account"),
            );
        }
        let formula = match row.kind {
            RowKind::Sell => "investment account + cumulative cash",
            _ => "property equity + investment account + cumulative cash",
        };
        let mut root = Explained::info(format!("{} — wealth at {}", row.label, row.target));
        root.value = row.net_position;
        root.formula(formula).parts(parts)
    }
}
