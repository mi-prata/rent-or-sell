//! Renting for a while, then selling: every strategy is "sell at month `s`",
//! valued at the one horizon (`s` = 0 is selling now, `s` = horizon is renting
//! throughout). With cash-flow parity your pocket sees the same flows in each,
//! so their values at the horizon compare directly, and each can be stated as
//! the return that selling now would need to match it.
//!
//! A strategy's value is plain keep's property equity at the horizon plus the
//! matching sell row's `vs_keep` (see [`AltModel::sold_value`]).

use serde::Serialize;

use crate::alternative::{AltModel, BreakEven, InvestSpec};
use crate::money::Nok;
use crate::sale::{SaleModel, TaxFreeWindow};
use crate::scenario::Scenario;

/// Differences smaller than this share of a value count as "about the same":
/// well inside what the model's assumptions can tell apart.
pub const NEAR_TIE: f64 = 0.005;

/// Investment returns the strategies are also valued at, for "what return
/// would you need to believe" (constant, taxed like the configured investment).
pub const RETURN_GRID: (f64, f64, f64) = (0.0, 0.12, 0.005);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    SellNow,
    /// Rent until the last tax-free month, then sell.
    SellLastTaxFree,
    /// Rent until the best sale month, then sell (only when that month is
    /// strictly between now and the horizon, is not the tax-free one, and
    /// beats selling now and the tax-free sale by more than [`NEAR_TIE`]).
    SellBest,
    /// Rent to the horizon.
    Rent,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Strategy {
    pub kind: StrategyKind,
    /// Months after the start of the sale (the horizon for `Rent`).
    pub sale_offset: usize,
    /// Worth at the horizon.
    pub value: Nok,
    /// The constant return, taxed like the configured investment, at which
    /// selling now ends up at `value`.
    pub equivalent_return: BreakEven,
}

/// One sale month, valued at the horizon.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SalePoint {
    pub offset: usize,
    pub value: Nok,
    /// The sale is free of gain tax.
    pub tax_free: bool,
}

/// The strategies valued at one constant return.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReturnPoint {
    pub rate: f64,
    /// Aligned with [`Strategies::strategies`].
    pub values: Vec<Nok>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Strategies {
    /// Sell now first and rent last; the sale months in between in order.
    pub strategies: Vec<Strategy>,
    /// Every month from now (0) to the horizon, at the configured return.
    pub by_sale: Vec<SalePoint>,
    pub by_return: Vec<ReturnPoint>,
}

/// `None` without `[alternative.invest]`: a sale needs somewhere to put the money.
pub fn strategies(
    scenario: &Scenario,
    sale: &SaleModel<'_>,
    alt: &AltModel<'_>,
    window: TaxFreeWindow,
) -> Option<Strategies> {
    let spec = alt.spec()?;
    let n = scenario.horizon_months();
    let by_sale: Vec<SalePoint> = (0..=n)
        .map(|s| SalePoint {
            offset: s,
            value: alt.sold_value(spec, s, n),
            tax_free: sale.outcome(s).tax_free.tax_free,
        })
        .collect();

    let mut offsets = vec![0];
    let tax_free = match window {
        TaxFreeWindow::Open(m) => sale.offset_of(m).filter(|&k| k > 0 && k < n),
        _ => None,
    };
    offsets.extend(tax_free);
    // The best month only earns a row when it is worth telling apart: a
    // one-month blip (e.g. a partial year's rent under the tax-free rental
    // threshold) is not.
    let rival = tax_free.map_or(by_sale[0].value.0, |k| {
        by_sale[k].value.0.max(by_sale[0].value.0)
    });
    let best = by_sale
        .iter()
        .max_by(|a, b| a.value.0.total_cmp(&b.value.0))
        .filter(|p| p.value.0 - rival > NEAR_TIE * p.value.0.abs())
        .map(|p| p.offset)
        .filter(|&k| k > 0 && k < n && Some(k) != tax_free);
    offsets.extend(best);
    offsets.sort_unstable();
    offsets.push(n);

    let strategies = offsets
        .iter()
        .map(|&s| Strategy {
            kind: match s {
                0 => StrategyKind::SellNow,
                _ if s == n => StrategyKind::Rent,
                _ if Some(s) == tax_free => StrategyKind::SellLastTaxFree,
                _ => StrategyKind::SellBest,
            },
            sale_offset: s,
            value: by_sale[s].value,
            equivalent_return: alt
                .equivalent_return(by_sale[s].value, n)
                .expect("an investment is configured"),
        })
        .collect();

    let (lo, hi, step) = RETURN_GRID;
    let steps = ((hi - lo) / step).round() as usize;
    let by_return = (0..=steps)
        .map(|i| {
            let rate = lo + step * i as f64;
            let spec = InvestSpec::constant(rate, spec.tax(), n);
            ReturnPoint {
                rate,
                values: offsets
                    .iter()
                    .map(|&s| alt.sold_value(&spec, s, n))
                    .collect(),
            }
        })
        .collect();

    Some(Strategies {
        strategies,
        by_sale,
        by_return,
    })
}
