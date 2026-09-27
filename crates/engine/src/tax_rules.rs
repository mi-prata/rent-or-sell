//! Versioned tax rule sets.
//!
//! Rule *values* are data: one TOML file per income year under `crates/engine/tax-rules/`,
//! each value carrying its source and the date it was last verified. How a rule
//! *works* is code; if the law changes a rule's structure, add new code gated
//! on the rule set's `income_year`.

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, invalid};
use crate::money::{Nok, Rate};

const BUILTIN: &[&str] = &[include_str!("../tax-rules/2026.toml")];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param<T> {
    pub value: T,
    pub source: String,
    /// ISO date (YYYY-MM-DD) the value was last checked against `source`.
    pub verified_on: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxRuleSet {
    pub income_year: i32,
    #[serde(default)]
    pub description: Option<String>,
    /// Tax rate on alminnelig inntekt (net rental income, interest deductions).
    pub capital_income_rate: Param<Rate>,
    /// Rental income up to this amount per calendar year is tax-free.
    pub rental_tax_free_threshold: Param<Nok>,
    /// Whether loan fees (termingebyr) are deductible like interest.
    pub loan_fees_deductible: Param<bool>,
    /// Tax-free sale: ownership must exceed this many months.
    pub sale_min_ownership_months: Param<u32>,
    /// Tax-free sale: months lived in the home within the window before the sale.
    pub sale_occupancy_months: Param<u32>,
    /// Tax-free sale: length of that window.
    pub sale_occupancy_window_months: Param<u32>,
    /// Share/equity-fund gains are multiplied by this before `capital_income_rate`.
    pub share_income_uplift_factor: Param<f64>,
    /// Wealth tax: net wealth up to this amount per person is untaxed (bunnfradrag).
    pub wealth_threshold: Param<Nok>,
    /// Wealth tax rate (municipal + state) above `wealth_threshold`.
    pub wealth_rate: Param<Rate>,
    /// Wealth tax: per-person threshold for the higher rate.
    pub wealth_high_threshold: Param<Nok>,
    /// Wealth tax rate (municipal + state) above `wealth_high_threshold`.
    pub wealth_high_rate: Param<Rate>,
    /// Primary home: share of the assessed market value taxed up to `primary_home_step`.
    pub primary_home_low_rate: Param<Rate>,
    /// Primary home: value (whole dwelling) where the higher valuation rate starts.
    pub primary_home_step: Param<Nok>,
    /// Primary home: share of the assessed market value taxed above `primary_home_step`.
    pub primary_home_high_rate: Param<Rate>,
    /// Secondary home (e.g. rented out): share of the assessed market value taxed.
    pub secondary_home_rate: Param<Rate>,
    /// Shares and the equity part of funds: share of market value taxed.
    pub share_valuation_rate: Param<Rate>,
}

impl TaxRuleSet {
    pub fn from_toml_str(s: &str) -> Result<TaxRuleSet, EngineError> {
        let set: TaxRuleSet = toml::from_str(s)?;
        let rate = set.capital_income_rate.value.0;
        if !(0.0..1.0).contains(&rate) {
            return Err(invalid(format!(
                "capital_income_rate {rate} must be in [0, 1)"
            )));
        }
        let rates = [
            ("wealth_rate", set.wealth_rate.value),
            ("wealth_high_rate", set.wealth_high_rate.value),
            ("primary_home_low_rate", set.primary_home_low_rate.value),
            ("primary_home_high_rate", set.primary_home_high_rate.value),
            ("secondary_home_rate", set.secondary_home_rate.value),
            ("share_valuation_rate", set.share_valuation_rate.value),
        ];
        if let Some((name, r)) = rates.iter().find(|(_, r)| !(0.0..=1.0).contains(&r.0)) {
            return Err(invalid(format!("{name} {} must be in [0, 1]", r.0)));
        }
        if set.wealth_high_threshold.value < set.wealth_threshold.value {
            return Err(invalid(
                "wealth_high_threshold must not be below wealth_threshold",
            ));
        }
        Ok(set)
    }
}

/// All known rule sets, ordered by income year.
#[derive(Clone, Debug)]
pub struct TaxRules {
    sets: Vec<TaxRuleSet>,
}

impl TaxRules {
    /// The rule sets shipped in `crates/engine/tax-rules/`.
    pub fn builtin() -> TaxRules {
        let sets = BUILTIN
            .iter()
            .map(|s| TaxRuleSet::from_toml_str(s).expect("built-in tax rules must parse"))
            .collect();
        TaxRules::from_sets(sets).expect("built-in tax rules must be consistent")
    }

    pub fn from_sets(mut sets: Vec<TaxRuleSet>) -> Result<TaxRules, EngineError> {
        if sets.is_empty() {
            return Err(invalid("at least one tax rule set is required"));
        }
        sets.sort_by_key(|s| s.income_year);
        if sets
            .windows(2)
            .any(|w| w[0].income_year == w[1].income_year)
        {
            return Err(invalid("duplicate income_year in tax rule sets"));
        }
        Ok(TaxRules { sets })
    }

    /// The latest rule set not after `year`; years before the earliest set use
    /// the earliest one.
    pub fn for_year(&self, year: i32) -> &TaxRuleSet {
        self.sets
            .iter()
            .rev()
            .find(|s| s.income_year <= year)
            .unwrap_or(&self.sets[0])
    }

    pub fn sets(&self) -> &[TaxRuleSet] {
        &self.sets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_parses_and_selects_latest_not_after() {
        let rules = TaxRules::builtin();
        assert_eq!(rules.for_year(2026).income_year, 2026);
        assert_eq!(rules.for_year(2040).income_year, 2026);
        assert_eq!(rules.for_year(2020).income_year, 2026);
        assert_eq!(rules.for_year(2026).capital_income_rate.value, Rate(0.22));
    }
}
