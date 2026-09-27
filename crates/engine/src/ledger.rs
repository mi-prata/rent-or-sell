//! Calendar-year roll-up of the monthly simulation, including tax.

use serde::Serialize;

use crate::keep::MonthRow;
use crate::money::Nok;
use crate::scenario::CostLine;
use crate::tax_rules::TaxRules;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct YearRow {
    pub year: i32,
    /// Simulated months in this calendar year (first and last year may be partial).
    pub months: u32,
    /// Income year of the tax rule set applied.
    pub rules_year: i32,
    pub gross_rent: Nok,
    pub vacancy_loss: Nok,
    pub rent_collected: Nok,
    pub management_fee: Nok,
    pub costs: Vec<Nok>,
    pub operating_costs: Nok,
    pub interest: Nok,
    pub loan_fees: Nok,
    pub principal: Nok,
    /// Borettslag: fellesgjeld interest paid through felleskost.
    pub fellesgjeld_interest: Nok,
    /// Borettslag: fellesgjeld principal paid through felleskost.
    pub fellesgjeld_principal: Nok,
    pub pre_tax_cash_flow: Nok,
    /// False when rent collected is within the tax-free threshold.
    pub rental_income_taxable: bool,
    /// Rent collected − deductible costs, or zero when rental income is tax-free.
    pub taxable_rental_result: Nok,
    /// Interest, fellesgjeld interest and (if deductible) loan fees. Deducted
    /// even when rental income is tax-free.
    pub interest_deduction: Nok,
    /// rate × (taxable rental result − interest deduction). Negative is a saving.
    pub tax: Nok,
    /// Taxable net wealth of the property and loan at 31 December (zero if
    /// the year ends before December, or without `[wealth]`).
    pub taxable_wealth: Nok,
    /// Wealth tax the property and loan add, valued at 31 December.
    pub wealth_tax: Nok,
    /// Pre-tax cash flow − tax − wealth tax.
    pub after_tax_cash_flow: Nok,
    /// Average monthly shortfall before tax: what you top up out of pocket.
    pub avg_monthly_out_of_pocket: Nok,
    /// After-tax cash flow spread evenly over the year's months.
    pub after_tax_monthly: Nok,
    pub loan_balance_end: Nok,
    pub fellesgjeld_balance_end: Nok,
}

pub fn aggregate(cost_lines: &[CostLine], months: &[MonthRow], rules: &TaxRules) -> Vec<YearRow> {
    let mut years: Vec<YearRow> = Vec::new();
    for chunk in months.chunk_by(|a, b| a.month.year() == b.month.year()) {
        years.push(year_row(cost_lines, chunk, rules));
    }
    years
}

fn year_row(cost_lines: &[CostLine], months: &[MonthRow], rules: &TaxRules) -> YearRow {
    let year = months[0].month.year();
    let set = rules.for_year(year);
    let sum = |f: fn(&MonthRow) -> Nok| months.iter().map(f).sum::<Nok>();

    let rent_collected = sum(|m| m.rent_collected);
    let management_fee = sum(|m| m.management_fee);
    let costs: Vec<Nok> = (0..cost_lines.len())
        .map(|i| months.iter().map(|m| m.costs[i]).sum())
        .collect();
    let interest = sum(|m| m.interest);
    let loan_fees = sum(|m| m.loan_fee);
    let fellesgjeld_interest = sum(|m| m.fellesgjeld_interest);
    let pre_tax_cash_flow = sum(|m| m.pre_tax_cash_flow);

    let rental_income_taxable = rent_collected > set.rental_tax_free_threshold.value;
    let taxable_rental_result = if rental_income_taxable {
        let deductible_costs: Nok = cost_lines
            .iter()
            .zip(&costs)
            .filter(|(line, _)| line.deductible)
            .map(|(_, amount)| *amount)
            .sum();
        rent_collected - management_fee - deductible_costs
    } else {
        Nok::ZERO
    };
    let interest_deduction = if set.loan_fees_deductible.value {
        interest + fellesgjeld_interest + loan_fees
    } else {
        interest + fellesgjeld_interest
    };
    let tax = (taxable_rental_result - interest_deduction) * set.capital_income_rate.value;
    let wealth_tax = sum(|m| m.wealth_tax);
    let after_tax_cash_flow = pre_tax_cash_flow - tax - wealth_tax;
    let n = months.len() as f64;

    YearRow {
        year,
        months: months.len() as u32,
        rules_year: set.income_year,
        gross_rent: sum(|m| m.gross_rent),
        vacancy_loss: sum(|m| m.vacancy_loss),
        rent_collected,
        management_fee,
        operating_costs: sum(|m| m.operating_costs),
        costs,
        interest,
        loan_fees,
        principal: sum(|m| m.principal),
        fellesgjeld_interest,
        fellesgjeld_principal: sum(|m| m.fellesgjeld_principal),
        pre_tax_cash_flow,
        rental_income_taxable,
        taxable_rental_result,
        interest_deduction,
        tax,
        taxable_wealth: sum(|m| m.taxable_wealth),
        wealth_tax,
        after_tax_cash_flow,
        avg_monthly_out_of_pocket: sum(|m| (-m.pre_tax_cash_flow).max(Nok::ZERO)) / n,
        after_tax_monthly: after_tax_cash_flow / n,
        loan_balance_end: months[months.len() - 1].loan_balance,
        fellesgjeld_balance_end: months[months.len() - 1].fellesgjeld_balance,
    }
}
