//! Selling the property at a given month: sale price, costs, gain tax, and the
//! owner's resulting net position.
//!
//! A sale "at month S" happens at the start of month S (S = 0 is `start`, S = N
//! is just after the horizon): months before S were rented out, and the debt
//! repaid is the loan balance at that point.

use serde::Serialize;

use crate::KeepResult;
use crate::explain::Explained;
use crate::ledger;
use crate::money::Nok;
use crate::scenario::Scenario;
use crate::tax_rules::{TaxRuleSet, TaxRules};
use crate::time::YearMonth;

/// Result of checking the tax-free sale rule for a sale month.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TaxFreeCheck {
    /// Whole months from purchase to sale.
    pub owned_months: i64,
    pub min_owned_months: u32,
    /// Months fully lived in within the window before the sale.
    pub lived_months: i64,
    pub required_lived_months: u32,
    pub window_months: u32,
    pub tax_free: bool,
}

/// Checks the tax-free rule for a sale in `sale` month, in whole months and
/// conservatively: a partial month never counts. Ownership must exceed the
/// minimum; occupancy counts only months strictly between moving in and
/// moving out, within the full months of the window before the sale month.
pub fn tax_free_check(scenario: &Scenario, rules: &TaxRuleSet, sale: YearMonth) -> TaxFreeCheck {
    let owned_months = sale.months_since(scenario.property.purchase_date);
    let min_owned = rules.sale_min_ownership_months.value;
    let window = rules.sale_occupancy_window_months.value;
    let required = rules.sale_occupancy_months.value;

    // Full months inside the window: sale − (window − 1) ..= sale − 1.
    let lived_months = lived_months(
        scenario,
        sale.add_months(-(window as i64 - 1)),
        sale.add_months(-1),
    );

    TaxFreeCheck {
        owned_months,
        min_owned_months: min_owned,
        lived_months,
        required_lived_months: required,
        window_months: window,
        tax_free: owned_months > min_owned as i64 && lived_months >= required as i64,
    }
}

/// Months fully lived in (moved_in + 1 ..= moved_out − 1, so partial months
/// never count) within `first..=last`.
pub fn lived_months(scenario: &Scenario, first: YearMonth, last: YearMonth) -> i64 {
    let first = first.max(scenario.moved_in().add_months(1));
    let last = last.min(scenario.moved_out().add_months(-1));
    (last.months_since(first) + 1).max(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "status", content = "last_month", rename_all = "snake_case")]
pub enum TaxFreeWindow {
    /// A tax-free sale is possible until (and including) this month.
    Open(YearMonth),
    /// The window closed; this was the last qualifying month.
    Closed(YearMonth),
    /// No month ever qualifies (e.g. never lived there long enough).
    Never,
}

/// Finds the last month a tax-free sale is possible, searching from the
/// purchase to the end of the horizon.
pub fn tax_free_window(scenario: &Scenario, rules: &TaxRules) -> TaxFreeWindow {
    let first = scenario.property.purchase_date;
    let last = scenario.start.add_months(scenario.horizon_months() as i64);
    let span = last.months_since(first);
    let found = (0..=span)
        .rev()
        .map(|k| first.add_months(k))
        .find(|m| tax_free_check(scenario, rules.for_year(m.year()), *m).tax_free);
    match found {
        Some(m) if m >= scenario.start => TaxFreeWindow::Open(m),
        Some(m) => TaxFreeWindow::Closed(m),
        None => TaxFreeWindow::Never,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SaleOutcome {
    pub month: YearMonth,
    /// Months after `start` (0 = sell now).
    pub offset: usize,
    pub rules_year: i32,
    /// Owner's share of the market value (for a borettslag, the total price
    /// including fellesgjeld).
    pub price: Nok,
    pub broker_fee: Nok,
    pub fixed_costs: Nok,
    pub sale_costs: Nok,
    /// Owner's share of purchase price + purchase costs + improvements, plus
    /// (borettslag) the fellesgjeld at purchase.
    pub entry_value: Nok,
    /// (price − sale costs) − entry value.
    pub gain: Nok,
    pub tax_free: TaxFreeCheck,
    /// Tax on the gain; negative for a deductible loss; zero if tax-free.
    pub gain_tax: Nok,
    pub debt_repaid: Nok,
    /// Borettslag: the fellesgjeld the buyer takes over.
    pub fellesgjeld: Nok,
    /// price − sale costs − debt − fellesgjeld − gain tax.
    pub equity: Nok,
    /// Sum of after-tax cash flows of the months rented before the sale.
    pub cumulative_cash: Nok,
    /// equity + cumulative cash.
    pub net_position: Nok,
}

/// Pre-computed inputs for evaluating sales at any month of the horizon.
pub struct SaleModel<'a> {
    scenario: &'a Scenario,
    rules: &'a TaxRules,
    keep: &'a KeepResult,
    /// Market value growth factor per offset 0..=N (monthly compounding).
    price_index: Vec<f64>,
    /// Cost growth factor per offset 0..=N (annual steps).
    cost_index: Vec<f64>,
}

/// Property value growth factor per offset 0..=N (monthly compounding).
pub fn price_index(scenario: &Scenario) -> Vec<f64> {
    let n = scenario.horizon_months();
    let growth = scenario.property.price_growth.resolve(scenario.start, n);
    let mut index = Vec::with_capacity(n + 1);
    index.push(1.0);
    for g in growth {
        let prev = index[index.len() - 1];
        index.push(prev * (1.0 + g).powf(1.0 / 12.0));
    }
    index
}

impl<'a> SaleModel<'a> {
    pub fn new(scenario: &'a Scenario, rules: &'a TaxRules, keep: &'a KeepResult) -> Self {
        let n = scenario.horizon_months();
        let price_index = price_index(scenario);
        let cost_index = scenario
            .costs
            .growth
            .annual_step_index(scenario.start, n + 1);
        SaleModel {
            scenario,
            rules,
            keep,
            price_index,
            cost_index,
        }
    }

    /// Offset of `month` from `start`, if within 0..=N.
    pub fn offset_of(&self, month: YearMonth) -> Option<usize> {
        let k = month.months_since(self.scenario.start);
        (0..=self.scenario.horizon_months() as i64)
            .contains(&k)
            .then_some(k as usize)
    }

    pub fn outcome(&self, offset: usize) -> SaleOutcome {
        let s = self.scenario;
        let p = &s.property;
        let share = p.ownership_share;
        let month = s.start.add_months(offset as i64);
        let rules = self.rules.for_year(month.year());

        let price = p.market_value * (share * self.price_index[offset]);
        let broker_fee = price * s.sale.broker_rate;
        let fixed_costs = s.sale.fixed_costs * (share * self.cost_index[offset]);
        let sale_costs = broker_fee + fixed_costs;
        let improvements: Nok = p.improvements.iter().map(|i| i.amount).sum();
        let fellesgjeld_at_purchase = s
            .fellesgjeld
            .as_ref()
            .map_or(Nok::ZERO, |f| f.at_purchase());
        let entry_value =
            (p.purchase_price + p.purchase_costs + improvements) * share + fellesgjeld_at_purchase;
        let gain = price - sale_costs - entry_value;
        let tax_free = tax_free_check(s, rules, month);
        let gain_tax = if tax_free.tax_free {
            Nok::ZERO
        } else {
            gain * rules.capital_income_rate.value
        };
        let debt_repaid = match offset {
            0 => s.loan.as_ref().map_or(Nok::ZERO, |l| l.balance),
            k => self.keep.months[k - 1].loan_balance,
        };
        let fellesgjeld = match offset {
            0 => s.fellesgjeld.as_ref().map_or(Nok::ZERO, |f| f.balance),
            k => self.keep.months[k - 1].fellesgjeld_balance,
        };
        let equity = price - sale_costs - debt_repaid - fellesgjeld - gain_tax;
        let cumulative_cash = ledger::aggregate(
            &self.keep.cost_lines,
            &self.keep.months[..offset],
            self.rules,
        )
        .iter()
        .map(|y| y.after_tax_cash_flow)
        .sum();

        SaleOutcome {
            month,
            offset,
            rules_year: rules.income_year,
            price,
            broker_fee,
            fixed_costs,
            sale_costs,
            entry_value,
            gain,
            tax_free,
            gain_tax,
            debt_repaid,
            fellesgjeld,
            equity,
            cumulative_cash,
            net_position: equity + cumulative_cash,
        }
    }

    /// How `o`'s net position was derived.
    pub fn explain(&self, o: &SaleOutcome) -> Explained {
        let s = self.scenario;
        let p = &s.property;
        let share = p.ownership_share;
        let rules_ref = |key: &str| format!("tax-rules {}: {key}", o.rules_year);
        let rate = self
            .rules
            .for_year(o.month.year())
            .capital_income_rate
            .value;

        let borettslag = s.fellesgjeld.is_some();
        let price_label = if borettslag {
            "Sale price (your share, incl. fellesgjeld)"
        } else {
            "Sale price (your share)"
        };
        let price = Explained::value(price_label, o.price).formula(format!(
            "market value {} × share {share} × growth factor {:.4}",
            p.market_value, self.price_index[o.offset]
        ));
        let sale_costs = Explained::value("Sale costs", o.sale_costs)
            .formula("broker fee + fixed costs")
            .parts(vec![
                Explained::value("Broker fee", o.broker_fee)
                    .formula(format!("price × {}", s.sale.broker_rate)),
                Explained::value("Fixed costs", o.fixed_costs).formula(format!(
                    "{} × share {share} × cost growth {:.4}",
                    s.sale.fixed_costs, self.cost_index[o.offset]
                )),
            ]);
        let improvements: Nok = p.improvements.iter().map(|i| i.amount).sum();
        let mut entry_formula = format!(
            "(purchase price {} + purchase costs {} + improvements {}) × share {share}",
            p.purchase_price, p.purchase_costs, improvements
        );
        if let Some(f) = &s.fellesgjeld {
            entry_formula.push_str(&format!(
                " + fellesgjeld at purchase {} (so fellesgjeld paid down since counts as cost)",
                f.at_purchase()
            ));
        }
        let mut entry =
            Explained::value("Entry value (inngangsverdi)", o.entry_value).formula(entry_formula);
        if borettslag {
            entry = entry
                .source("FSFIN § 9-5-1 (2): corrected for changes in the share of fellesgjeld");
        }
        let check = &o.tax_free;
        let rule = Explained::info(format!(
            "Tax-free sale: {} — owned {} months (must exceed {}), lived {} full months of the {} before the sale (need {})",
            if check.tax_free { "yes" } else { "no" },
            check.owned_months,
            check.min_owned_months,
            check.lived_months,
            check.window_months,
            check.required_lived_months,
        ))
        .source(rules_ref(
            "sale_min_ownership_months, sale_occupancy_months, sale_occupancy_window_months",
        ));
        let gain = Explained::value("Gain", o.gain)
            .formula("sale price − sale costs − entry value")
            .parts(vec![entry]);
        let gain_tax = if check.tax_free {
            Explained::value("Gain tax", o.gain_tax)
                .formula("0 (tax-free sale)")
                .parts(vec![gain, rule])
        } else {
            Explained::value("Gain tax", o.gain_tax)
                .formula(format!("gain × {rate} (a loss gives a deduction)"))
                .source(rules_ref("capital_income_rate"))
                .parts(vec![gain, rule])
        };
        let debt = Explained::value("Debt repaid", o.debt_repaid).formula(if o.offset == 0 {
            "loan balance at start".to_string()
        } else {
            format!("loan balance after {} payments", o.offset)
        });
        let equity = if borettslag {
            let fellesgjeld =
                Explained::value("Fellesgjeld taken over by the buyer", o.fellesgjeld).formula(
                    if o.offset == 0 {
                        "fellesgjeld at start".to_string()
                    } else {
                        format!("fellesgjeld after {} payments", o.offset)
                    },
                );
            Explained::value("Equity after sale", o.equity)
                .formula("sale price − sale costs − debt repaid − fellesgjeld − gain tax")
                .parts(vec![price, sale_costs, debt, fellesgjeld, gain_tax])
        } else {
            Explained::value("Equity after sale", o.equity)
                .formula("sale price − sale costs − debt repaid − gain tax")
                .parts(vec![price, sale_costs, debt, gain_tax])
        };

        let years = ledger::aggregate(
            &self.keep.cost_lines,
            &self.keep.months[..o.offset],
            self.rules,
        );
        let cash = Explained::value("Cumulative cash from renting", o.cumulative_cash)
            .formula("sum of after-tax cash flows before the sale (no interest)")
            .parts(
                years
                    .iter()
                    .map(|y| {
                        Explained::value(
                            format!("{} ({} months)", y.year, y.months),
                            y.after_tax_cash_flow,
                        )
                        .formula(format!(
                            "pre-tax cash flow {} − tax {} − wealth tax {}",
                            y.pre_tax_cash_flow, y.tax, y.wealth_tax
                        ))
                    })
                    .collect(),
            );

        Explained::value(format!("Wealth if sold {}", o.month), o.net_position)
            .formula("equity after sale + cumulative cash")
            .parts(vec![equity, cash])
    }
}
