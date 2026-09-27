//! One-at-a-time sensitivity: re-runs the scenario with one input bumped and
//! reports the headline figures, to show which inputs move the decision.

use serde::Serialize;

use crate::alternative::{BreakEven, RowKind};
use crate::money::Nok;
use crate::scenario::{Cost, Scenario, WEEKS_PER_YEAR};
use crate::tax_rules::TaxRules;
use crate::{EngineError, evaluate};

/// The headline figures of one run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SensitivityRow {
    pub label: String,
    /// Keep (with any savings invested): net position at the horizon.
    pub keep_net_position: Nok,
    /// Selling now and investing, minus keeping (`None` without `[alternative.invest]`).
    pub sell_now_vs_keep: Option<Nok>,
    /// After-tax return the proceeds of selling now need to match keeping.
    pub sell_now_break_even: Option<BreakEven>,
}

type Bump = (&'static str, fn(&mut Scenario));

/// The fixed set of bumps, each applied on its own to the scenario.
const BUMPS: &[Bump] = &[
    ("Interest rate +1 pp", |s| shift_rates(s, 0.01)),
    ("Interest rate −1 pp", |s| shift_rates(s, -0.01)),
    ("Price growth +1 pp", |s| {
        s.property.price_growth = s.property.price_growth.map(|g| g + 0.01)
    }),
    ("Price growth −1 pp", |s| {
        s.property.price_growth = s.property.price_growth.map(|g| g - 0.01)
    }),
    ("Rent +10%", |s| {
        s.rental.monthly_rent = s.rental.monthly_rent * 1.1
    }),
    ("Rent −10%", |s| {
        s.rental.monthly_rent = s.rental.monthly_rent * 0.9
    }),
    ("Vacancy +1 month/yr", |s| {
        s.rental.vacancy_weeks =
            (s.rental.vacancy_weeks + WEEKS_PER_YEAR / 12.0).min(WEEKS_PER_YEAR)
    }),
    ("Running costs +20%", |s| scale_costs(s, 1.2)),
];

/// The base run followed by one row per bump.
pub fn sensitivity(
    scenario: &Scenario,
    rules: &TaxRules,
) -> Result<Vec<SensitivityRow>, EngineError> {
    let mut rows = vec![run("Base", scenario, rules)?];
    for (label, bump) in BUMPS {
        let mut s = scenario.clone();
        bump(&mut s);
        rows.push(run(label, &s, rules)?);
    }
    Ok(rows)
}

fn run(label: &str, scenario: &Scenario, rules: &TaxRules) -> Result<SensitivityRow, EngineError> {
    let e = evaluate(scenario, rules)?;
    let keep = &e.comparison[0];
    let sell_now = e
        .comparison
        .iter()
        .find(|r| r.kind == RowKind::Sell && r.sale_month == Some(scenario.start));
    Ok(SensitivityRow {
        label: label.to_string(),
        keep_net_position: keep.net_position.unwrap_or(Nok::ZERO),
        sell_now_vs_keep: sell_now.and_then(|r| r.vs_keep),
        sell_now_break_even: sell_now.and_then(|r| r.break_even),
    })
}

/// Shifts the loan's and the fellesgjeld's rate paths (never below 0).
fn shift_rates(s: &mut Scenario, delta: f64) {
    let shift = |r: f64| (r + delta).max(0.0);
    if let Some(l) = &mut s.loan {
        l.nominal_rate = l.nominal_rate.map(shift);
    }
    if let Some(f) = &mut s.fellesgjeld {
        f.nominal_rate = f.nominal_rate.map(shift);
    }
}

fn scale_costs(s: &mut Scenario, factor: f64) {
    let scale = |c: &mut Cost| {
        c.monthly = c.monthly.map(|m| m * factor);
        c.annual = c.annual.map(|a| a * factor);
    };
    // A borettslag's felleskost invoice includes the fellesgjeld payment,
    // which is not a running cost: scale only the rest of it.
    let fellesgjeld_part = crate::keep::fellesgjeld_payment(s) / s.property.ownership_share;
    if let Some(c) = &mut s.costs.felleskostnader {
        let bump = |x: Nok, per_month: f64| x + (x - fellesgjeld_part * per_month) * (factor - 1.0);
        c.monthly = c.monthly.map(|m| bump(m, 1.0));
        c.annual = c.annual.map(|a| bump(a, 12.0));
    }
    let c = &mut s.costs;
    for cost in [
        &mut c.municipal_fees,
        &mut c.property_tax,
        &mut c.insurance,
        &mut c.maintenance,
    ]
    .into_iter()
    .flatten()
    {
        scale(cost);
    }
    for o in &mut c.other {
        o.monthly = o.monthly.map(|m| m * factor);
        o.annual = o.annual.map(|a| a * factor);
    }
}
