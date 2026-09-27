//! Sale outcomes, the tax-free rule and net position: hand-worked examples.

mod common;

use approx::assert_relative_eq;
use common::{loan, simple, ym};
use rent_or_sell_engine::sale::{TaxFreeWindow, tax_free_check, tax_free_window};
use rent_or_sell_engine::scenario::{Improvement, LoanKind};
use rent_or_sell_engine::{Nok, OptionKind, Rate, Scenario, TaxRules, evaluate};

fn rules() -> TaxRules {
    TaxRules::builtin()
}

/// Lived there 2015-2019, rented out since: any gain is taxable.
/// Bought for 3 000 000 + 75 000 costs, 100 000 improvement; 2% broker, 30 000 fixed.
fn taxable(market_value: f64) -> Scenario {
    let mut s = simple(0.0);
    s.property.market_value = Nok(market_value);
    s.property.purchase_date = ym("2015-01");
    s.property.purchase_price = Nok(3_000_000.0);
    s.property.purchase_costs = Nok(75_000.0);
    s.property.improvements = vec![Improvement {
        date: ym("2017-06"),
        amount: Nok(100_000.0),
        description: None,
    }];
    s.owner.moved_out = Some(ym("2020-01"));
    s.sale.broker_rate = Rate(0.02);
    s.sale.fixed_costs = Nok(30_000.0);
    s
}

fn sell_now(s: &Scenario) -> rent_or_sell_engine::SaleOutcome {
    let e = evaluate(s, &rules()).unwrap();
    assert_eq!(e.options[0].kind, OptionKind::SellNow);
    e.options[0].outcome.clone()
}

#[test]
fn last_tax_free_month_after_moving_out() {
    // Lived there from 2019-05; first month not living there 2026-08, so the
    // last full month lived is 2026-07. A sale in S counts full months
    // S−23 ..= S−1; 12 of them must be ≤ 2026-07 → S−23 ≤ 2025-08 → S ≤ 2027-07.
    let mut s = simple(0.0);
    s.property.purchase_date = ym("2019-05");
    s.owner.moved_out = Some(ym("2026-08"));
    s.start = ym("2026-10");
    let r = rules();
    let set = r.for_year(2027);
    let ok = tax_free_check(&s, set, ym("2027-07"));
    assert!(ok.tax_free);
    assert_eq!(ok.lived_months, 12);
    let late = tax_free_check(&s, set, ym("2027-08"));
    assert!(!late.tax_free);
    assert_eq!(late.lived_months, 11);
    assert_eq!(tax_free_window(&s, &r), TaxFreeWindow::Open(ym("2027-07")));

    let e = evaluate(&s, &r).unwrap();
    let kinds: Vec<OptionKind> = e.options.iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        vec![
            OptionKind::SellNow,
            OptionKind::SellLastTaxFree,
            OptionKind::KeepToHorizon
        ]
    );
    assert_eq!(e.options[1].outcome.month, ym("2027-07"));
}

#[test]
fn ownership_must_exceed_the_minimum() {
    // Bought 2025-01 and moved in; renting from 2026-02 (moved out = start).
    // Selling 2026-02: owned 13 months (> 12), lived 2025-02 ..= 2026-01 = 12.
    let mut s = simple(0.0);
    s.property.purchase_date = ym("2025-01");
    s.start = ym("2026-02");
    let mut set = rules().for_year(2026).clone();
    let c = tax_free_check(&s, &set, ym("2026-02"));
    assert_eq!((c.owned_months, c.lived_months, c.tax_free), (13, 12, true));

    // The threshold is a parameter: raise it and the same sale is taxable.
    set.sale_min_ownership_months.value = 13;
    assert!(!tax_free_check(&s, &set, ym("2026-02")).tax_free);
}

#[test]
fn closed_and_never_windows() {
    // Last full month lived 2019-12 → S−23 ≤ 2019-01 → S ≤ 2020-12.
    let s = taxable(4_000_000.0);
    assert_eq!(
        tax_free_window(&s, &rules()),
        TaxFreeWindow::Closed(ym("2020-12"))
    );
    let e = evaluate(&s, &rules()).unwrap();
    assert!(
        e.options
            .iter()
            .all(|o| o.kind != OptionKind::SellLastTaxFree)
    );
    assert!(e.notes.iter().any(|n| n.contains("closed after 2020-12")));

    let mut never = simple(0.0);
    never.owner.moved_in = Some(ym("2026-01"));
    never.owner.moved_out = Some(ym("2026-01"));
    assert_eq!(tax_free_window(&never, &rules()), TaxFreeWindow::Never);
}

#[test]
fn taxable_gain() {
    // Price 5 000 000; costs 2% = 100 000 + 30 000 = 130 000.
    // Entry 3 000 000 + 75 000 + 100 000 = 3 175 000.
    // Gain 5 000 000 − 130 000 − 3 175 000 = 1 695 000 → tax 22% = 372 900.
    // No loan: equity = 5 000 000 − 130 000 − 372 900 = 4 497 100.
    let o = sell_now(&taxable(5_000_000.0));
    assert!(!o.tax_free.tax_free);
    assert_relative_eq!(o.sale_costs.0, 130_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.entry_value.0, 3_175_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.gain.0, 1_695_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.gain_tax.0, 372_900.0, epsilon = 1e-6);
    assert_relative_eq!(o.equity.0, 4_497_100.0, epsilon = 1e-6);
    assert_eq!(o.cumulative_cash, Nok::ZERO);
    assert_relative_eq!(o.net_position.0, 4_497_100.0, epsilon = 1e-6);
}

#[test]
fn taxable_loss_is_deductible() {
    // Price 2 900 000; costs 58 000 + 30 000 = 88 000.
    // Gain 2 900 000 − 88 000 − 3 175 000 = −363 000 → tax −79 860 (a deduction).
    let o = sell_now(&taxable(2_900_000.0));
    assert_relative_eq!(o.gain.0, -363_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.gain_tax.0, -79_860.0, epsilon = 1e-6);
    assert_relative_eq!(
        o.equity.0,
        2_900_000.0 - 88_000.0 + 79_860.0,
        epsilon = 1e-6
    );
}

#[test]
fn tax_free_loss_is_not_deductible() {
    // simple(): lived there until start, so selling now is tax-free.
    let mut s = simple(0.0);
    s.property.market_value = Nok(3_500_000.0);
    let o = sell_now(&s);
    assert!(o.tax_free.tax_free);
    assert!(o.gain.0 < 0.0);
    assert_eq!(o.gain_tax, Nok::ZERO);
}

#[test]
fn ownership_share_scales_property_but_not_loan() {
    // Half of a property worth 6 000 000 with a 1 000 000 loan of your own.
    // Sell now (tax-free): 3 000 000 − 1.5% × 3 000 000 − 20 000 × 0.5 − 1 000 000.
    let mut s = simple(20_000.0);
    s.property.market_value = Nok(6_000_000.0);
    s.property.ownership_share = 0.5;
    s.loan = Some(loan(1_000_000.0, LoanKind::Serial, 0.05, "2045-12"));
    s.sale.broker_rate = Rate(0.015);
    s.sale.fixed_costs = Nok(20_000.0);
    let e = evaluate(&s, &rules()).unwrap();
    let o = &e.options[0].outcome;
    assert_relative_eq!(o.price.0, 3_000_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.entry_value.0, 2_000_000.0, epsilon = 1e-6);
    assert_relative_eq!(o.debt_repaid.0, 1_000_000.0, epsilon = 1e-6);
    assert_relative_eq!(
        o.net_position.0,
        3_000_000.0 - 45_000.0 - 10_000.0 - 1_000_000.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(e.keep.months[0].gross_rent.0, 10_000.0, epsilon = 1e-9);
}

#[test]
fn cumulative_cash_uses_partial_year_tax() {
    // Rent 10 000/month, no costs or loan; sell 2026-07 after 6 rented months.
    // Rent 60 000 > 20 000 threshold → tax 13 200; cash = 60 000 − 13 200 = 46 800.
    let mut s = simple(10_000.0);
    s.sale.sell_at = vec![ym("2026-07")];
    let e = evaluate(&s, &rules()).unwrap();
    let o = &e
        .options
        .iter()
        .find(|o| o.kind == OptionKind::SellAt)
        .unwrap()
        .outcome;
    assert_eq!(o.offset, 6);
    assert_relative_eq!(o.cumulative_cash.0, 46_800.0, epsilon = 1e-6);
}

#[test]
fn horizon_option_matches_last_year_end_and_ledger() {
    let s = taxable(5_000_000.0);
    let e = evaluate(&s, &rules()).unwrap();
    let horizon = &e.options.last().unwrap().outcome;
    assert_eq!(horizon, e.year_end.last().unwrap());
    let total: Nok = e.keep.years.iter().map(|y| y.after_tax_cash_flow).sum();
    assert_relative_eq!(horizon.cumulative_cash.0, total.0, epsilon = 1e-6);
}

#[test]
fn invalid_sale_inputs_are_rejected() {
    let mut s = simple(0.0);
    s.sale.sell_at = vec![ym("2030-01")];
    assert!(evaluate(&s, &rules()).is_err(), "sell_at beyond horizon");

    let mut s = simple(0.0);
    s.owner.moved_out = Some(ym("2026-06"));
    assert!(evaluate(&s, &rules()).is_err(), "moved out after start");
}
