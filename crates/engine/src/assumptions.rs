//! The model's simplifications, in plain words, grouped by topic. Only the
//! groups and items that apply to the scenario are returned, so every output
//! can show them with the results.

use serde::Serialize;

use crate::scenario::{PropertyKind, Scenario};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct AssumptionGroup {
    pub title: String,
    pub items: Vec<String>,
}

fn group(title: &str, items: &[&str]) -> AssumptionGroup {
    AssumptionGroup {
        title: title.into(),
        items: items.iter().map(|s| s.to_string()).collect(),
    }
}

/// The simplifications that apply to `scenario`.
pub fn assumptions(scenario: &Scenario) -> Vec<AssumptionGroup> {
    let mut out = vec![group(
        "Tax",
        &[
            "Only the tax that renting or selling changes is counted, at the 22% rate on ordinary income.",
            "A tax loss from renting counts as a saving, which assumes other taxable income to offset it.",
            "Tax is counted in the year it arises, not when it is paid (advance tax and the final settlement are ignored).",
        ],
    )];

    let mut renting = group(
        "Renting",
        &[
            "Vacancy reduces the rent evenly across the year (2 weeks ≈ 4% of the rent).",
            "Rent and costs rise once a year, on the anniversary of the start month; costs are paid evenly each month.",
        ],
    );
    if scenario.property.ownership_share != 1.0 {
        renting.items.push(
            "With part ownership, the property's value, purchase price, improvements, rent and costs count at the owned share; the loan counts in full."
                .into(),
        );
    }
    out.push(renting);

    out.push(group(
        "Selling",
        &[
            "A sale takes effect at the start of its month; the property is rented until then. House prices grow a little every month.",
            "The tax-free sale window is checked in whole months, erring on the safe side; the actual rule counts days, so sell with a margin.",
        ],
    ));

    out.push(group(
        "Comparing",
        &[
            "Each option is valued as if everything were sold and cashed out at the end of the period, after all costs and taxes.",
            "Cash-flow parity: every option has the same out-of-pocket cash flows. After a sale, the money renting would have needed (or paid out) goes into (or comes out of) the investment.",
        ],
    ));

    if scenario.alternative.invest.is_some() {
        out.push(group(
            "Investing",
            &[
                "The investment is taxed either on cashing out, like an ASK (22% × 1.72 of the gain, ignoring the tax-free allowance), or at 22% of each year's return.",
                "If withdrawals exceed the balance, the shortfall is borrowed at the same return.",
            ],
        ));
    }

    if scenario.wealth.is_some() {
        out.push(group(
            "Wealth tax",
            &[
                "Only the extra wealth tax each option adds on top of other net wealth is counted. It is assessed at 31 December, so a period ending before December has none in its last year.",
                "The property counts at its tax-assessed value (as a secondary home once no longer lived in), funds at the share discount; tax owed on the investment's gain is not deducted. The debt discount counts only this option's assets and debts.",
                "Renting's wealth tax is paid out of pocket; in other options, any difference comes from the investment.",
            ],
        ));
    }

    if scenario.property.kind == PropertyKind::Borettslag {
        out.push(group(
            "Borettslag",
            &[
                "The price includes the unit's share of the common debt (fellesgjeld), which the buyer takes over; the broker fee is assumed to apply to the full price (unconfirmed).",
                "The monthly common costs are one invoice: the common-debt part is split off and follows its loan, the rest rises with costs.",
                "Common-debt interest is tax-deductible, repayments are not; for wealth tax the common debt counts as debt.",
            ],
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::examples;

    fn titles(s: &Scenario) -> Vec<String> {
        assumptions(s).into_iter().map(|g| g.title).collect()
    }

    #[test]
    fn only_the_groups_that_apply() {
        let basic = Scenario::from_toml_str(examples::BASIC).unwrap();
        assert_eq!(
            titles(&basic),
            [
                "Tax",
                "Renting",
                "Selling",
                "Comparing",
                "Investing",
                "Wealth tax"
            ]
        );
        let bl = Scenario::from_toml_str(examples::BORETTSLAG).unwrap();
        assert!(titles(&bl).contains(&"Borettslag".to_string()));

        let mut plain = basic.clone();
        plain.wealth = None;
        assert!(!assumptions(&plain).iter().any(|g| g.title == "Wealth tax"));
    }

    #[test]
    fn no_field_names_or_you() {
        let s = Scenario::from_toml_str(examples::BORETTSLAG).unwrap();
        for item in assumptions(&s).iter().flat_map(|g| &g.items) {
            assert!(!item.contains('_') && !item.contains('['), "{item}");
            let lower = item.to_lowercase();
            assert!(!lower.contains(" you") && !lower.contains("your"), "{item}");
        }
    }
}
