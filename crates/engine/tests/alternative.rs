//! Investment account, cash-flow parity, keep-and-prepay and break-even.

mod common;

use approx::assert_relative_eq;
use common::{loan, simple, ym};
use rent_or_sell_engine::alternative::{InvestSpec, run_account};
use rent_or_sell_engine::scenario::{Invest, InvestTax, LoanKind, Savings};
use rent_or_sell_engine::{
    BreakEven, Evaluation, Nok, RowKind, Scenario, TaxRules, TimePath, evaluate,
};

fn rules() -> TaxRules {
    TaxRules::builtin()
}

fn invest(r: f64, tax: InvestTax) -> Option<Invest> {
    Some(Invest {
        annual_return: TimePath::Constant(r),
        tax,
        equity_share: 1.0,
    })
}

fn basic() -> Scenario {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    Scenario::from_toml_str(&toml).unwrap()
}

fn row<'a>(e: &'a Evaluation, label_start: &str) -> &'a rent_or_sell_engine::ComparisonRow {
    e.comparison
        .iter()
        .find(|r| r.label.starts_with(label_start))
        .unwrap()
}

fn rate(b: Option<BreakEven>) -> f64 {
    match b {
        Some(BreakEven::Rate(r)) => r,
        other => panic!("expected a break-even rate, got {other:?}"),
    }
}

#[test]
fn account_growth_and_tax_modes() {
    // 1 000 000 for 12 months from January: 10% a year.
    let s = simple(0.0);
    let run = |tax| {
        let spec = InvestSpec::from_config(&invest(0.10, tax).unwrap(), s.start, 12);
        run_account(
            &s,
            &rules(),
            &spec,
            &[(0, Nok(1_000_000.0))],
            &[0.0; 12],
            12,
        )
    };
    // After tax: 1 100 000.
    assert_relative_eq!(
        run(InvestTax::AfterTax).value.0,
        1_100_000.0,
        epsilon = 1e-6
    );
    // Deferred: gain 100 000 × 22% × 1.72 = 37 840 → 1 062 160.
    let d = run(InvestTax::Deferred);
    assert_relative_eq!(d.liquidation_tax.0, 37_840.0, epsilon = 1e-6);
    assert_relative_eq!(d.value.0, 1_062_160.0, epsilon = 1e-6);
    // Yearly: 22% of the year's 100 000 at year end → 1 078 000.
    let y = run(InvestTax::Yearly);
    assert_relative_eq!(y.yearly_tax.0, 22_000.0, epsilon = 1e-6);
    assert_relative_eq!(y.value.0, 1_078_000.0, epsilon = 1e-6);
}

#[test]
fn sell_now_without_keep_flows_just_compounds_the_proceeds() {
    // No rent, costs or loan: the proceeds (4 000 000, no sale costs) grow 6% in a year.
    let mut s = simple(0.0);
    s.alternative.invest = invest(0.06, InvestTax::AfterTax);
    let e = evaluate(&s, &rules()).unwrap();
    let sell = row(&e, "Sell now");
    assert_relative_eq!(
        sell.account.as_ref().unwrap().value.0,
        4_240_000.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(sell.vs_keep.unwrap().0, 240_000.0, epsilon = 1e-6);
    // Keep's equity is flat, so the proceeds need 0% to match keeping.
    assert!(rate(sell.break_even).abs() < 1e-9);
}

#[test]
fn parity_at_zero_return_matches_selling_without_investing() {
    // At 0% the account = proceeds − Σ keep's after-tax cash flows after the
    // sale; adding the full cumulative cash leaves proceeds + the cash before
    // the sale, which is the sale's plain net position (equity + cumulative cash).
    // Wealth tax differs between options, so it is switched off here.
    let mut s = basic();
    s.savings = None;
    s.wealth = None;
    s.alternative.invest = invest(0.0, InvestTax::AfterTax);
    let e = evaluate(&s, &rules()).unwrap();
    let sells: Vec<_> = e
        .comparison
        .iter()
        .filter(|r| r.kind == RowKind::Sell)
        .collect();
    assert_eq!(sells.len(), 3);
    for r in sells {
        let sale = e
            .options
            .iter()
            .find(|o| Some(o.outcome.month) == r.sale_month)
            .unwrap();
        assert_relative_eq!(
            r.net_position.unwrap().0,
            sale.outcome.net_position.0,
            epsilon = 1e-4
        );
    }
}

#[test]
fn break_even_return_reproduces_keep() {
    let base = evaluate(&basic(), &rules()).unwrap();
    for label in ["Sell now", "Sell last tax-free", "Sell 2030-01"] {
        let r = rate(row(&base, label).break_even);
        let mut s = basic();
        s.savings = None; // break-even leaves savings out
        s.alternative.invest = invest(r, InvestTax::AfterTax);
        let e = evaluate(&s, &rules()).unwrap();
        assert!(row(&e, label).vs_keep.unwrap().0.abs() < 1.0, "{label}");
    }
    // The yearly column's last entry is the sell-now break-even at the horizon.
    assert_eq!(
        base.year_end_break_even.last().copied(),
        row(&base, "Sell now").break_even
    );
}

#[test]
fn prepay_break_even_reproduces_keep_and_splits_the_decision() {
    let base = evaluate(&basic(), &rules()).unwrap();
    let r = rate(row(&base, "Rent, prepay").break_even);
    let vs_keep = |ret: f64| {
        let mut s = basic();
        s.alternative.invest = invest(ret, InvestTax::AfterTax);
        let e = evaluate(&s, &rules()).unwrap();
        row(&e, "Rent, prepay").vs_keep.unwrap().0
    };
    assert!(vs_keep(r).abs() < 1.0);
    assert!(vs_keep(r - 0.01) > 0.0, "below break-even, prepaying wins");
    assert!(vs_keep(r + 0.01) < 0.0, "above break-even, investing wins");
}

#[test]
fn prepayment_beyond_the_balance_is_invested() {
    // 100 000 loan, 300 000 savings: 100 000 repays the loan, 200 000 is invested.
    let mut s = simple(10_000.0);
    s.loan = Some(loan(100_000.0, LoanKind::Annuity, 0.05, "2040-12"));
    s.savings = Some(Savings {
        amount: Nok(300_000.0),
        at: None,
    });
    s.alternative.invest = invest(0.0, InvestTax::AfterTax);
    let e = evaluate(&s, &rules()).unwrap();
    let prepay = row(&e, "Rent, prepay");
    assert_relative_eq!(
        prepay.account.as_ref().unwrap().deposits.0,
        200_000.0,
        epsilon = 1e-6
    );
    // The loan is gone: equity = full value, no interest afterwards.
    assert_relative_eq!(prepay.property_equity.0, 4_000_000.0, epsilon = 1e-6);
}

#[test]
fn without_invest_only_break_even_is_shown() {
    let mut s = basic();
    s.savings = None;
    s.alternative.invest = None;
    let e = evaluate(&s, &rules()).unwrap();
    let sell = row(&e, "Sell now");
    assert!(sell.net_position.is_none());
    assert!(sell.break_even.is_some());
    assert!(e.comparison.iter().all(|r| r.kind != RowKind::KeepPrepay));
}

#[test]
fn savings_require_an_investment_and_a_date_in_the_horizon() {
    let mut s = simple(0.0);
    s.savings = Some(Savings {
        amount: Nok(1.0),
        at: None,
    });
    assert!(evaluate(&s, &rules()).is_err());
    s.alternative.invest = invest(0.05, InvestTax::AfterTax);
    assert!(evaluate(&s, &rules()).is_ok());
    s.savings.as_mut().unwrap().at = Some(ym("2027-01"));
    assert!(evaluate(&s, &rules()).is_err());
}

#[test]
fn after_tax_equivalent_return() {
    // After-tax input: the return itself.
    let mut s = simple(0.0);
    s.horizon_years = 12;
    s.alternative.invest = invest(0.05, InvestTax::AfterTax);
    let r = evaluate(&s, &rules())
        .unwrap()
        .invest_after_tax_return
        .unwrap();
    assert!((r - 0.05).abs() < 1e-12);
    // Deferred, 6% for 12 years: 1.06^12 − 1 = 1.0122 gain, taxed at 37.84%.
    s.alternative.invest = invest(0.06, InvestTax::Deferred);
    let r = evaluate(&s, &rules())
        .unwrap()
        .invest_after_tax_return
        .unwrap();
    let expected = (1.0 + (1.06_f64.powi(12) - 1.0) * (1.0 - 0.22 * 1.72)).powf(1.0 / 12.0) - 1.0;
    assert!((r - expected).abs() < 1e-9, "{r} vs {expected}");
    // Yearly: 6% × 0.78 = 4.68% every year.
    s.alternative.invest = invest(0.06, InvestTax::Yearly);
    let r = evaluate(&s, &rules())
        .unwrap()
        .invest_after_tax_return
        .unwrap();
    assert!((r - 0.0468).abs() < 1e-3, "{r}");
}

#[test]
fn headline_end_values_differ_by_the_sell_now_gap() {
    let e = evaluate(&basic(), &rules()).unwrap();
    let h = rent_or_sell_engine::headline(&basic(), &e);
    let gap = h.keep_end - h.sell_end.unwrap();
    assert!((gap + row(&e, "Sell now").vs_keep.unwrap().0).abs() < 1e-6);
    assert_eq!(h.gap, Some(gap));
}

#[test]
fn loan_paid_off_is_the_final_payment_within_the_horizon() {
    // The example's loan runs to 2051-09, past its 10-year horizon.
    let e = evaluate(&basic(), &rules()).unwrap();
    assert!(
        rent_or_sell_engine::headline(&basic(), &e)
            .loan_paid_off
            .is_none()
    );
    let mut s = basic();
    s.loan.as_mut().unwrap().end = ym("2030-06");
    let e = evaluate(&s, &rules()).unwrap();
    let paid = rent_or_sell_engine::headline(&s, &e).loan_paid_off.unwrap();
    assert_eq!(paid.month, "2030-06");
    assert_eq!(paid.offset, 45); // 2026-10 .. 2030-06 is 45 months
}
