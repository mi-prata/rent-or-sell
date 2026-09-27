//! The owner's monthly cash flow in phases, split where a debt is paid off.

use approx::assert_relative_eq;
use rent_or_sell_engine::headline::{CashPhase, Debt, Trend};
use rent_or_sell_engine::{Scenario, TaxRules, evaluate, examples, headline};

fn phases(s: &Scenario) -> (Vec<CashPhase>, f64) {
    let e = evaluate(s, &TaxRules::builtin()).unwrap();
    let h = headline(s, &e);
    (h.cash, e.comparison[0].cumulative_cash.0)
}

fn basic(years: u32) -> Scenario {
    let mut s = Scenario::from_toml_str(examples::BASIC).unwrap();
    s.horizon_years = years;
    s
}

#[test]
fn phases_add_up_to_renting_cash() {
    for s in [
        basic(10),
        basic(30),
        Scenario::from_toml_str(examples::BORETTSLAG).unwrap(),
    ] {
        let (p, total) = phases(&s);
        assert_relative_eq!(
            p.iter().map(|x| x.total).sum::<f64>(),
            total,
            epsilon = 1e-6
        );
    }
}

#[test]
fn one_phase_while_the_loan_runs() {
    // The loan ends in 2051-09, after a 10-year horizon.
    let (p, _) = phases(&basic(10));
    assert_eq!(p.len(), 1);
    assert_eq!(
        (p[0].start.as_str(), p[0].end.as_str()),
        ("2026-10", "2036-09")
    );
    assert!(p[0].monthly < 0.0);
    assert_eq!(p[0].ends_with, None);
}

#[test]
fn the_loan_payoff_splits_paid_in_from_paid_out() {
    let (p, _) = phases(&basic(30));
    assert_eq!(p.len(), 2, "{p:?}");
    assert_eq!(p[0].end, "2051-09");
    assert_eq!(p[0].ends_with, Some(Debt::Loan));
    assert!(p[0].monthly < 0.0);
    assert_eq!(p[1].start, "2051-10");
    assert!(p[1].monthly > 0.0);
    // Without the loan, the payout grows with the rent.
    assert_eq!(p[1].trend, Trend::Rising);
    assert_eq!(p[1].rent_growth, Some(0.03));
}
