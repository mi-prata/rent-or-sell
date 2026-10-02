#![allow(dead_code)]

use rent_or_sell_engine::scenario::{
    Alternative, Cost, Costs, Loan, LoanKind, Owner, Property, Rental, SCHEMA_VERSION, Sale,
};
use rent_or_sell_engine::{Nok, Rate, Scenario, TimePath, YearMonth};

pub fn ym(s: &str) -> YearMonth {
    s.parse().unwrap()
}

/// A one-year scenario starting 2026-01 with no loan, no vacancy and no growth.
/// Bought 2020-01 at its current value; lived there until `start`.
pub fn simple(monthly_rent: f64) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "test".into(),
        description: None,
        start: ym("2026-01"),
        horizon_years: 1,
        property: Property {
            kind: Default::default(),
            market_value: Nok(4_000_000.0),
            ownership_share: 1.0,
            purchase_date: ym("2020-01"),
            purchase_price: Nok(4_000_000.0),
            purchase_costs: Nok(0.0),
            improvements: Vec::new(),
            price_growth: TimePath::Constant(0.0),
        },
        loan: None,
        fellesgjeld: None,
        rental: Rental {
            monthly_rent: Nok(monthly_rent),
            vacancy_weeks: 0.0,
            rent_growth: TimePath::Constant(0.0),
            management_fee_rate: Rate(0.0),
        },
        costs: Costs::default(),
        owner: Owner::default(),
        sale: Sale::default(),
        alternative: Alternative::default(),
        wealth: None,
    }
}

pub fn monthly_cost(amount: f64, deductible: bool) -> Option<Cost> {
    Some(Cost {
        monthly: Some(Nok(amount)),
        annual: None,
        deductible,
    })
}

pub fn loan(balance: f64, kind: LoanKind, rate: f64, end: &str) -> Loan {
    Loan {
        balance: Nok(balance),
        kind,
        end: ym(end),
        monthly_fee: Nok(0.0),
        nominal_rate: TimePath::Constant(rate),
        interest_only_until: None,
    }
}
