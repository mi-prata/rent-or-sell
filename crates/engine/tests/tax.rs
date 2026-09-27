//! Hand-worked tax examples.

mod common;

use approx::assert_relative_eq;
use common::{loan, monthly_cost, simple};
use rent_or_sell_engine::scenario::LoanKind;
use rent_or_sell_engine::{Nok, TaxRules, run_keep};

#[test]
fn net_rental_income_taxed_at_22_percent() {
    // Rent 10 000/month, deductible costs 2 000/month, non-deductible 500/month, no loan.
    // Taxable: 12 × (10 000 − 2 000) = 96 000 → tax 22% = 21 120.
    // Cash flow: 12 × (10 000 − 2 500) = 90 000 → after tax 68 880.
    let mut s = simple(10_000.0);
    s.costs.maintenance = monthly_cost(2_000.0, true);
    s.costs.insurance = monthly_cost(500.0, false);
    let y = &run_keep(&s, &TaxRules::builtin()).unwrap().years[0];
    assert_eq!(y.months, 12);
    assert_relative_eq!(y.taxable_rental_result.0, 96_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.tax.0, 21_120.0, epsilon = 1e-6);
    assert_relative_eq!(y.pre_tax_cash_flow.0, 90_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.after_tax_cash_flow.0, 68_880.0, epsilon = 1e-6);
    assert_eq!(y.avg_monthly_out_of_pocket, Nok::ZERO);
}

#[test]
fn interest_deduction_can_make_tax_negative() {
    // Interest-heavy year: 2 400 000 serial loan at 6% ending far in the future.
    // Rent 12 000/month, no costs. January interest = 2 400 000 × 0.005 = 12 000,
    // and interest falls as principal is repaid, so the year's interest is
    // Σ 0.005 × (B − k·P) for k = 0..11, with P = B / remaining months.
    let mut s = simple(12_000.0);
    s.loan = Some(loan(2_400_000.0, LoanKind::Serial, 0.06, "2045-12")); // 240 payments
    let y = &run_keep(&s, &TaxRules::builtin()).unwrap().years[0];
    let p = 2_400_000.0 / 240.0;
    let interest: f64 = (0..12).map(|k| 0.005 * (2_400_000.0 - k as f64 * p)).sum();
    assert_relative_eq!(y.interest.0, interest, epsilon = 1e-6);
    assert_relative_eq!(y.tax.0, 0.22 * (144_000.0 - interest), epsilon = 1e-6);
    assert!(y.tax.0 > 0.0);

    s.rental.monthly_rent = Nok(8_000.0);
    let y = &run_keep(&s, &TaxRules::builtin()).unwrap().years[0];
    assert_relative_eq!(y.tax.0, 0.22 * (96_000.0 - interest), epsilon = 1e-6);
    assert!(y.tax.0 < 0.0, "a deficit is a tax saving");
    assert!(y.avg_monthly_out_of_pocket.0 > 0.0);
}

#[test]
fn rent_within_threshold_is_tax_free_and_costs_not_deducted() {
    // 1 500 × 12 = 18 000 ≤ 20 000: untaxed, and costs are not deductible.
    let mut s = simple(1_500.0);
    s.costs.maintenance = monthly_cost(1_000.0, true);
    let r = run_keep(&s, &TaxRules::builtin()).unwrap();
    let y = &r.years[0];
    assert!(!y.rental_income_taxable);
    assert_eq!(y.tax, Nok::ZERO);
    assert!(r.notes.iter().any(|n| n.contains("tax-free threshold")));
}

#[test]
fn partial_calendar_years_and_rule_fallback_note() {
    let mut s = simple(10_000.0);
    s.start = common::ym("2026-10");
    s.horizon_years = 2;
    let r = run_keep(&s, &TaxRules::builtin()).unwrap();
    let months: Vec<u32> = r.years.iter().map(|y| y.months).collect();
    assert_eq!(months, vec![3, 12, 9]);
    assert!(r.years.iter().all(|y| y.rules_year == 2026));
    assert!(r.notes.iter().any(|n| n.contains("2027–2028")));
}
