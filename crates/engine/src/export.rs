//! Tabular views of a result, shared by CSV export (and later the web UI).

use crate::alternative::BreakEven;
use crate::money::Nok;
use crate::sale::SaleOutcome;
use crate::sensitivity::SensitivityRow;
use crate::{Evaluation, KeepResult};

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Int(i64),
    Num(f64),
    /// A fraction, written with six decimals (0.061234 = 6.1234%).
    Rate(f64),
    Text(String),
    Bool(bool),
}

impl Cell {
    /// Plain machine-readable text: numbers with two decimals and no grouping.
    pub fn to_plain(&self) -> String {
        match self {
            Cell::Int(i) => i.to_string(),
            Cell::Num(x) => format!("{x:.2}"),
            Cell::Rate(x) => format!("{x:.6}"),
            Cell::Text(s) => s.clone(),
            Cell::Bool(b) => b.to_string(),
        }
    }
}

impl From<BreakEven> for Cell {
    fn from(b: BreakEven) -> Cell {
        match b {
            BreakEven::Rate(r) => Cell::Rate(r),
            BreakEven::Below(r) => Cell::Text(format!("<{r}")),
            BreakEven::Above(r) => Cell::Text(format!(">{r}")),
        }
    }
}

impl From<Nok> for Cell {
    fn from(n: Nok) -> Cell {
        Cell::Num(n.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

impl Table {
    /// Comma-separated rendering (no quoting; headers and cells never contain commas
    /// except user-given cost names, which the CLI writes through a real CSV writer).
    pub fn to_plain_csv(&self) -> String {
        let mut out = self.headers.join(",");
        out.push('\n');
        for row in &self.rows {
            let cells: Vec<String> = row.iter().map(Cell::to_plain).collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        out
    }
}

fn headers(fixed_before: &[&str], result: &KeepResult, fixed_after: &[&str]) -> Vec<String> {
    fixed_before
        .iter()
        .map(|s| s.to_string())
        .chain(result.cost_lines.iter().map(|c| format!("cost:{}", c.name)))
        .chain(fixed_after.iter().map(|s| s.to_string()))
        .collect()
}

/// The yearly ledger plus, per year, the outcome of selling right after it.
pub fn yearly_table(evaluation: &Evaluation) -> Table {
    let result = &evaluation.keep;
    let mut headers = headers(
        &[
            "year",
            "months",
            "rules_year",
            "gross_rent",
            "vacancy_loss",
            "rent_collected",
            "management_fee",
        ],
        result,
        &[
            "operating_costs",
            "interest",
            "loan_fees",
            "principal",
            "fellesgjeld_interest",
            "fellesgjeld_principal",
            "pre_tax_cash_flow",
            "rental_income_taxable",
            "taxable_rental_result",
            "interest_deduction",
            "tax",
            "taxable_wealth",
            "wealth_tax",
            "after_tax_cash_flow",
            "avg_monthly_out_of_pocket",
            "after_tax_monthly",
            "loan_balance_end",
            "fellesgjeld_balance_end",
        ],
    );
    headers.extend(SALE_HEADERS.iter().map(|h| format!("sold_at_year_end:{h}")));
    headers.push("break_even_sell_now".into());
    let rows = result
        .years
        .iter()
        .zip(&evaluation.year_end)
        .zip(&evaluation.year_end_break_even)
        .map(|((y, sale), be)| {
            let mut row = vec![
                Cell::Int(y.year as i64),
                Cell::Int(y.months as i64),
                Cell::Int(y.rules_year as i64),
                y.gross_rent.into(),
                y.vacancy_loss.into(),
                y.rent_collected.into(),
                y.management_fee.into(),
            ];
            row.extend(y.costs.iter().map(|c| Cell::from(*c)));
            row.extend([
                y.operating_costs.into(),
                y.interest.into(),
                y.loan_fees.into(),
                y.principal.into(),
                y.fellesgjeld_interest.into(),
                y.fellesgjeld_principal.into(),
                y.pre_tax_cash_flow.into(),
                Cell::Bool(y.rental_income_taxable),
                y.taxable_rental_result.into(),
                y.interest_deduction.into(),
                y.tax.into(),
                y.taxable_wealth.into(),
                y.wealth_tax.into(),
                y.after_tax_cash_flow.into(),
                y.avg_monthly_out_of_pocket.into(),
                y.after_tax_monthly.into(),
                y.loan_balance_end.into(),
                y.fellesgjeld_balance_end.into(),
            ]);
            row.extend(sale_cells(sale));
            row.push((*be).into());
            row
        })
        .collect();
    Table { headers, rows }
}

const SALE_HEADERS: &[&str] = &[
    "month",
    "price",
    "sale_costs",
    "entry_value",
    "gain",
    "tax_free",
    "gain_tax",
    "debt_repaid",
    "fellesgjeld",
    "equity",
    "cumulative_cash",
    "net_position",
];

fn sale_cells(o: &SaleOutcome) -> Vec<Cell> {
    vec![
        Cell::Text(o.month.to_string()),
        o.price.into(),
        o.sale_costs.into(),
        o.entry_value.into(),
        o.gain.into(),
        Cell::Bool(o.tax_free.tax_free),
        o.gain_tax.into(),
        o.debt_repaid.into(),
        o.fellesgjeld.into(),
        o.equity.into(),
        o.cumulative_cash.into(),
        o.net_position.into(),
    ]
}

/// One row per sale option (sell now, last tax-free month, chosen dates, horizon).
pub fn options_table(evaluation: &Evaluation) -> Table {
    let headers = std::iter::once("option".to_string())
        .chain(SALE_HEADERS.iter().map(|h| h.to_string()))
        .collect();
    let rows = evaluation
        .options
        .iter()
        .map(|o| {
            let mut row = vec![Cell::Text(o.label.clone())];
            row.extend(sale_cells(&o.outcome));
            row
        })
        .collect();
    Table { headers, rows }
}

pub fn monthly_table(result: &KeepResult) -> Table {
    let headers = headers(
        &[
            "month",
            "gross_rent",
            "vacancy_loss",
            "rent_collected",
            "management_fee",
        ],
        result,
        &[
            "operating_costs",
            "interest",
            "loan_fee",
            "principal",
            "pre_tax_cash_flow",
            "loan_balance",
            "fellesgjeld_interest",
            "fellesgjeld_principal",
            "fellesgjeld_balance",
            "taxable_wealth",
            "wealth_tax",
        ],
    );
    let rows = result
        .months
        .iter()
        .map(|m| {
            let mut row = vec![
                Cell::Text(m.month.to_string()),
                m.gross_rent.into(),
                m.vacancy_loss.into(),
                m.rent_collected.into(),
                m.management_fee.into(),
            ];
            row.extend(m.costs.iter().map(|c| Cell::from(*c)));
            row.extend([
                m.operating_costs.into(),
                m.interest.into(),
                m.loan_fee.into(),
                m.principal.into(),
                m.pre_tax_cash_flow.into(),
                m.loan_balance.into(),
                m.fellesgjeld_interest.into(),
                m.fellesgjeld_principal.into(),
                m.fellesgjeld_balance.into(),
                m.taxable_wealth.into(),
                m.wealth_tax.into(),
            ]);
            row
        })
        .collect();
    Table { headers, rows }
}

/// Keep vs sell at the horizon: one row per option.
pub fn comparison_table(evaluation: &Evaluation) -> Table {
    let headers = [
        "option",
        "sale_month",
        "target",
        "property_equity",
        "account_deposits",
        "account_flows",
        "account_growth",
        "account_tax",
        "account_value",
        "cumulative_cash",
        "wealth_tax",
        "net_position",
        "vs_keep",
        "break_even",
    ]
    .map(String::from)
    .to_vec();
    let opt = |n: Option<Nok>| n.map_or(Cell::Text(String::new()), Cell::from);
    let rows = evaluation
        .comparison
        .iter()
        .map(|r| {
            let a = r.account.as_ref();
            vec![
                Cell::Text(r.label.clone()),
                Cell::Text(r.sale_month.map_or(String::new(), |m| m.to_string())),
                Cell::Text(r.target.to_string()),
                r.property_equity.into(),
                opt(a.map(|a| a.deposits)),
                opt(a.map(|a| a.flows)),
                opt(a.map(|a| a.growth)),
                opt(a.map(|a| a.yearly_tax + a.liquidation_tax)),
                opt(a.map(|a| a.value)),
                r.cumulative_cash.into(),
                opt(r.wealth_tax),
                opt(r.net_position),
                opt(r.vs_keep),
                r.break_even.map_or(Cell::Text(String::new()), Cell::from),
            ]
        })
        .collect();
    Table { headers, rows }
}

/// One row per sensitivity run (base first).
pub fn sensitivity_table(rows: &[SensitivityRow]) -> Table {
    let headers = [
        "change",
        "keep_net_position",
        "keep_vs_base",
        "sell_now_vs_keep",
        "sell_now_break_even",
    ]
    .map(String::from)
    .to_vec();
    let base = rows[0].keep_net_position;
    let rows = rows
        .iter()
        .map(|r| {
            vec![
                Cell::Text(r.label.clone()),
                r.keep_net_position.into(),
                (r.keep_net_position - base).into(),
                r.sell_now_vs_keep
                    .map_or(Cell::Text(String::new()), Cell::from),
                r.sell_now_break_even
                    .map_or(Cell::Text(String::new()), Cell::from),
            ]
        })
        .collect();
    Table { headers, rows }
}
