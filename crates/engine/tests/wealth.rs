//! Wealth tax: hand-worked keep ledger and comparison rows.

mod common;

use approx::assert_relative_eq;
use common::{loan, simple};
use proptest::prelude::*;
use rent_or_sell_engine::scenario::{Invest, InvestTax, LoanKind, Wealth};
use rent_or_sell_engine::{Evaluation, Nok, Scenario, TaxRules, TimePath, evaluate};

fn rules() -> TaxRules {
    TaxRules::builtin()
}

fn wealth(other: f64, persons: u32) -> Option<Wealth> {
    Some(Wealth {
        persons,
        other_net_wealth: TimePath::Constant(other),
        assessed_market_value: None,
    })
}

/// `simple` (4M, no loan, no rent, 2026-01 for 12 months) with a 0% after-tax
/// investment of the given equity share.
fn scenario(other: f64, persons: u32, equity_share: f64) -> Scenario {
    let mut s = simple(0.0);
    s.wealth = wealth(other, persons);
    s.alternative.invest = Some(Invest {
        annual_return: TimePath::Constant(0.0),
        tax: InvestTax::AfterTax,
        equity_share,
    });
    s
}

fn keep_wealth_tax(s: &Scenario) -> f64 {
    evaluate(s, &rules()).unwrap().keep.years[0].wealth_tax.0
}

fn row<'a>(e: &'a Evaluation, label_start: &str) -> &'a rent_or_sell_engine::ComparisonRow {
    e.comparison
        .iter()
        .find(|r| r.label.starts_with(label_start))
        .unwrap()
}

#[test]
fn keep_ledger_wealth_tax_is_marginal() {
    // Secondary home 4M at 100%, no debt.
    // Other 0: (4M − 1.9M) × 1% = 21 000.
    assert_relative_eq!(
        keep_wealth_tax(&scenario(0.0, 1, 1.0)),
        21_000.0,
        epsilon = 1e-6
    );
    // Other 1M: tax(5M) − tax(1M) = 31 000 − 0.
    assert_relative_eq!(
        keep_wealth_tax(&scenario(1e6, 1, 1.0)),
        31_000.0,
        epsilon = 1e-6
    );
    // Other 3M, already above the threshold: the full 1% of 4M.
    assert_relative_eq!(
        keep_wealth_tax(&scenario(3e6, 1, 1.0)),
        40_000.0,
        epsilon = 1e-6
    );
    // Spouses: (4M − 3.8M) × 1% = 2 000.
    assert_relative_eq!(
        keep_wealth_tax(&scenario(0.0, 2, 1.0)),
        2_000.0,
        epsilon = 1e-6
    );
}

#[test]
fn wealth_tax_is_paid_in_the_after_tax_cash_flow() {
    let e = evaluate(&scenario(0.0, 1, 1.0), &rules()).unwrap();
    let y = &e.keep.years[0];
    assert_relative_eq!(
        y.after_tax_cash_flow.0,
        (y.pre_tax_cash_flow - y.tax - y.wealth_tax).0,
        epsilon = 1e-9
    );
    assert_relative_eq!(y.taxable_wealth.0, 4_000_000.0, epsilon = 1e-6);
    // Only December carries it.
    let months_with_tax = e
        .keep
        .months
        .iter()
        .filter(|m| m.wealth_tax.0 != 0.0)
        .count();
    assert_eq!(months_with_tax, 1);
}

#[test]
fn loan_is_deducted_and_the_assessed_value_is_used() {
    // Assessed 3M instead of the 4M market value; 1M interest-free loan
    // repaid over 100 months → balance 880 000 after December.
    let mut s = scenario(0.0, 1, 1.0);
    s.wealth.as_mut().unwrap().assessed_market_value = Some(Nok(3_000_000.0));
    s.loan = Some(loan(1_000_000.0, LoanKind::Serial, 0.0, "2034-04"));
    let e = evaluate(&s, &rules()).unwrap();
    let y = &e.keep.years[0];
    assert_relative_eq!(y.loan_balance_end.0, 880_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.taxable_wealth.0, 2_120_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.wealth_tax.0, 2_200.0, epsilon = 1e-6);
}

#[test]
fn sell_now_pays_wealth_tax_on_the_discounted_investment() {
    // Sold at once: 4M in an equity fund, valued at 80% = 3.2M.
    // Sell row: (3.2M − 1.9M) × 1% = 13 000; keep: 21 000 from your pocket.
    // The account pays 13 000 − 21 000 = −8 000 (i.e. gets 8 000 back).
    let e = evaluate(&scenario(0.0, 1, 1.0), &rules()).unwrap();
    let sell = row(&e, "Sell now");
    let account = sell.account.as_ref().unwrap();
    assert_relative_eq!(account.wealth_tax.0, 13_000.0, epsilon = 1e-6);
    assert_relative_eq!(account.keep_wealth_tax.0, 21_000.0, epsilon = 1e-6);
    assert_relative_eq!(account.value.0, 4_008_000.0, epsilon = 1e-6);
    assert_relative_eq!(sell.cumulative_cash.0, -21_000.0, epsilon = 1e-6);
    assert_relative_eq!(sell.vs_keep.unwrap().0, 8_000.0, epsilon = 1e-6);
    assert_relative_eq!(sell.wealth_tax.unwrap().0, 13_000.0, epsilon = 1e-6);
    assert_relative_eq!(
        row(&e, "Rent").wealth_tax.unwrap().0,
        21_000.0,
        epsilon = 1e-6
    );

    // In a bank account (no discount), the sell row's wealth tax equals keep's.
    let e = evaluate(&scenario(0.0, 1, 0.0), &rules()).unwrap();
    assert!(row(&e, "Sell now").vs_keep.unwrap().0.abs() < 1e-6);
}

#[test]
fn debt_reduction_applies_when_keeping_with_invested_savings() {
    // Keep with a 2M interest-free loan (balance 1.76M after December) and
    // 1M savings in an equity fund: gross assets 5M, share discount 200 000,
    // debt reduction = 1.76M × 200 000 / 5M = 70 400.
    // Net = 4M + 800 000 − (1.76M − 70 400) = 3 110 400 → tax 12 104.
    // Plain keep (no savings): 4M − 1.76M = 2.24M → tax 3 400.
    let mut s = scenario(0.0, 1, 1.0);
    s.loan = Some(loan(2_000_000.0, LoanKind::Serial, 0.0, "2034-04"));
    s.savings = Some(rent_or_sell_engine::scenario::Savings {
        amount: Nok(1_000_000.0),
        at: None,
    });
    let e = evaluate(&s, &rules()).unwrap();
    assert_relative_eq!(e.keep.years[0].wealth_tax.0, 3_400.0, epsilon = 1e-6);
    let keep = row(&e, "Rent (savings invested)");
    let account = keep.account.as_ref().unwrap();
    assert_relative_eq!(account.wealth_tax.0, 12_104.0, epsilon = 1e-6);
    assert_relative_eq!(
        account.value.0,
        1_000_000.0 - (12_104.0 - 3_400.0),
        epsilon = 1e-6
    );
}

#[test]
fn below_the_threshold_wealth_tax_changes_nothing() {
    // A 1M property and 1M in savings never reach the 1.9M threshold.
    let mut with = scenario(0.0, 1, 1.0);
    with.property.market_value = Nok(1_000_000.0);
    with.property.purchase_price = Nok(1_000_000.0);
    with.savings = Some(rent_or_sell_engine::scenario::Savings {
        amount: Nok(800_000.0),
        at: None,
    });
    let mut without = with.clone();
    without.wealth = None;
    let (a, b) = (
        evaluate(&with, &rules()).unwrap(),
        evaluate(&without, &rules()).unwrap(),
    );
    for (x, y) in a.comparison.iter().zip(&b.comparison) {
        assert_eq!(x.net_position, y.net_position, "{}", x.label);
        assert_eq!(x.break_even, y.break_even, "{}", x.label);
    }
}

#[test]
fn no_wealth_tax_for_a_horizon_ending_before_december() {
    let mut s = scenario(0.0, 1, 1.0);
    s.start = common::ym("2026-12");
    s.property.purchase_date = common::ym("2020-01");
    // 2026-12 is inside the horizon, 2027-12 is not (it ends 2027-11).
    let e = evaluate(&s, &rules()).unwrap();
    let taxed: Vec<i32> = e
        .keep
        .years
        .iter()
        .filter(|y| y.wealth_tax.0 != 0.0)
        .map(|y| y.year)
        .collect();
    assert_eq!(taxed, vec![2026]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// The marginal wealth tax never falls as other wealth or the property's
    /// value rises (the tax schedule is convex).
    #[test]
    fn marginal_wealth_tax_is_monotone(
        other in 0.0..30e6_f64,
        extra in 0.0..10e6_f64,
        value in 0.0..30e6_f64,
        bump in 0.0..5e6_f64,
    ) {
        let at = |other: f64, value: f64| {
            let mut s = scenario(other, 1, 1.0);
            s.property.market_value = Nok(value);
            keep_wealth_tax(&s)
        };
        prop_assert!(at(other + extra, value) >= at(other, value) - 1e-6);
        prop_assert!(at(other, value + bump) >= at(other, value) - 1e-6);
    }

    /// Selling into a bank account never pays less wealth tax than into an
    /// equity fund (the share discount only helps).
    #[test]
    fn equity_share_lowers_wealth_tax(other in 0.0..30e6_f64, equity in 0.0..=1.0_f64) {
        let tax = |share: f64| {
            let e = evaluate(&scenario(other, 1, share), &rules()).unwrap();
            row(&e, "Sell now").wealth_tax.unwrap().0
        };
        prop_assert!(tax(equity) <= tax(0.0) + 1e-6);
        prop_assert!(tax(1.0) <= tax(equity) + 1e-6);
    }
}
