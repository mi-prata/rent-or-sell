//! Scenario inputs, as read from a TOML file.
//!
//! Property-level amounts (value, purchase, improvements, rent, running and
//! fixed sale costs) are for the whole property and are scaled by
//! `property.ownership_share`; the loan is the owner's own and is not scaled.
//! All unknown fields are rejected, so typos in hand-edited files fail loudly
//! instead of silently falling back to defaults.

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, invalid};
use crate::money::{Nok, Rate};
use crate::time::YearMonth;
use crate::timepath::TimePath;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_HORIZON_YEARS: u32 = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// First simulated month.
    pub start: YearMonth,
    pub horizon_years: u32,
    pub property: Property,
    /// No `[loan]` section means the property is debt-free.
    #[serde(default)]
    pub loan: Option<Loan>,
    /// Your share of a borettslag's common debt; required for (and only
    /// allowed with) `property.kind = "borettslag"`.
    #[serde(default)]
    pub fellesgjeld: Option<Fellesgjeld>,
    pub rental: Rental,
    #[serde(default)]
    pub costs: Costs,
    #[serde(default)]
    pub owner: Owner,
    #[serde(default)]
    pub sale: Sale,
    #[serde(default)]
    pub alternative: Alternative,
    /// Wealth tax inputs; without them, wealth tax is not modelled.
    #[serde(default)]
    pub wealth: Option<Wealth>,
}

/// Wealth tax. The tax counted is the *marginal* one: what this
/// option's holdings add on top of your other net wealth.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Wealth {
    /// 1, or 2 for spouses assessed together (both thresholds double). With 2,
    /// `other_net_wealth` and `property.ownership_share` are the household's.
    #[serde(default = "one_person")]
    pub persons: u32,
    /// Taxable net wealth outside this property, its loan and the investment
    /// account (after valuation discounts), at each year end.
    #[serde(default)]
    pub other_net_wealth: TimePath,
    /// Skatteetaten's calculated market value (beregnet markedsverdi) of the
    /// whole property at `start`, from your tax return; grows with
    /// `property.price_growth`. Defaults to `property.market_value`.
    #[serde(default)]
    pub assessed_market_value: Option<Nok>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Property {
    #[serde(default)]
    pub kind: PropertyKind,
    /// Market value of the whole property at `start`. For a borettslag, the
    /// total price including the fellesgjeld.
    pub market_value: Nok,
    #[serde(default = "one")]
    pub ownership_share: f64,
    pub purchase_date: YearMonth,
    /// For a borettslag, the price excluding fellesgjeld (see
    /// `fellesgjeld.at_purchase`).
    pub purchase_price: Nok,
    /// Dokumentavgift, tinglysing and other costs of buying.
    #[serde(default)]
    pub purchase_costs: Nok,
    /// Påkostninger (not maintenance) made before `start`.
    #[serde(default)]
    pub improvements: Vec<Improvement>,
    /// Annual price growth, compounded monthly.
    #[serde(default)]
    pub price_growth: TimePath,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    /// Owned outright (including a unit in an eierseksjonssameie).
    #[default]
    Selveier,
    /// A share in a housing cooperative, usually with fellesgjeld.
    Borettslag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Improvement {
    pub date: YearMonth,
    pub amount: Nok,
    #[serde(default)]
    pub description: Option<String>,
}

/// When the owner lived in the property, for the tax-free sale rule. Assumes
/// continuous occupancy from `moved_in` until `moved_out`. Set both to the
/// same month if you never lived there.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// Defaults to `property.purchase_date`.
    #[serde(default)]
    pub moved_in: Option<YearMonth>,
    /// First month you no longer lived there. Defaults to `start`.
    #[serde(default)]
    pub moved_out: Option<YearMonth>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Sale {
    /// Broker commission as a share of the sale price.
    #[serde(default)]
    pub broker_rate: Rate,
    /// Marketing, takst, oppgjør etc. at `start` prices; grows with `costs.growth`.
    #[serde(default)]
    pub fixed_costs: Nok,
    /// Extra sale months to compare, besides now / last tax-free / horizon.
    #[serde(default)]
    pub sell_at: Vec<YearMonth>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum LoanKind {
    /// Equal total payments (re-computed whenever the rate changes).
    Annuity,
    /// Equal principal payments.
    Serial,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Loan {
    /// Outstanding balance at `start`.
    pub balance: Nok,
    pub kind: LoanKind,
    /// Month of the final payment.
    pub end: YearMonth,
    /// Termingebyr per payment.
    #[serde(default)]
    pub monthly_fee: Nok,
    /// Annual nominal rate; the monthly rate is nominal / 12.
    pub nominal_rate: TimePath,
    /// No principal is paid up to and including this month; afterwards the
    /// loan is re-amortised over the remaining term.
    #[serde(default)]
    pub interest_only_until: Option<YearMonth>,
}

/// Your share of a borettslag's fellesgjeld, paid through felleskost. Its
/// interest is deductible like mortgage interest; its principal is not.
/// `costs.felleskostnader` is the whole monthly invoice; the engine takes the
/// fellesgjeld payment out of it and treats the rest as operating costs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Fellesgjeld {
    /// Your share outstanding at `start`.
    pub balance: Nok,
    /// Your share when you bought (raises the entry value for gain tax by
    /// what was paid down since). Defaults to `balance`.
    #[serde(default)]
    pub at_purchase: Option<Nok>,
    pub kind: LoanKind,
    pub end: YearMonth,
    pub nominal_rate: TimePath,
    #[serde(default)]
    pub interest_only_until: Option<YearMonth>,
}

impl Fellesgjeld {
    /// The fellesgjeld as a loan schedule input (no fees).
    pub fn as_loan(&self) -> Loan {
        Loan {
            balance: self.balance,
            kind: self.kind,
            end: self.end,
            monthly_fee: Nok::ZERO,
            nominal_rate: self.nominal_rate.clone(),
            interest_only_until: self.interest_only_until,
        }
    }

    pub fn at_purchase(&self) -> Nok {
        self.at_purchase.unwrap_or(self.balance)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Rental {
    /// Gross monthly rent at `start`.
    pub monthly_rent: Nok,
    /// Expected weeks a year without a tenant (0 to 52).
    #[serde(default)]
    pub vacancy_weeks: f64,
    /// Annual rent growth, applied on each anniversary of `start`.
    #[serde(default)]
    pub rent_growth: TimePath,
    /// Management fee as a share of rent collected.
    #[serde(default)]
    pub management_fee_rate: Rate,
}

/// Vacancy is entered in weeks; rent is lost in proportion to the year.
pub const WEEKS_PER_YEAR: f64 = 52.0;

impl Rental {
    /// Expected share of rent lost to vacancy, spread evenly over every month.
    pub fn vacancy_share(&self) -> Rate {
        Rate(self.vacancy_weeks / WEEKS_PER_YEAR)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Costs {
    /// Annual cost growth (e.g. CPI), applied on each anniversary of `start`.
    #[serde(default)]
    pub growth: TimePath,
    /// For a borettslag: the whole invoice, including the fellesgjeld payment.
    #[serde(default)]
    pub felleskostnader: Option<Cost>,
    #[serde(default)]
    pub municipal_fees: Option<Cost>,
    #[serde(default)]
    pub property_tax: Option<Cost>,
    #[serde(default)]
    pub insurance: Option<Cost>,
    #[serde(default)]
    pub maintenance: Option<Cost>,
    #[serde(default)]
    pub other: Vec<NamedCost>,
}

/// What money not tied up in the property is invested in.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Alternative {
    #[serde(default)]
    pub invest: Option<Invest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Invest {
    /// Annual return, compounded monthly.
    pub annual_return: TimePath,
    #[serde(default)]
    pub tax: InvestTax,
    /// Share of the investment in equities, for wealth tax: that part is valued
    /// with the share discount, the rest (bank, interest funds) in full.
    #[serde(default = "one")]
    pub equity_share: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum InvestTax {
    /// The return is already after tax.
    #[default]
    AfterTax,
    /// Each calendar year's return is taxed at the capital income rate.
    Yearly,
    /// The gain is taxed on liquidation as share income (ASK / equity funds).
    Deferred,
}

/// A running cost given as either a monthly or an annual amount.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Cost {
    #[serde(default)]
    pub monthly: Option<Nok>,
    #[serde(default)]
    pub annual: Option<Nok>,
    /// Whether the cost reduces taxable rental income (maintenance yes, improvements no).
    #[serde(default = "yes")]
    pub deductible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct NamedCost {
    pub name: String,
    #[serde(default)]
    pub monthly: Option<Nok>,
    #[serde(default)]
    pub annual: Option<Nok>,
    #[serde(default = "yes")]
    pub deductible: bool,
}

/// A cost resolved to its monthly amount at `start`, before growth.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CostLine {
    pub name: String,
    pub monthly_base: Nok,
    pub deductible: bool,
}

fn one() -> f64 {
    1.0
}

fn one_person() -> u32 {
    1
}

fn yes() -> bool {
    true
}

fn monthly_amount(
    name: &str,
    monthly: Option<Nok>,
    annual: Option<Nok>,
) -> Result<Nok, EngineError> {
    let amount = match (monthly, annual) {
        (Some(m), None) => m,
        (None, Some(a)) => a / 12.0,
        _ => {
            return Err(invalid(format!(
                "cost {name:?}: give exactly one of `monthly` or `annual`"
            )));
        }
    };
    if !amount.0.is_finite() || amount.0 < 0.0 {
        return Err(invalid(format!("cost {name:?}: amount must be ≥ 0")));
    }
    Ok(amount)
}

impl Costs {
    /// All cost items in a fixed order, standard categories first.
    pub fn lines(&self) -> Result<Vec<CostLine>, EngineError> {
        let standard = [
            ("felleskostnader", &self.felleskostnader),
            ("municipal_fees", &self.municipal_fees),
            ("property_tax", &self.property_tax),
            ("insurance", &self.insurance),
            ("maintenance", &self.maintenance),
        ];
        let mut lines = Vec::new();
        for (name, cost) in standard {
            if let Some(c) = cost {
                lines.push(CostLine {
                    name: name.to_string(),
                    monthly_base: monthly_amount(name, c.monthly, c.annual)?,
                    deductible: c.deductible,
                });
            }
        }
        for c in &self.other {
            if lines.iter().any(|l| l.name == c.name) {
                return Err(invalid(format!("duplicate cost name {:?}", c.name)));
            }
            lines.push(CostLine {
                name: c.name.clone(),
                monthly_base: monthly_amount(&c.name, c.monthly, c.annual)?,
                deductible: c.deductible,
            });
        }
        Ok(lines)
    }
}

impl Scenario {
    pub fn from_toml_str(s: &str) -> Result<Scenario, EngineError> {
        let scenario: Scenario = toml::from_str(s)?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// The scenario as TOML (comments from an imported file are not kept).
    pub fn to_toml_string(&self) -> Result<String, EngineError> {
        toml::to_string(self).map_err(|e| invalid(format!("cannot write TOML: {e}")))
    }

    pub fn horizon_months(&self) -> usize {
        self.horizon_years as usize * 12
    }

    pub fn moved_in(&self) -> YearMonth {
        self.owner.moved_in.unwrap_or(self.property.purchase_date)
    }

    pub fn moved_out(&self) -> YearMonth {
        self.owner.moved_out.unwrap_or(self.start)
    }

    pub fn validate(&self) -> Result<(), EngineError> {
        let horizon_end = self.start.add_months(self.horizon_months() as i64);
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema_version {} is not supported (expected {SCHEMA_VERSION})",
                self.schema_version
            )));
        }
        if !(1..=MAX_HORIZON_YEARS).contains(&self.horizon_years) {
            return Err(invalid(format!(
                "horizon_years must be 1-{MAX_HORIZON_YEARS}"
            )));
        }
        non_negative("property.market_value", self.property.market_value)?;
        let share = self.property.ownership_share;
        if !(share > 0.0 && share <= 1.0) {
            return Err(invalid("property.ownership_share must be in (0, 1]"));
        }
        let p = &self.property;
        non_negative("property.purchase_price", p.purchase_price)?;
        non_negative("property.purchase_costs", p.purchase_costs)?;
        if p.purchase_date > self.start {
            return Err(invalid("property.purchase_date must not be after start"));
        }
        for i in &p.improvements {
            non_negative("property.improvements.amount", i.amount)?;
            if i.date < p.purchase_date || i.date >= self.start {
                return Err(invalid(format!(
                    "improvement dated {} must be between purchase_date and before start",
                    i.date
                )));
            }
        }
        p.price_growth
            .validate("property.price_growth", -0.99, 10.0)?;
        let (moved_in, moved_out) = (self.moved_in(), self.moved_out());
        if !(p.purchase_date <= moved_in && moved_in <= moved_out && moved_out <= self.start) {
            return Err(invalid(
                "owner: need purchase_date ≤ moved_in ≤ moved_out ≤ start (renting starts at start)",
            ));
        }
        fraction("sale.broker_rate", self.sale.broker_rate)?;
        non_negative("sale.fixed_costs", self.sale.fixed_costs)?;
        for d in &self.sale.sell_at {
            if *d < self.start || *d > horizon_end {
                return Err(invalid(format!(
                    "sale.sell_at {d} is outside the horizon ({} to {horizon_end})",
                    self.start
                )));
            }
        }
        if let Some(loan) = &self.loan {
            validate_loan("loan", loan, self.start)?;
        }
        match (p.kind, &self.fellesgjeld) {
            (PropertyKind::Borettslag, Some(f)) => {
                validate_loan("fellesgjeld", &f.as_loan(), self.start)?;
                if let Some(v) = f.at_purchase {
                    non_negative("fellesgjeld.at_purchase", v)?;
                }
            }
            (PropertyKind::Borettslag, None) => {
                return Err(invalid(
                    "property.kind = \"borettslag\" needs a [fellesgjeld] section (balance = 0 if there is none)",
                ));
            }
            (PropertyKind::Selveier, Some(_)) => {
                return Err(invalid(
                    "[fellesgjeld] is only for property.kind = \"borettslag\"",
                ));
            }
            (PropertyKind::Selveier, None) => {}
        }
        if let Some(invest) = &self.alternative.invest {
            invest
                .annual_return
                .validate("alternative.invest.annual_return", -0.99, 10.0)?;
            if !(0.0..=1.0).contains(&invest.equity_share) {
                return Err(invalid(
                    "alternative.invest.equity_share must be between 0 and 1",
                ));
            }
        }
        if let Some(w) = &self.wealth {
            if !(1..=2).contains(&w.persons) {
                return Err(invalid("wealth.persons must be 1 or 2"));
            }
            w.other_net_wealth
                .validate("wealth.other_net_wealth", -1e12, 1e12)?;
            if let Some(v) = w.assessed_market_value {
                non_negative("wealth.assessed_market_value", v)?;
            }
        }
        non_negative("rental.monthly_rent", self.rental.monthly_rent)?;
        if !(0.0..=WEEKS_PER_YEAR).contains(&self.rental.vacancy_weeks) {
            return Err(invalid("rental.vacancy_weeks must be between 0 and 52"));
        }
        fraction(
            "rental.management_fee_rate",
            self.rental.management_fee_rate,
        )?;
        self.rental
            .rent_growth
            .validate("rental.rent_growth", -0.99, 10.0)?;
        self.costs.growth.validate("costs.growth", -0.99, 10.0)?;
        self.costs.lines()?;
        Ok(())
    }
}

fn validate_loan(what: &str, loan: &Loan, start: YearMonth) -> Result<(), EngineError> {
    non_negative(&format!("{what}.balance"), loan.balance)?;
    non_negative(&format!("{what}.monthly_fee"), loan.monthly_fee)?;
    if loan.balance.0 > 0.0 && loan.end < start {
        return Err(invalid(format!(
            "{what}.end is before start but the balance is not zero"
        )));
    }
    if let Some(io) = loan.interest_only_until
        && io >= loan.end
    {
        return Err(invalid(format!(
            "{what}.interest_only_until must be before {what}.end"
        )));
    }
    loan.nominal_rate
        .validate(&format!("{what}.nominal_rate"), 0.0, 1.0)
}

fn non_negative(what: &str, v: Nok) -> Result<(), EngineError> {
    if v.0.is_finite() && v.0 >= 0.0 {
        Ok(())
    } else {
        Err(invalid(format!("{what} must be ≥ 0")))
    }
}

fn fraction(what: &str, r: Rate) -> Result<(), EngineError> {
    if (0.0..=1.0).contains(&r.0) {
        Ok(())
    } else {
        Err(invalid(format!("{what} must be between 0 and 1")))
    }
}
