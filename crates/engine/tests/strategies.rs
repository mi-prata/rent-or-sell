//! Rent until a month, then sell: values at the horizon and equivalent returns.

use approx::assert_relative_eq;
use rent_or_sell_engine::scenario::{Invest, InvestTax};
use rent_or_sell_engine::{
    BreakEven, Evaluation, RowKind, Scenario, StrategyKind, TaxRules, TimePath, evaluate, headline,
};

fn rules() -> TaxRules {
    TaxRules::builtin()
}

fn basic() -> Scenario {
    Scenario::from_toml_str(rent_or_sell_engine::examples::BASIC).unwrap()
}

fn with_return(mut s: Scenario, r: f64, tax: InvestTax) -> Scenario {
    s.alternative.invest = Some(Invest {
        annual_return: TimePath::Constant(r),
        tax,
        equity_share: 1.0,
    });
    s
}

fn rate(b: BreakEven) -> f64 {
    match b {
        BreakEven::Rate(r) => r,
        other => panic!("expected a rate, got {other:?}"),
    }
}

fn keep_equity(e: &Evaluation) -> f64 {
    e.comparison[0].property_equity.0
}

#[test]
fn sale_months_match_the_comparison_rows() {
    let e = evaluate(&basic(), &rules()).unwrap();
    let st = e.strategies.as_ref().unwrap();
    let n = basic().horizon_months();
    assert_eq!(st.by_sale.len(), n + 1);
    for row in e.comparison.iter().filter(|r| r.kind == RowKind::Sell) {
        let s = row.sale_month.unwrap().months_since(basic().start) as usize;
        let expected = keep_equity(&e) + row.vs_keep.unwrap().0;
        assert_relative_eq!(st.by_sale[s].value.0, expected, epsilon = 1e-6);
    }
    assert_relative_eq!(st.by_sale[n].value.0, keep_equity(&e), epsilon = 1e-6);
}

#[test]
fn basic_strategies_are_now_tax_free_month_and_rent() {
    let s = basic();
    let e = evaluate(&s, &rules()).unwrap();
    let h = headline(&s, &e);
    let kinds: Vec<_> = h.strategies.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [
            StrategyKind::SellNow,
            StrategyKind::SellLastTaxFree,
            StrategyKind::Rent
        ]
    );
    assert_eq!(h.strategies[1].sale_month, "2027-07");
    assert_eq!(h.strategies[2].sale_month, h.horizon);
    let v = h.verdict.as_ref().unwrap();
    assert_eq!((v.winner, v.loser), (2, 0));
    assert!(!v.tie);
    assert_relative_eq!(v.gap, h.gap.unwrap(), epsilon = 1e-6);
    assert_relative_eq!(
        v.gap_share.unwrap(),
        v.gap / h.strategies[0].value,
        epsilon = 1e-12
    );
    assert_relative_eq!(v.chosen_return.unwrap(), 0.06, epsilon = 1e-9);
    assert_relative_eq!(h.strategies[2].vs_sell_now, h.gap.unwrap(), epsilon = 1e-6);
    assert!(h.returns_before_tax);
    // Tax-free up to 2027-07, then not.
    let point = |m: &str| h.by_sale.iter().find(|p| p.month == m).unwrap();
    assert!(point("2027-07").tax_free);
    assert!(!point("2027-08").tax_free);
    assert!(point("2027-08").vs_sell_now < 0.0);
}

#[test]
fn selling_nows_equivalent_is_the_configured_return() {
    for tax in [InvestTax::Deferred, InvestTax::Yearly, InvestTax::AfterTax] {
        let s = with_return(basic(), 0.06, tax);
        let e = evaluate(&s, &rules()).unwrap();
        let now = &e.strategies.as_ref().unwrap().strategies[0];
        assert_eq!(now.kind, StrategyKind::SellNow);
        assert_relative_eq!(rate(now.equivalent_return), 0.06, epsilon = 1e-9);
    }
}

#[test]
fn investing_at_rentings_equivalent_ties_with_renting() {
    let e = evaluate(&basic(), &rules()).unwrap();
    let rent = e.strategies.as_ref().unwrap().strategies.last().unwrap();
    assert_eq!(rent.kind, StrategyKind::Rent);
    let r = rate(rent.equivalent_return);
    assert!(r > 0.06, "{r}"); // renting wins at 6%
    let s = with_return(basic(), r, InvestTax::Deferred);
    let h = headline(&s, &evaluate(&s, &rules()).unwrap());
    assert!(h.gap.unwrap().abs() < 1.0, "{}", h.gap.unwrap());
}

#[test]
fn after_tax_equivalent_matches_the_break_even() {
    // Without wealth tax the savings don't interact with the sale, so this
    // matches the break-even (which leaves the savings out) exactly; with it,
    // they shift what sits above the threshold, which moves it slightly.
    let mut s = with_return(basic(), 0.04, InvestTax::AfterTax);
    s.wealth = None;
    let e = evaluate(&s, &rules()).unwrap();
    let rent = e.strategies.as_ref().unwrap().strategies.last().unwrap();
    let sell_now = e
        .comparison
        .iter()
        .find(|r| r.kind == RowKind::Sell)
        .unwrap();
    assert_relative_eq!(
        rate(rent.equivalent_return),
        rate(sell_now.break_even.unwrap()),
        epsilon = 1e-9
    );
}

#[test]
fn by_return_holds_renting_flat_and_matches_the_configured_return() {
    let s = basic();
    let e = evaluate(&s, &rules()).unwrap();
    let st = e.strategies.as_ref().unwrap();
    let rent = st.strategies.len() - 1;
    for w in st.by_return.windows(2) {
        assert_relative_eq!(w[0].values[rent].0, w[1].values[rent].0, epsilon = 1e-6);
        assert!(w[1].values[0].0 > w[0].values[0].0);
    }
    let at6 = st
        .by_return
        .iter()
        .find(|p| (p.rate - 0.06).abs() < 1e-9)
        .unwrap();
    for (v, x) in at6.values.iter().zip(&st.strategies) {
        assert_relative_eq!(v.0, x.value.0, epsilon = 1e-6);
    }
}

#[test]
fn without_an_investment_there_are_no_strategies() {
    let mut s = basic();
    s.alternative.invest = None;
    s.savings = None;
    let e = evaluate(&s, &rules()).unwrap();
    assert!(e.strategies.is_none());
    let h = headline(&s, &e);
    assert!(h.strategies.is_empty() && h.by_sale.is_empty() && h.verdict.is_none());
}

#[test]
fn a_high_return_makes_selling_now_the_verdict_without_a_one_month_blip() {
    // At 12% selling clearly wins; selling a month or so later can edge it
    // by a hair (a partial year's rent under the tax-free threshold), which
    // must not become the verdict or a "best month" row.
    let s = with_return(basic(), 0.12, InvestTax::Deferred);
    let h = headline(&s, &evaluate(&s, &rules()).unwrap());
    let v = h.verdict.as_ref().unwrap();
    assert_eq!(h.strategies[v.winner].kind, StrategyKind::SellNow);
    assert_eq!(h.strategies[v.loser].kind, StrategyKind::Rent);
    assert!(
        h.strategies
            .iter()
            .all(|x| x.kind != StrategyKind::SellBest)
    );
    assert!(!matches!(
        v.timing,
        Some(rent_or_sell_engine::headline::Timing::Better { .. })
    ));
    for x in &h.strategies {
        let near = (x.value - h.strategies[v.winner].value).abs()
            <= rent_or_sell_engine::strategies::NEAR_TIE * h.strategies[v.winner].value;
        assert_eq!(x.about_same, near && x.kind != StrategyKind::SellNow);
    }
}

#[test]
fn a_later_sale_that_beats_both_ends_is_the_timing_line() {
    // The borettslag example: renting beats selling now, and selling in the
    // last tax-free month beats renting by more than the near-tie margin.
    let s = Scenario::from_toml_str(rent_or_sell_engine::examples::BORETTSLAG).unwrap();
    let h = headline(&s, &evaluate(&s, &rules()).unwrap());
    let v = h.verdict.as_ref().unwrap();
    assert_eq!(h.strategies[v.winner].kind, StrategyKind::Rent);
    match v.timing {
        Some(rent_or_sell_engine::headline::Timing::Better { index, gain, .. }) => {
            assert_eq!(h.strategies[index].kind, StrategyKind::SellLastTaxFree);
            assert_relative_eq!(
                gain,
                h.strategies[index].value - h.strategies[v.winner].value,
                epsilon = 1e-6
            );
        }
        ref other => panic!("expected a better sale month, got {other:?}"),
    }
}
