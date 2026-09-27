//! Invariants that must hold for any valid input.

mod common;

use common::{loan, simple, ym};
use proptest::prelude::*;
use rent_or_sell_engine::loan::schedule;
use rent_or_sell_engine::scenario::{Loan, LoanKind};
use rent_or_sell_engine::timepath::{Point, TimePath};
use rent_or_sell_engine::{Nok, TaxRules, run_keep};

fn kind() -> impl Strategy<Value = LoanKind> {
    prop_oneof![Just(LoanKind::Annuity), Just(LoanKind::Serial)]
}

/// A loan starting 2026-01 with 1–40 years left, a rate that may change once,
/// and (half the time) an interest-only period.
fn any_loan() -> impl Strategy<Value = Loan> {
    (
        1_000.0..20_000_000.0f64,
        kind(),
        1..=40i64,
        0.0..0.15f64,
        0.0..0.15f64,
        1..480i64,
        (any::<bool>(), 0..120i64),
    )
        .prop_map(|(balance, kind, years, r1, r2, change, (io, io_months))| {
            let start = ym("2026-01");
            let last = years * 12 - 1;
            let interest_only_until = (io && io_months < last).then(|| start.add_months(io_months));
            Loan {
                balance: Nok(balance),
                kind,
                end: start.add_months(years * 12 - 1),
                monthly_fee: Nok(0.0),
                nominal_rate: TimePath::Steps(vec![
                    Point {
                        from: start,
                        value: r1,
                    },
                    Point {
                        from: start.add_months(change),
                        value: r2,
                    },
                ]),
                interest_only_until,
            }
        })
}

proptest! {
    #[test]
    fn balance_never_negative_and_principal_repays_loan(l in any_loan()) {
        let rows = schedule(&l, ym("2026-01"), 40 * 12 + 6);
        for r in &rows {
            prop_assert!(r.closing_balance.0 >= 0.0);
            prop_assert!(r.principal.0 >= 0.0);
        }
        let repaid: f64 = rows.iter().map(|r| r.principal.0).sum();
        prop_assert!((repaid - l.balance.0).abs() < 1e-6 * l.balance.0.max(1.0));
        prop_assert_eq!(rows.last().unwrap().closing_balance, Nok::ZERO);
    }

    #[test]
    fn without_rent_or_costs_cash_flow_is_minus_debt_service(
        balance in 0.0..10_000_000.0f64, kind in kind(), rate in 0.0..0.12f64
    ) {
        let mut s = simple(0.0);
        s.horizon_years = 3;
        s.loan = Some(loan(balance, kind, rate, "2045-12"));
        s.loan.as_mut().unwrap().monthly_fee = Nok(50.0);
        let r = run_keep(&s, &TaxRules::builtin()).unwrap();
        for m in &r.months {
            let debt_service = m.interest + m.principal + m.loan_fee;
            prop_assert!((m.pre_tax_cash_flow + debt_service).0.abs() < 1e-6);
        }
    }

    /// A higher rate never helps: monthly pre-tax cash flow and each year's
    /// after-tax result excluding principal (the part that is not saved as equity)
    /// are never better.
    #[test]
    fn higher_rate_never_improves_results(
        balance in 1_000.0..10_000_000.0f64,
        kind in kind(),
        rate in 0.0..0.10f64,
        delta in 0.0..0.05f64,
        rent in 0.0..40_000.0f64,
    ) {
        let run = |r: f64| {
            let mut s = simple(rent);
            s.horizon_years = 5;
            s.loan = Some(loan(balance, kind, r, "2050-12"));
            run_keep(&s, &TaxRules::builtin()).unwrap()
        };
        let (low, high) = (run(rate), run(rate + delta));
        for (a, b) in low.months.iter().zip(&high.months) {
            prop_assert!(b.pre_tax_cash_flow.0 <= a.pre_tax_cash_flow.0 + 1e-6);
        }
        for (a, b) in low.years.iter().zip(&high.years) {
            let a_result = a.after_tax_cash_flow + a.principal;
            let b_result = b.after_tax_cash_flow + b.principal;
            prop_assert!(b_result.0 <= a_result.0 + 1e-6);
        }
    }
}

mod sale_properties {
    use super::common::{loan, simple, ym};
    use proptest::prelude::*;
    use rent_or_sell_engine::scenario::LoanKind;
    use rent_or_sell_engine::{Nok, Rate, TaxRules, TimePath, evaluate};

    proptest! {
        /// Faster price growth never lowers the net position of any sale.
        #[test]
        fn net_position_non_decreasing_in_price_growth(
            g in -0.05..0.08f64, dg in 0.0..0.05f64, rent in 0.0..30_000.0f64,
            moved_out_back in 0i64..60,
        ) {
            let run = |growth: f64| {
                let mut s = simple(rent);
                s.horizon_years = 4;
                s.owner.moved_out = Some(ym("2026-01").add_months(-moved_out_back));
                s.property.price_growth = TimePath::Constant(growth);
                s.sale.broker_rate = Rate(0.02);
                s.loan = Some(loan(2_000_000.0, LoanKind::Annuity, 0.05, "2050-12"));
                evaluate(&s, &TaxRules::builtin()).unwrap()
            };
            let (low, high) = (run(g), run(g + dg));
            for (a, b) in low.year_end.iter().zip(&high.year_end) {
                prop_assert!(b.net_position.0 >= a.net_position.0 - 1e-6);
                prop_assert!(!a.tax_free.tax_free || a.gain_tax == Nok::ZERO);
            }
        }

        /// With no rent, costs, interest, fees or growth, paying down the loan
        /// out of pocket moves money from cash to equity: net position before
        /// gain tax is the same whenever you sell.
        #[test]
        fn repaying_debt_out_of_pocket_does_not_change_wealth(
            balance in 0.0..5_000_000.0f64, kind in prop_oneof![Just(LoanKind::Annuity), Just(LoanKind::Serial)],
            fixed in 0.0..100_000.0f64,
        ) {
            let mut s = simple(0.0);
            s.horizon_years = 5;
            s.loan = Some(loan(balance, kind, 0.0, "2040-12"));
            s.sale.fixed_costs = Nok(fixed);
            let e = evaluate(&s, &TaxRules::builtin()).unwrap();
            let now = &e.options[0].outcome;
            let base = now.net_position + now.gain_tax;
            for o in &e.year_end {
                prop_assert!(((o.net_position + o.gain_tax) - base).0.abs() < 1e-4);
            }
        }
    }
}

mod alternative_properties {
    use super::common::{loan, simple};
    use proptest::prelude::*;
    use rent_or_sell_engine::scenario::{Invest, InvestTax, LoanKind};
    use rent_or_sell_engine::{BreakEven, Nok, Rate, RowKind, TaxRules, TimePath, evaluate};

    fn scenario(ret: f64, rent: f64, growth: f64, rate: f64) -> rent_or_sell_engine::Scenario {
        let mut s = simple(rent);
        s.horizon_years = 5;
        s.property.price_growth = TimePath::Constant(growth);
        s.sale.broker_rate = Rate(0.02);
        s.sale.fixed_costs = Nok(30_000.0);
        s.loan = Some(loan(2_500_000.0, LoanKind::Annuity, rate, "2050-12"));
        s.alternative.invest = Some(Invest {
            annual_return: TimePath::Constant(ret),
            tax: InvestTax::AfterTax,
            equity_share: 1.0,
        });
        s
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// A higher return never lowers a sell option's net position.
        #[test]
        fn sell_outcome_non_decreasing_in_return(
            ret in -0.05..0.12f64, d in 0.0..0.05f64, rent in 0.0..30_000.0f64,
        ) {
            let run = |r: f64| evaluate(&scenario(r, rent, 0.03, 0.05), &TaxRules::builtin()).unwrap();
            let (lo, hi) = (run(ret), run(ret + d));
            for (a, b) in lo.comparison.iter().zip(&hi.comparison).filter(|(a, _)| a.kind == RowKind::Sell) {
                prop_assert!(b.net_position.unwrap().0 >= a.net_position.unwrap().0 - 1e-6);
            }
        }

        /// Returns above the break-even favour selling; below it, keeping.
        #[test]
        fn break_even_separates_sell_from_keep(
            ret in -0.05..0.15f64, rent in 0.0..30_000.0f64,
            growth in -0.03..0.08f64, rate in 0.01..0.08f64,
        ) {
            let e = evaluate(&scenario(ret, rent, growth, rate), &TaxRules::builtin()).unwrap();
            for r in e.comparison.iter().filter(|r| r.kind == RowKind::Sell) {
                let vs_keep = r.vs_keep.unwrap().0;
                match r.break_even.unwrap() {
                    BreakEven::Rate(be) if ret > be + 1e-6 => prop_assert!(vs_keep > -1e-3),
                    BreakEven::Rate(be) if ret < be - 1e-6 => prop_assert!(vs_keep < 1e-3),
                    BreakEven::Below(_) => prop_assert!(vs_keep > -1e-3),
                    BreakEven::Above(_) => prop_assert!(vs_keep < 1e-3),
                    _ => {}
                }
            }
        }
    }
}
