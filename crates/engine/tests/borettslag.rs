//! Borettslag: fellesgjeld in cash flow, rental tax, sale, gain and wealth tax.

mod common;

use approx::assert_relative_eq;
use common::{simple, ym};
use rent_or_sell_engine::scenario::{Fellesgjeld, LoanKind, PropertyKind, Wealth};
use rent_or_sell_engine::{Nok, OptionKind, Scenario, TaxRules, TimePath, evaluate};

fn rules() -> TaxRules {
    TaxRules::builtin()
}

fn fellesgjeld(balance: f64, rate: f64, end: &str) -> Fellesgjeld {
    Fellesgjeld {
        balance: Nok(balance),
        at_purchase: None,
        kind: LoanKind::Serial,
        end: ym(end),
        nominal_rate: TimePath::Constant(rate),
        interest_only_until: None,
    }
}

/// `simple` as a borettslag unit with the given fellesgjeld.
fn borettslag(rent: f64, fg: Fellesgjeld) -> Scenario {
    let mut s = simple(rent);
    s.property.kind = PropertyKind::Borettslag;
    s.fellesgjeld = Some(fg);
    s
}

#[test]
fn fellesgjeld_interest_is_deductible_and_principal_is_not() {
    // 1.2M serial over 120 months at 6%: principal 10 000/month, interest
    // 0.005 × Σ(1.2M − 10 000·m) for m = 0..11 = 68 700.
    let s = borettslag(20_000.0, fellesgjeld(1_200_000.0, 0.06, "2035-12"));
    let y = &evaluate(&s, &rules()).unwrap().keep.years[0];
    assert_relative_eq!(y.fellesgjeld_interest.0, 68_700.0, epsilon = 1e-6);
    assert_relative_eq!(y.fellesgjeld_principal.0, 120_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.fellesgjeld_balance_end.0, 1_080_000.0, epsilon = 1e-6);
    assert_relative_eq!(
        y.pre_tax_cash_flow.0,
        240_000.0 - 68_700.0 - 120_000.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(y.tax.0, (240_000.0 - 68_700.0) * 0.22, epsilon = 1e-6);
}

#[test]
fn fellesgjeld_interest_is_deducted_even_when_rent_is_tax_free() {
    // 12 000 kr rent a year is under the 20 000 threshold.
    let s = borettslag(1_000.0, fellesgjeld(1_200_000.0, 0.06, "2035-12"));
    let y = &evaluate(&s, &rules()).unwrap().keep.years[0];
    assert!(!y.rental_income_taxable);
    assert_relative_eq!(y.tax.0, -68_700.0 * 0.22, epsilon = 1e-6);
}

#[test]
fn sale_nets_the_fellesgjeld_and_corrects_the_gain_for_what_was_paid_down() {
    // Bought 2020-01 for 3.8M + 1.2M fellesgjeld; total price now 5.5M.
    // Sold at the horizon (2027-01) with 1.08M fellesgjeld left, not tax-free.
    // Gain = 5.5M − (3.8M + 1.2M) = 500 000 (on prices excluding fellesgjeld:
    // 4.42M − 3.8M = 620 000, minus the 120 000 paid down).
    let mut s = borettslag(0.0, fellesgjeld(1_200_000.0, 0.0, "2035-12"));
    s.property.market_value = Nok(5_500_000.0);
    s.property.purchase_price = Nok(3_800_000.0);
    let e = evaluate(&s, &rules()).unwrap();
    let o = &e
        .options
        .iter()
        .find(|o| o.kind == OptionKind::KeepToHorizon)
        .unwrap()
        .outcome;
    assert!(!o.tax_free.tax_free);
    assert_relative_eq!(o.entry_value.0, 5_000_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.gain.0, 500_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.fellesgjeld.0, 1_080_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.gain_tax.0, 110_000.0, epsilon = 1e-6);
    assert_relative_eq!(
        o.equity.0,
        5_500_000.0 - 1_080_000.0 - 110_000.0,
        epsilon = 1e-6
    );
    // Selling now: the buyer takes over the full 1.2M.
    let now = &e.options[0].outcome;
    assert_relative_eq!(now.equity.0, 5_500_000.0 - 1_200_000.0, epsilon = 1e-6);
}

#[test]
fn fellesgjeld_bought_higher_than_today_raises_the_entry_value() {
    let mut fg = fellesgjeld(1_000_000.0, 0.0, "2035-12");
    fg.at_purchase = Some(Nok(1_300_000.0));
    let s = borettslag(0.0, fg);
    let e = evaluate(&s, &rules()).unwrap();
    // 4M purchase price + 1.3M fellesgjeld at purchase.
    assert_relative_eq!(
        e.options[0].outcome.entry_value.0,
        5_300_000.0,
        epsilon = 1e-6
    );
}

#[test]
fn zero_fellesgjeld_borettslag_matches_selveier() {
    let mut selveier = simple(15_000.0);
    selveier.horizon_years = 3;
    selveier.loan = Some(common::loan(
        2_000_000.0,
        LoanKind::Annuity,
        0.05,
        "2050-12",
    ));
    selveier.alternative.invest = Some(rent_or_sell_engine::scenario::Invest {
        annual_return: TimePath::Constant(0.05),
        tax: rent_or_sell_engine::scenario::InvestTax::Deferred,
        equity_share: 1.0,
    });
    let mut b = selveier.clone();
    b.property.kind = PropertyKind::Borettslag;
    b.fellesgjeld = Some(fellesgjeld(0.0, 0.05, "2040-12"));
    let (x, y) = (
        evaluate(&selveier, &rules()).unwrap(),
        evaluate(&b, &rules()).unwrap(),
    );
    for (p, q) in x.options.iter().zip(&y.options) {
        assert_eq!(p.outcome.net_position, q.outcome.net_position);
    }
    for (p, q) in x.comparison.iter().zip(&y.comparison) {
        assert_eq!(p.net_position, q.net_position);
    }
}

#[test]
fn interest_only_fellesgjeld_jumps_when_the_period_ends() {
    let mut fg = fellesgjeld(1_200_000.0, 0.06, "2035-12");
    fg.interest_only_until = Some(ym("2026-06"));
    let s = borettslag(0.0, fg);
    let e = evaluate(&s, &rules()).unwrap();
    let m = &e.keep.months;
    assert_eq!(m[5].fellesgjeld_principal, Nok::ZERO);
    assert_relative_eq!(m[5].fellesgjeld_interest.0, 6_000.0, epsilon = 1e-9);
    // Then serial over the remaining 114 months.
    assert_relative_eq!(
        m[6].fellesgjeld_principal.0,
        1_200_000.0 / 114.0,
        epsilon = 1e-6
    );
}

#[test]
fn fellesgjeld_is_debt_for_wealth_tax() {
    // 4M total price, 1M fellesgjeld (interest-free, repaid 2035-12): at the
    // end of 2026 the fellesgjeld is 1M − 12 × 1M/120 = 900 000.
    // Net 4M − 900 000 = 3.1M → (3.1M − 1.9M) × 1% = 12 000.
    let mut s = borettslag(0.0, fellesgjeld(1_000_000.0, 0.0, "2035-12"));
    s.wealth = Some(Wealth {
        persons: 1,
        other_net_wealth: TimePath::Constant(0.0),
        assessed_market_value: None,
    });
    let y = &evaluate(&s, &rules()).unwrap().keep.years[0];
    assert_relative_eq!(y.wealth_tax.0, 12_000.0, epsilon = 1e-6);
}

#[test]
fn validation() {
    let mut s = simple(0.0);
    s.property.kind = PropertyKind::Borettslag;
    assert!(
        evaluate(&s, &rules()).is_err(),
        "borettslag needs [fellesgjeld]"
    );
    let mut s = simple(0.0);
    s.fellesgjeld = Some(fellesgjeld(1.0, 0.0, "2035-12"));
    assert!(
        evaluate(&s, &rules()).is_err(),
        "[fellesgjeld] needs borettslag"
    );
    let mut fg = fellesgjeld(1.0, 0.0, "2035-12");
    fg.interest_only_until = Some(ym("2035-12"));
    assert!(evaluate(&borettslag(0.0, fg), &rules()).is_err());
}

#[test]
fn rental_limit_notes() {
    let notes = |s: &Scenario| evaluate(s, &rules()).unwrap().warnings;
    let has = |s: &Scenario, what: &str| notes(s).iter().any(|n| n.contains(what));
    let mut s = borettslag(0.0, fellesgjeld(0.0, 0.0, "2035-12"));
    assert!(!has(&s, "Borettslag"));
    s.horizon_years = 4;
    assert!(has(&s, "limited to 3 years"));
    // Never lived there (moved in and out at purchase).
    let mut s = borettslag(0.0, fellesgjeld(0.0, 0.0, "2035-12"));
    s.owner.moved_in = Some(ym("2020-01"));
    s.owner.moved_out = Some(ym("2020-01"));
    assert!(has(&s, "at least 12 of the 24 months"));
}

#[test]
fn felleskost_invoice_is_split_into_fellesgjeld_and_operating_costs() {
    // Serial 1.2M at 6%: first month 6 000 interest + 10 000 principal.
    // Invoice 20 000 → 4 000 operating; in month one the two add back up.
    let mut s = borettslag(0.0, fellesgjeld(1_200_000.0, 0.06, "2035-12"));
    s.costs.felleskostnader = common::monthly_cost(20_000.0, true);
    let e = evaluate(&s, &rules()).unwrap();
    assert_relative_eq!(e.keep.cost_lines[0].monthly_base.0, 4_000.0, epsilon = 1e-6);
    let m = &e.keep.months[0];
    let billed = m.costs[0] + m.fellesgjeld_interest + m.fellesgjeld_principal;
    assert_relative_eq!(billed.0, 20_000.0, epsilon = 1e-6);
    assert!(e.notes.iter().any(|n| n.contains("invoice split")));
    // With half the unit, the invoice is for the whole unit and the
    // fellesgjeld is your share: 20 000 − 16 000 / 0.5 < 0 → error.
    s.property.ownership_share = 0.5;
    assert!(evaluate(&s, &rules()).is_err());
}

#[test]
fn an_invoice_smaller_than_the_fellesgjeld_payment_is_an_error() {
    let mut s = borettslag(0.0, fellesgjeld(1_200_000.0, 0.06, "2035-12"));
    s.costs.felleskostnader = common::monthly_cost(15_000.0, true);
    let err = evaluate(&s, &rules()).unwrap_err().to_string();
    assert!(err.contains("whole monthly invoice"), "{err}");
}

#[test]
fn cost_bump_scales_only_the_operating_part_of_the_invoice() {
    // Operating part 4 000/month → +20% = +800/month = 9 600 a year,
    // deductible, so keep's net position falls by 9 600 × 0.78.
    let mut s = borettslag(20_000.0, fellesgjeld(1_200_000.0, 0.06, "2035-12"));
    s.costs.felleskostnader = common::monthly_cost(20_000.0, true);
    let rows = rent_or_sell_engine::sensitivity(&s, &rules()).unwrap();
    let bumped = rows
        .iter()
        .find(|r| r.label == "Running costs +20%")
        .unwrap();
    let delta = bumped.keep_net_position.0 - rows[0].keep_net_position.0;
    assert_relative_eq!(delta, -9_600.0 * 0.78, epsilon = 1e-6);
}
