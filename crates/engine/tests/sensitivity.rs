//! One-at-a-time sensitivity runs.

mod common;

use approx::assert_relative_eq;
use common::{loan, monthly_cost, simple};
use rent_or_sell_engine::scenario::LoanKind;
use rent_or_sell_engine::{Scenario, SensitivityRow, TaxRules, evaluate, sensitivity};

fn basic() -> Scenario {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    Scenario::from_toml_str(&toml).unwrap()
}

fn row<'a>(rows: &'a [SensitivityRow], label: &str) -> &'a SensitivityRow {
    rows.iter().find(|r| r.label == label).unwrap()
}

#[test]
fn base_row_is_the_plain_evaluation() {
    let s = basic();
    let rows = sensitivity(&s, &TaxRules::builtin()).unwrap();
    let e = evaluate(&s, &TaxRules::builtin()).unwrap();
    assert_eq!(rows[0].label, "Base");
    assert_eq!(
        Some(rows[0].keep_net_position),
        e.comparison[0].net_position
    );
    let sell_now = e
        .comparison
        .iter()
        .find(|r| r.label.starts_with("Sell now"))
        .unwrap();
    assert_eq!(rows[0].sell_now_vs_keep, sell_now.vs_keep);
    assert_eq!(rows[0].sell_now_break_even, sell_now.break_even);
    assert_eq!(rows.len(), 9);
}

#[test]
fn bumps_move_keeping_the_expected_way() {
    let rows = sensitivity(&basic(), &TaxRules::builtin()).unwrap();
    let keep = |l: &str| row(&rows, l).keep_net_position.0;
    let base = keep("Base");
    assert!(keep("Interest rate +1 pp") < base);
    assert!(keep("Interest rate −1 pp") > base);
    assert!(keep("Price growth +1 pp") > base);
    assert!(keep("Price growth −1 pp") < base);
    assert!(keep("Rent +10%") > base);
    assert!(keep("Rent −10%") < base);
    assert!(keep("Vacancy +1 month/yr") < base);
    assert!(keep("Running costs +20%") < base);
}

#[test]
fn rent_and_cost_bumps_are_exact_in_a_simple_case() {
    // One year, no loan, 20 000 rent and 5 000 deductible costs a month,
    // no price growth: keep's net position moves by the after-tax cash flow.
    let mut s = simple(20_000.0);
    s.costs.felleskostnader = monthly_cost(5_000.0, true);
    let rows = sensitivity(&s, &TaxRules::builtin()).unwrap();
    let delta = |l: &str| row(&rows, l).keep_net_position.0 - rows[0].keep_net_position.0;
    assert_relative_eq!(delta("Rent +10%"), 24_000.0 * 0.78, epsilon = 1e-6);
    assert_relative_eq!(
        delta("Running costs +20%"),
        -12_000.0 * 0.78,
        epsilon = 1e-6
    );
    assert_relative_eq!(
        delta("Vacancy +1 month/yr"),
        -20_000.0 * 0.78,
        epsilon = 1e-6
    );
    // No loan: rate bumps change nothing.
    assert_eq!(delta("Interest rate +1 pp"), 0.0);
}

#[test]
fn rates_never_go_below_zero() {
    let mut s = simple(0.0);
    s.loan = Some(loan(1_000_000.0, LoanKind::Serial, 0.005, "2040-12"));
    // −1 pp would be −0.5%; it is floored at 0, so the run succeeds.
    let rows = sensitivity(&s, &TaxRules::builtin()).unwrap();
    assert!(row(&rows, "Interest rate −1 pp").keep_net_position > rows[0].keep_net_position);
}
