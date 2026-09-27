//! Money and rate newtypes.
//!
//! Amounts are `f64` kroner: the model projects uncertain futures, so exact
//! decimal arithmetic buys nothing, while `f64` keeps powers (annuity formula)
//! and large sensitivity sweeps simple. Round only for display and where a
//! tax rule requires it.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// An amount in Norwegian kroner.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(transparent)]
pub struct Nok(pub f64);

/// A fraction, e.g. `Rate(0.22)` for 22%.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(transparent)]
pub struct Rate(pub f64);

impl Nok {
    pub const ZERO: Nok = Nok(0.0);

    pub fn value(self) -> f64 {
        self.0
    }

    pub fn max(self, other: Nok) -> Nok {
        Nok(self.0.max(other.0))
    }

    pub fn min(self, other: Nok) -> Nok {
        Nok(self.0.min(other.0))
    }
}

impl Rate {
    pub fn value(self) -> f64 {
        self.0
    }
}

impl Add for Nok {
    type Output = Nok;
    fn add(self, rhs: Nok) -> Nok {
        Nok(self.0 + rhs.0)
    }
}

impl Sub for Nok {
    type Output = Nok;
    fn sub(self, rhs: Nok) -> Nok {
        Nok(self.0 - rhs.0)
    }
}

impl Neg for Nok {
    type Output = Nok;
    fn neg(self) -> Nok {
        Nok(-self.0)
    }
}

impl AddAssign for Nok {
    fn add_assign(&mut self, rhs: Nok) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Nok {
    fn sub_assign(&mut self, rhs: Nok) {
        self.0 -= rhs.0;
    }
}

impl Mul<f64> for Nok {
    type Output = Nok;
    fn mul(self, rhs: f64) -> Nok {
        Nok(self.0 * rhs)
    }
}

impl Mul<Rate> for Nok {
    type Output = Nok;
    fn mul(self, rhs: Rate) -> Nok {
        Nok(self.0 * rhs.0)
    }
}

impl Div<f64> for Nok {
    type Output = Nok;
    fn div(self, rhs: f64) -> Nok {
        Nok(self.0 / rhs)
    }
}

impl Sum for Nok {
    // Fold from +0.0: `f64`'s own `sum` of an empty iterator is −0.0, which
    // would print as "-0.00" in exports.
    fn sum<I: Iterator<Item = Nok>>(iter: I) -> Nok {
        Nok(iter.fold(0.0, |acc, n| acc + n.0))
    }
}

impl<'a> Sum<&'a Nok> for Nok {
    fn sum<I: Iterator<Item = &'a Nok>>(iter: I) -> Nok {
        Nok(iter.fold(0.0, |acc, n| acc + n.0))
    }
}

/// Whole kroner with space-grouped thousands, e.g. `-1 234 567`.
impl fmt::Display for Nok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rounded = self.0.round() as i64;
        let digits = rounded.unsigned_abs().to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 1);
        if rounded < 0 {
            grouped.push('-');
        }
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                grouped.push(' ');
            }
            grouped.push(c);
        }
        f.pad(&grouped)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&format!("{:.2}%", self.0 * 100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_groups_thousands() {
        assert_eq!(Nok(0.4).to_string(), "0");
        assert_eq!(Nok(999.0).to_string(), "999");
        assert_eq!(Nok(1234.0).to_string(), "1 234");
        assert_eq!(Nok(-1234567.6).to_string(), "-1 234 568");
        assert_eq!(format!("{:>8}", Nok(1234.0)), "   1 234");
    }

    #[test]
    fn empty_sum_is_positive_zero() {
        let total: Nok = std::iter::empty::<Nok>().sum();
        assert!(total.0.is_sign_positive());
    }
}
