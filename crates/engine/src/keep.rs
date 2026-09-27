//! Monthly simulation of keeping the property and renting it out.

use serde::Serialize;

use crate::error::{EngineError, invalid};
use crate::loan::{self, Prepayment};
use crate::money::Nok;
use crate::scenario::{CostLine, Scenario};
use crate::time::YearMonth;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonthRow {
    pub month: YearMonth,
    pub gross_rent: Nok,
    pub vacancy_loss: Nok,
    pub rent_collected: Nok,
    pub management_fee: Nok,
    /// One amount per cost line, in the order of `KeepResult::cost_lines`.
    pub costs: Vec<Nok>,
    /// Management fee plus all cost lines.
    pub operating_costs: Nok,
    pub interest: Nok,
    pub loan_fee: Nok,
    pub principal: Nok,
    pub loan_balance: Nok,
    /// Borettslag: fellesgjeld interest paid through felleskost (deductible).
    pub fellesgjeld_interest: Nok,
    /// Borettslag: fellesgjeld principal paid through felleskost (not deductible).
    pub fellesgjeld_principal: Nok,
    /// Borettslag: your share of the fellesgjeld after this month.
    pub fellesgjeld_balance: Nok,
    /// Rent collected − operating costs − interest − fees − principal −
    /// fellesgjeld interest and principal.
    pub pre_tax_cash_flow: Nok,
    /// December only: the property's and loan's taxable net wealth at year end.
    pub taxable_wealth: Nok,
    /// December only: wealth tax the property and loan add (see `wealth`).
    pub wealth_tax: Nok,
}

/// Borettslag: your fellesgjeld payment (interest + principal) in the `start`
/// month, i.e. the part of the felleskost invoice that services the fellesgjeld.
pub fn fellesgjeld_payment(scenario: &Scenario) -> Nok {
    scenario.fellesgjeld.as_ref().map_or(Nok::ZERO, |f| {
        let first = loan::schedule(&f.as_loan(), scenario.start, 1)[0];
        first.interest + first.principal
    })
}

/// The cost lines, with a borettslag's felleskostnader (the whole invoice)
/// reduced to its operating part: the fellesgjeld is modelled separately.
pub fn cost_lines(scenario: &Scenario) -> Result<Vec<CostLine>, EngineError> {
    let mut lines = scenario.costs.lines()?;
    let payment = fellesgjeld_payment(scenario);
    if payment.0 > 0.0
        && let Some(line) = lines.iter_mut().find(|l| l.name == "felleskostnader")
    {
        // The invoice is for the whole unit; the fellesgjeld is your share.
        let fellesgjeld_part = payment / scenario.property.ownership_share;
        if fellesgjeld_part.0 > line.monthly_base.0 + 1e-6 {
            return Err(invalid(format!(
                "costs.felleskostnader ({}/month) is less than the fellesgjeld payment it includes ({}/month); for a borettslag, enter the whole monthly invoice",
                line.monthly_base, fellesgjeld_part
            )));
        }
        line.monthly_base = (line.monthly_base - fellesgjeld_part).max(Nok::ZERO);
    }
    Ok(lines)
}

pub fn simulate_months(scenario: &Scenario) -> Result<(Vec<CostLine>, Vec<MonthRow>), EngineError> {
    simulate_months_with(scenario, None)
}

/// Like [`simulate_months`], with an optional lump-sum prepayment of the loan.
/// The prepayment itself comes from savings, not from the monthly cash flow.
pub fn simulate_months_with(
    scenario: &Scenario,
    prepay: Option<Prepayment>,
) -> Result<(Vec<CostLine>, Vec<MonthRow>), EngineError> {
    let n = scenario.horizon_months();
    let start = scenario.start;
    let cost_lines = cost_lines(scenario)?;
    let rent_index = scenario.rental.rent_growth.annual_step_index(start, n);
    let cost_index = scenario.costs.growth.annual_step_index(start, n);
    let loan_rows = scenario
        .loan
        .as_ref()
        .map(|l| loan::schedule_with(l, start, n, prepay));
    let fellesgjeld_rows = scenario
        .fellesgjeld
        .as_ref()
        .map(|f| loan::schedule(&f.as_loan(), start, n));
    // Rent and running costs are for the whole property; the loan is already the owner's.
    let share = scenario.property.ownership_share;

    let rows = (0..n)
        .map(|m| {
            let gross_rent = scenario.rental.monthly_rent * (share * rent_index[m]);
            let vacancy_loss = gross_rent * scenario.rental.vacancy_share();
            let rent_collected = gross_rent - vacancy_loss;
            let management_fee = rent_collected * scenario.rental.management_fee_rate;
            let costs: Vec<Nok> = cost_lines
                .iter()
                .map(|c| c.monthly_base * (share * cost_index[m]))
                .collect();
            let operating_costs = management_fee + costs.iter().sum();
            let (interest, loan_fee, principal, loan_balance) = match &loan_rows {
                Some(rows) => {
                    let r = &rows[m];
                    (r.interest, r.fee, r.principal, r.closing_balance)
                }
                None => (Nok::ZERO, Nok::ZERO, Nok::ZERO, Nok::ZERO),
            };
            let (fg_interest, fg_principal, fg_balance) = match &fellesgjeld_rows {
                Some(rows) => {
                    let r = &rows[m];
                    (r.interest, r.principal, r.closing_balance)
                }
                None => (Nok::ZERO, Nok::ZERO, Nok::ZERO),
            };
            MonthRow {
                month: start.add_months(m as i64),
                gross_rent,
                vacancy_loss,
                rent_collected,
                management_fee,
                costs,
                operating_costs,
                interest,
                loan_fee,
                principal,
                loan_balance,
                fellesgjeld_interest: fg_interest,
                fellesgjeld_principal: fg_principal,
                fellesgjeld_balance: fg_balance,
                pre_tax_cash_flow: rent_collected
                    - operating_costs
                    - interest
                    - loan_fee
                    - principal
                    - fg_interest
                    - fg_principal,
                taxable_wealth: Nok::ZERO,
                wealth_tax: Nok::ZERO,
            }
        })
        .collect();
    Ok((cost_lines, rows))
}
