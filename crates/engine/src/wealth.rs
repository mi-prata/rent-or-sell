//! Wealth tax (formuesskatt), valued at 31 December.
//!
//! The tax counted for an option is *marginal*: the wealth tax on your other
//! net wealth plus the option's holdings, minus the wealth tax on your other
//! net wealth alone. Holdings are the property (assessed value, your share),
//! its loan and the investment account.

use std::ops::Add;

use serde::Serialize;

use crate::explain::Explained;
use crate::money::{Nok, Rate};
use crate::scenario::{Scenario, Wealth};
use crate::tax_rules::{TaxRuleSet, TaxRules};
use crate::time::YearMonth;

/// What an option holds at a year end, at market values (your share).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Holdings {
    /// Assessed market value of a home you live in.
    pub primary_home: Nok,
    /// Assessed market value of a home you don't live in (e.g. rented out).
    pub secondary_home: Nok,
    /// Ownership share of the primary home: the valuation step applies to the
    /// whole dwelling's value.
    pub home_share: f64,
    /// Shares and the equity part of funds.
    pub shares: Nok,
    /// Bank deposits and interest funds: valued in full.
    pub cash: Nok,
    pub debt: Nok,
}

impl Add for Holdings {
    type Output = Holdings;
    fn add(self, o: Holdings) -> Holdings {
        Holdings {
            primary_home: self.primary_home + o.primary_home,
            secondary_home: self.secondary_home + o.secondary_home,
            home_share: if self.primary_home.0 != 0.0 {
                self.home_share
            } else {
                o.home_share
            },
            shares: self.shares + o.shares,
            cash: self.cash + o.cash,
            debt: self.debt + o.debt,
        }
    }
}

/// Holdings turned into taxable net wealth.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Valuation {
    /// All assets at market value (the debt-reduction denominator).
    pub gross_assets: Nok,
    pub primary_home: Nok,
    pub secondary_home: Nok,
    pub shares: Nok,
    pub cash: Nok,
    /// Sum of the taxable values above.
    pub taxable_assets: Nok,
    /// Market value − taxable value of the shares.
    pub share_discount: Nok,
    pub debt: Nok,
    /// debt × share discount / gross assets (gjeldsreduksjon).
    pub debt_reduction: Nok,
    /// taxable assets − (debt − debt reduction). Can be negative.
    pub net: Nok,
}

/// Valuation discounts and the debt-reduction rule. The primary-home discount
/// does not reduce debt; only the share discount does.
pub fn value(h: &Holdings, set: &TaxRuleSet) -> Valuation {
    let step = set.primary_home_step.value * h.home_share;
    let primary = h.primary_home.min(step) * set.primary_home_low_rate.value
        + (h.primary_home - h.primary_home.min(step)) * set.primary_home_high_rate.value;
    let secondary = h.secondary_home * set.secondary_home_rate.value;
    let shares = h.shares * set.share_valuation_rate.value;
    let gross_assets = h.primary_home + h.secondary_home + h.shares + h.cash;
    let taxable_assets = primary + secondary + shares + h.cash;
    let share_discount = h.shares - shares;
    let debt_reduction = if gross_assets.0 > 0.0 {
        h.debt * (share_discount.0 / gross_assets.0)
    } else {
        Nok::ZERO
    };
    Valuation {
        gross_assets,
        primary_home: primary,
        secondary_home: secondary,
        shares,
        cash: h.cash,
        taxable_assets,
        share_discount,
        debt: h.debt,
        debt_reduction,
        net: taxable_assets - (h.debt - debt_reduction),
    }
}

/// Wealth tax on a household's total net taxable wealth.
pub fn tax_on(net: Nok, persons: u32, set: &TaxRuleSet) -> Nok {
    let p = persons as f64;
    let low = set.wealth_threshold.value * p;
    let high = set.wealth_high_threshold.value * p;
    let band = |from: Nok, to: Option<Nok>, rate: Rate| {
        let top = to.map_or(net, |t| net.min(t));
        (top - from).max(Nok::ZERO) * rate
    };
    band(low, Some(high), set.wealth_rate.value) + band(high, None, set.wealth_high_rate.value)
}

/// One option's wealth tax for one year end.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Assessment {
    /// The December the wealth is valued at the end of.
    pub month: YearMonth,
    pub rules_year: i32,
    pub persons: u32,
    pub holdings: Holdings,
    pub valuation: Valuation,
    pub other_net_wealth: Nok,
    pub tax_with: Nok,
    pub tax_without: Nok,
    /// tax_with − tax_without.
    pub tax: Nok,
}

/// Values holdings at year ends for a scenario with a `[wealth]` section.
pub struct WealthModel<'a> {
    scenario: &'a Scenario,
    rules: &'a TaxRules,
    config: &'a Wealth,
    /// Assessed value growth factor per offset 0..=N (the price path).
    price_index: Vec<f64>,
    equity_share: f64,
}

impl<'a> WealthModel<'a> {
    /// `None` when the scenario has no `[wealth]` section.
    pub fn new(scenario: &'a Scenario, rules: &'a TaxRules) -> Option<Self> {
        Some(WealthModel {
            scenario,
            rules,
            config: scenario.wealth.as_ref()?,
            price_index: crate::sale::price_index(scenario),
            equity_share: scenario
                .alternative
                .invest
                .as_ref()
                .map_or(1.0, |i| i.equity_share),
        })
    }

    /// Whether you still live in the property at the end of `month`.
    fn lives_there(&self, month: YearMonth) -> bool {
        self.scenario.moved_in() <= month && month < self.scenario.moved_out()
    }

    /// Your share of the assessed value at the end of month `m`.
    pub fn assessed_value(&self, m: usize) -> Nok {
        let p = &self.scenario.property;
        let base = self.config.assessed_market_value.unwrap_or(p.market_value);
        base * (p.ownership_share * self.price_index[m + 1])
    }

    /// The property and its debt (loan and fellesgjeld balances) at the end of month `m`.
    pub fn property(&self, m: usize, debt: Nok) -> Holdings {
        let value = self.assessed_value(m);
        let month = self.scenario.start.add_months(m as i64);
        let (primary_home, secondary_home) = if self.lives_there(month) {
            (value, Nok::ZERO)
        } else {
            (Nok::ZERO, value)
        };
        Holdings {
            primary_home,
            secondary_home,
            home_share: self.scenario.property.ownership_share,
            debt,
            ..Holdings::default()
        }
    }

    /// An investment account worth `value` (before any deferred tax, which is
    /// not deductible). A negative balance is borrowing.
    pub fn account(&self, value: Nok) -> Holdings {
        if value.0 < 0.0 {
            return Holdings {
                debt: -value,
                ..Holdings::default()
            };
        }
        Holdings {
            shares: value * self.equity_share,
            cash: value * (1.0 - self.equity_share),
            ..Holdings::default()
        }
    }

    /// The marginal wealth tax of `holdings` at the end of month `m`.
    pub fn assess(&self, m: usize, holdings: &Holdings) -> Assessment {
        let month = self.scenario.start.add_months(m as i64);
        let set = self.rules.for_year(month.year());
        let persons = self.config.persons;
        let valuation = value(holdings, set);
        let other = Nok(self.config.other_net_wealth.value_at(month));
        let tax_with = tax_on(other + valuation.net, persons, set);
        let tax_without = tax_on(other, persons, set);
        Assessment {
            month,
            rules_year: set.income_year,
            persons,
            holdings: *holdings,
            valuation,
            other_net_wealth: other,
            tax_with,
            tax_without,
            tax: tax_with - tax_without,
        }
    }

    /// How an assessment was derived.
    pub fn explain(&self, a: &Assessment) -> Explained {
        let set = self.rules.for_year(a.month.year());
        let (h, v) = (&a.holdings, &a.valuation);
        let src = |keys: &str| format!("tax-rules {}: {keys}", a.rules_year);
        let mut assets = Vec::new();
        if h.primary_home.0 != 0.0 {
            assets.push(
                Explained::value("Home, primary", v.primary_home)
                    .formula(format!(
                        "assessed value {} × {} up to {} (whole dwelling) and × {} above",
                        h.primary_home,
                        set.primary_home_low_rate.value,
                        set.primary_home_step.value,
                        set.primary_home_high_rate.value
                    ))
                    .source(src(
                        "primary_home_low_rate, primary_home_step, primary_home_high_rate",
                    )),
            );
        }
        if h.secondary_home.0 != 0.0 {
            assets.push(
                Explained::value("Home, secondary (you don't live there)", v.secondary_home)
                    .formula(format!(
                        "assessed value {} × {}",
                        h.secondary_home, set.secondary_home_rate.value
                    ))
                    .source(src("secondary_home_rate")),
            );
        }
        if h.shares.0 != 0.0 {
            assets.push(
                Explained::value("Investment, equity part", v.shares)
                    .formula(format!(
                        "market value {} × {}",
                        h.shares, set.share_valuation_rate.value
                    ))
                    .source(src("share_valuation_rate")),
            );
        }
        if h.cash.0 != 0.0 {
            assets.push(
                Explained::value("Investment, interest/bank part", v.cash)
                    .formula("market value (no discount)"),
            );
        }
        let mut parts = vec![
            Explained::value("Taxable assets", v.taxable_assets).parts(assets),
            Explained::value("Debt", -v.debt)
                .formula("loan and fellesgjeld balances (and a negative investment balance)"),
        ];
        if v.debt_reduction.0 != 0.0 {
            parts.push(
                Explained::value("Debt reduction (gjeldsreduksjon)", v.debt_reduction).formula(
                    format!(
                        "debt {} × share discount {} / gross assets {}",
                        v.debt, v.share_discount, v.gross_assets
                    ),
                ),
            );
        }
        let net = Explained::value("This option's taxable net wealth", v.net)
            .formula("taxable assets − (debt − debt reduction)")
            .parts(parts);
        let rates = format!(
            "{} above {} and {} above {} (× {} person{})",
            set.wealth_rate.value,
            set.wealth_threshold.value,
            set.wealth_high_rate.value,
            set.wealth_high_threshold.value,
            a.persons,
            if a.persons == 1 { "" } else { "s" }
        );
        Explained::value(
            format!("Wealth tax at the end of {}", a.month.year()),
            a.tax,
        )
        .formula("tax on (other + this option) − tax on other alone")
        .parts(vec![
            Explained::value("Your other net wealth", a.other_net_wealth)
                .formula("wealth.other_net_wealth"),
            net,
            Explained::value("Tax on other + this option", a.tax_with)
                .formula(rates.clone())
                .source(src(
                    "wealth_threshold, wealth_rate, wealth_high_threshold, wealth_high_rate",
                )),
            Explained::value("Tax on other alone", a.tax_without).formula(rates),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> TaxRuleSet {
        TaxRules::builtin().for_year(2026).clone()
    }

    #[test]
    fn tax_bands() {
        let s = set();
        assert_eq!(tax_on(Nok(1_900_000.0), 1, &s), Nok::ZERO);
        assert!((tax_on(Nok(2_900_000.0), 1, &s).0 - 10_000.0).abs() < 1e-6);
        // 19.6M at 1% + 1M at 1.1%.
        let top = tax_on(Nok(22_500_000.0), 1, &s).0;
        assert!((top - (196_000.0 + 11_000.0)).abs() < 1e-6);
        // Spouses: both thresholds double.
        assert_eq!(tax_on(Nok(3_800_000.0), 2, &s), Nok::ZERO);
        assert!((tax_on(Nok(4_800_000.0), 2, &s).0 - 10_000.0).abs() < 1e-6);
        assert_eq!(tax_on(Nok(-1_000_000.0), 1, &s), Nok::ZERO);
    }

    #[test]
    fn skatteetaten_debt_reduction_example() {
        // Debt 1.8M; wealth 6M of which 4.8M primary home, 1M equity funds,
        // 0.2M other: 300 000 of debt falls on the funds × 20% = 60 000.
        let h = Holdings {
            primary_home: Nok(4_800_000.0),
            home_share: 1.0,
            shares: Nok(1_000_000.0),
            cash: Nok(200_000.0),
            debt: Nok(1_800_000.0),
            ..Holdings::default()
        };
        let v = value(&h, &set());
        assert!((v.debt_reduction.0 - 60_000.0).abs() < 1e-6);
        // 25% × 4.8M + 80% × 1M + 0.2M − (1.8M − 60 000).
        let expected = 1_200_000.0 + 800_000.0 + 200_000.0 - 1_740_000.0;
        assert!((v.net.0 - expected).abs() < 1e-6);
    }

    #[test]
    fn primary_home_step_scales_with_share() {
        // Half of a 20M home: the step applies to the whole dwelling, so half
        // of 14M is valued at 25% and half of 6M at 70%.
        let h = Holdings {
            primary_home: Nok(10_000_000.0),
            home_share: 0.5,
            ..Holdings::default()
        };
        let v = value(&h, &set());
        assert!((v.primary_home.0 - (7_000_000.0 * 0.25 + 3_000_000.0 * 0.7)).abs() < 1e-6);
        assert_eq!(v.debt_reduction, Nok::ZERO);
    }
}
