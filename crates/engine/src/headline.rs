//! The headline figures of an evaluation: what the CLI summary and the web UI
//! show first. Plain numbers (kr as `f64`, months as `"YYYY-MM"`) so it
//! serialises into a simple view model.
//!
//! "The same money in, two outcomes": keeping and selling now both start from
//! the equity a sale would free up plus the same monthly cash flows; the end
//! values below are chosen so that their difference is exactly the
//! keep-vs-sell gap.

use serde::Serialize;

use crate::alternative::BreakEven;
use crate::assumptions::{AssumptionGroup, assumptions};
use crate::sale::TaxFreeWindow;
use crate::scenario::InvestTax;
use crate::scenario::Scenario;
use crate::strategies::{NEAR_TIE, StrategyKind};
use crate::timepath::TimePath;
use crate::{Evaluation, MonthRow, YearRow};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct Headline {
    pub name: String,
    /// First simulated month.
    pub start: String,
    /// Month the options are valued at (just after the last simulated month).
    pub horizon: String,
    pub horizon_years: u32,
    /// Keep's end value minus selling now's: positive means keep. `None`
    /// without `[alternative.invest]`.
    pub gap: Option<f64>,
    /// Equity a sale now would free up (after costs, debt and gain tax).
    pub proceeds_now: f64,
    /// Your equity in the flat at the horizon, after sale costs, debts and tax.
    pub keep_end: f64,
    /// The same money invested (selling now).
    pub sell_end: Option<f64>,
    /// Returns (the investment's and the strategies' equivalents) are before
    /// tax: the investment is taxed yearly or on cashing out.
    pub returns_before_tax: bool,
    /// Sell now, rent until a month then sell, rent to the horizon; all
    /// valued at the horizon. Empty without `[alternative.invest]`.
    pub strategies: Vec<StrategyRow>,
    /// Selling now vs renting for the whole period, and what the months in
    /// between add. `None` without `[alternative.invest]`.
    pub verdict: Option<Verdict>,
    /// Every sale month from now to the horizon, valued at the horizon.
    pub by_sale: Vec<SalePoint>,
    /// The strategies valued at constant returns (taxed like the investment).
    pub by_return: Vec<ReturnPoint>,
    /// The owner's monthly cash flow (after tax), the same in every option,
    /// in phases split where a debt is paid off or the flow changes direction.
    pub cash: Vec<CashPhase>,
    pub tax_free: TaxFree,
    pub warnings: Vec<String>,
    /// The mortgage's final payment, if it falls within the horizon.
    pub loan_paid_off: Option<PaidOff>,
    /// The simplifications that apply to this scenario, by topic.
    pub assumptions: Vec<AssumptionGroup>,
}

/// One strategy, valued at the horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct StrategyRow {
    pub kind: StrategyKind,
    /// Month of the sale (the horizon for renting throughout).
    pub sale_month: String,
    pub value: f64,
    pub vs_sell_now: f64,
    /// The yearly return, taxed like the investment, at which selling now
    /// ends up at `value`. `None` when outside the searched range (see `bound`).
    pub equivalent_return: Option<f64>,
    pub bound: Option<Bound>,
    /// Within [`NEAR_TIE`] of the verdict's winner (and not the winner).
    pub about_same: bool,
}

/// The headline decision: selling now or renting for the whole period (the
/// two ends of the choice), and whether selling at a month in between
/// changes it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct Verdict {
    /// Index into `strategies` of the winner: selling now (0) or renting (last).
    pub winner: usize,
    /// The other end of the choice.
    pub loser: usize,
    /// Winner minus loser (≥ 0).
    pub gap: f64,
    /// `gap` as a share of the loser's value; `None` when that is not positive.
    pub gap_share: Option<f64>,
    /// The gap is within [`NEAR_TIE`] of the winner's value.
    pub tie: bool,
    /// Renting's equivalent return: selling now wins above it.
    pub break_even: Option<f64>,
    pub break_even_bound: Option<Bound>,
    /// Selling now's equivalent return: the chosen one.
    pub chosen_return: Option<f64>,
    pub timing: Option<Timing>,
}

/// What selling at a month in between adds to the verdict.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Timing {
    /// This strategy beats the winner by more than [`NEAR_TIE`].
    Better {
        index: usize,
        gain: f64,
        gain_share: f64,
    },
    /// The tax-free sale is within [`NEAR_TIE`] of the winner.
    AboutSame { index: usize },
}

/// Selling in one month, valued at the horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct SalePoint {
    /// Months after the start (0 = now).
    pub offset: u32,
    pub month: String,
    pub value: f64,
    pub vs_sell_now: f64,
    pub tax_free: bool,
}

/// The strategies at one constant yearly return.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct ReturnPoint {
    pub rate: f64,
    /// Aligned with `Headline.strategies`.
    pub values: Vec<f64>,
}

/// When a loan is paid off.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct PaidOff {
    /// Month of the final payment.
    pub month: String,
    /// Months after the start at the end of that month (for a time axis).
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum Bound {
    /// Below the lowest return searched (for renting: selling wins even
    /// against a poor investment).
    Below,
    /// Above the highest return searched.
    Above,
}

/// A stretch of months with one kind of cash flow: paid in (negative) or
/// paid out (positive), after tax. Renting sets the amounts; under cash-flow
/// parity every option has them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct CashPhase {
    /// First and last month of the phase.
    pub start: String,
    pub end: String,
    /// Per month in the phase's first full calendar year, or the phase's
    /// average when it has none. Negative means paid in.
    pub monthly: f64,
    /// Sum over the phase; the phases add up to renting's cumulative cash.
    pub total: f64,
    /// How the amount moves from the first to the last full year.
    pub trend: Trend,
    /// The constant yearly rent growth, when a paid-out amount rises with it.
    pub rent_growth: Option<f64>,
    /// The debt whose final payment is the phase's last month.
    pub ends_with: Option<Debt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    Flat,
    Rising,
    Falling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum Debt {
    Loan,
    Fellesgjeld,
}

/// Change in size between the first and last full year that still counts as flat.
const FLAT: f64 = 0.05;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct TaxFree {
    pub status: TaxFreeStatus,
    /// Last qualifying month (open or closed).
    pub month: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum TaxFreeStatus {
    Open,
    Closed,
    Never,
}

pub fn headline(scenario: &Scenario, e: &Evaluation) -> Headline {
    let keep = &e.comparison[0];
    let keep_end = keep.property_equity;
    let sell_now = e
        .strategies
        .as_ref()
        .and_then(|st| st.strategies.first())
        .map(|st| st.value);
    let sell_end = sell_now.map(|v| v.0);
    let month = |offset: usize| scenario.start.add_months(offset as i64).to_string();
    let (strategies, by_sale, by_return) = match (&e.strategies, sell_now) {
        (Some(st), Some(now)) => (
            st.strategies
                .iter()
                .map(|x| {
                    let (equivalent_return, bound) = match x.equivalent_return {
                        BreakEven::Rate(r) => (Some(r), None),
                        BreakEven::Below(_) => (None, Some(Bound::Below)),
                        BreakEven::Above(_) => (None, Some(Bound::Above)),
                    };
                    StrategyRow {
                        kind: x.kind,
                        sale_month: month(x.sale_offset),
                        value: x.value.0,
                        vs_sell_now: (x.value - now).0,
                        equivalent_return,
                        bound,
                        about_same: false,
                    }
                })
                .collect(),
            st.by_sale
                .iter()
                .map(|p| SalePoint {
                    offset: p.offset as u32,
                    month: month(p.offset),
                    value: p.value.0,
                    vs_sell_now: (p.value - now).0,
                    tax_free: p.tax_free,
                })
                .collect(),
            st.by_return
                .iter()
                .map(|p| ReturnPoint {
                    rate: p.rate,
                    values: p.values.iter().map(|v| v.0).collect(),
                })
                .collect(),
        ),
        _ => (Vec::new(), Vec::new(), Vec::new()),
    };
    let mut strategies: Vec<StrategyRow> = strategies;
    let verdict = verdict(&strategies);
    if let Some(v) = &verdict {
        let top = strategies[v.winner].value;
        for (i, x) in strategies.iter_mut().enumerate() {
            x.about_same = i != v.winner && (x.value - top).abs() <= NEAR_TIE * top.abs();
        }
    }
    let returns_before_tax = scenario
        .alternative
        .invest
        .as_ref()
        .is_some_and(|i| i.tax != InvestTax::AfterTax);

    let tax_free = match e.tax_free_window {
        TaxFreeWindow::Open(m) => TaxFree {
            status: TaxFreeStatus::Open,
            month: Some(m.to_string()),
        },
        TaxFreeWindow::Closed(m) => TaxFree {
            status: TaxFreeStatus::Closed,
            month: Some(m.to_string()),
        },
        TaxFreeWindow::Never => TaxFree {
            status: TaxFreeStatus::Never,
            month: None,
        },
    };

    let proceeds_now = e.options[0].outcome.equity.0;

    let loan_paid_off = scenario
        .loan
        .as_ref()
        .filter(|l| l.balance.0 > 0.0)
        .and_then(|_| e.keep.months.iter().position(|m| m.loan_balance.0 <= 0.0))
        .map(|i| PaidOff {
            month: e.keep.months[i].month.to_string(),
            offset: i as u32 + 1,
        });

    Headline {
        name: scenario.name.clone(),
        start: scenario.start.to_string(),
        horizon: scenario
            .start
            .add_months(scenario.horizon_months() as i64)
            .to_string(),
        horizon_years: scenario.horizon_years,
        gap: sell_end.map(|s| keep_end.0 - s),
        proceeds_now,
        keep_end: keep_end.0,
        sell_end,
        returns_before_tax,
        strategies,
        verdict,
        by_sale,
        by_return,
        cash: cash_phases(scenario, e),
        tax_free,
        warnings: e.warnings.clone(),
        loan_paid_off,
        assumptions: assumptions(scenario),
    }
}

/// Renting's after-tax cash flow in phases. Each month carries its
/// pre-tax cash flow plus an even share of its year's tax and wealth tax, so
/// the phases add up to the yearly figures. A phase ends at a debt's final
/// payment, or before a full year whose cash flow changes direction.
fn cash_phases(scenario: &Scenario, e: &Evaluation) -> Vec<CashPhase> {
    let months = &e.keep.months;
    let years = &e.keep.years;
    if months.is_empty() {
        return Vec::new();
    }
    let year_of = |m: &MonthRow| years.iter().find(|y| y.year == m.month.year());
    let flow: Vec<f64> = months
        .iter()
        .map(|m| {
            let y = year_of(m).expect("every month has its year");
            (m.pre_tax_cash_flow - (y.tax + y.wealth_tax) * (1.0 / y.months as f64)).0
        })
        .collect();

    // Where each phase ends (inclusive month index).
    let mut ends: Vec<(usize, Option<Debt>)> = Vec::new();
    let paid_off = |balance: &dyn Fn(&MonthRow) -> f64, initial: f64| {
        (initial > 0.0)
            .then(|| months.iter().position(|m| balance(m) <= 0.0))
            .flatten()
    };
    let loan0 = scenario.loan.as_ref().map_or(0.0, |l| l.balance.0);
    let fg0 = e
        .keep
        .months
        .first()
        .map_or(0.0, |m| (m.fellesgjeld_balance + m.fellesgjeld_principal).0);
    if let Some(i) = paid_off(&|m| m.loan_balance.0, loan0) {
        ends.push((i, Some(Debt::Loan)));
    }
    if let Some(i) = paid_off(&|m| m.fellesgjeld_balance.0, fg0) {
        ends.push((i, Some(Debt::Fellesgjeld)));
    }
    let full: Vec<&YearRow> = years.iter().filter(|y| y.months == 12).collect();
    let first_of = |year: i32| months.iter().position(|m| m.month.year() == year);
    let payoffs: Vec<usize> = ends.iter().map(|&(i, _)| i).collect();
    for w in full.windows(2) {
        let (a, b) = (w[0].after_tax_cash_flow.0, w[1].after_tax_cash_flow.0);
        if let (true, Some(from), Some(to)) = (
            (a < 0.0) != (b < 0.0),
            first_of(w[0].year),
            first_of(w[1].year),
        ) {
            // A payoff between the two years already explains the change.
            if !payoffs.iter().any(|&p| p >= from && p < to) && to > 0 {
                ends.push((to - 1, None));
            }
        }
    }
    ends.push((months.len() - 1, None));
    ends.sort_by_key(|&(i, d)| (i, d.is_none()));
    ends.dedup_by_key(|&mut (i, _)| i);

    let rent_growth = match scenario.rental.rent_growth {
        TimePath::Constant(g) => Some(g),
        _ => None,
    };
    let mut from = 0;
    ends.into_iter()
        .map(|(to, ends_with)| {
            let span = from..=to;
            // Full calendar years inside the phase.
            let inside: Vec<&YearRow> = full
                .iter()
                .copied()
                .filter(|y| {
                    months[span.clone()]
                        .iter()
                        .filter(|m| m.month.year() == y.year)
                        .count()
                        == 12
                })
                .collect();
            let total: f64 = flow[span.clone()].iter().sum();
            let monthly = inside
                .first()
                .map_or(total / (to - from + 1) as f64, |y| y.after_tax_monthly.0);
            let trend = match (inside.first(), inside.last()) {
                (Some(a), Some(b)) if a.after_tax_monthly.0 != 0.0 => {
                    let ratio = b.after_tax_monthly.0 / a.after_tax_monthly.0;
                    if ratio > 1.0 + FLAT {
                        Trend::Rising
                    } else if ratio < 1.0 - FLAT {
                        Trend::Falling
                    } else {
                        Trend::Flat
                    }
                }
                _ => Trend::Flat,
            };
            let phase = CashPhase {
                start: months[from].month.to_string(),
                end: months[to].month.to_string(),
                monthly,
                total,
                trend,
                rent_growth: rent_growth.filter(|_| trend == Trend::Rising && monthly > 0.0),
                ends_with,
            };
            from = to + 1;
            phase
        })
        .collect()
}

/// Selling now vs renting for the whole period; the in-between strategies
/// only change the story when they beat the winner by more than [`NEAR_TIE`].
fn verdict(strategies: &[StrategyRow]) -> Option<Verdict> {
    if strategies.len() < 2 {
        return None;
    }
    let (now, rent) = (0, strategies.len() - 1);
    let (winner, loser) = if strategies[rent].value >= strategies[now].value {
        (rent, now)
    } else {
        (now, rent)
    };
    let (w, l) = (&strategies[winner], &strategies[loser]);
    let gap = w.value - l.value;
    let near = |a: f64, b: f64| (a - b).abs() <= NEAR_TIE * b.abs();
    let between = 1..rent;
    let better = between
        .clone()
        .filter(|&i| strategies[i].value - w.value > NEAR_TIE * w.value.abs())
        .max_by(|&a, &b| strategies[a].value.total_cmp(&strategies[b].value));
    let timing = match better {
        Some(i) => Some(Timing::Better {
            index: i,
            gain: strategies[i].value - w.value,
            gain_share: (strategies[i].value - w.value) / w.value.abs(),
        }),
        None => between
            .filter(|&i| strategies[i].kind == StrategyKind::SellLastTaxFree)
            .find(|&i| near(strategies[i].value, w.value))
            .map(|index| Timing::AboutSame { index }),
    };
    Some(Verdict {
        winner,
        loser,
        gap,
        gap_share: (l.value > 0.0).then(|| gap / l.value),
        tie: near(l.value, w.value),
        break_even: strategies[rent].equivalent_return,
        break_even_bound: strategies[rent].bound,
        chosen_return: strategies[now].equivalent_return,
        timing,
    })
}
