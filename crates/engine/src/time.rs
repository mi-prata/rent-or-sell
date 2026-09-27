use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, invalid};

/// A calendar month, written `"YYYY-MM"` in scenario files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify), tsify(type = "string"))]
pub struct YearMonth {
    year: i32,
    month: u8,
}

impl YearMonth {
    pub fn new(year: i32, month: u8) -> Result<Self, EngineError> {
        if !(1..=12).contains(&month) {
            return Err(invalid(format!("month must be 1-12, got {month}")));
        }
        Ok(YearMonth { year, month })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    fn ordinal(self) -> i64 {
        self.year as i64 * 12 + (self.month as i64 - 1)
    }

    fn from_ordinal(ordinal: i64) -> Self {
        YearMonth {
            year: ordinal.div_euclid(12) as i32,
            month: (ordinal.rem_euclid(12) + 1) as u8,
        }
    }

    pub fn add_months(self, months: i64) -> Self {
        Self::from_ordinal(self.ordinal() + months)
    }

    /// Number of months from `earlier` to `self` (negative if `self` is earlier).
    pub fn months_since(self, earlier: YearMonth) -> i64 {
        self.ordinal() - earlier.ordinal()
    }
}

impl FromStr for YearMonth {
    type Err = EngineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || invalid(format!("expected a month as \"YYYY-MM\", got {s:?}"));
        let (y, m) = s.split_once('-').ok_or_else(bad)?;
        if y.len() != 4 || m.len() != 2 {
            return Err(bad());
        }
        let year = y.parse().map_err(|_| bad())?;
        let month = m.parse().map_err(|_| bad())?;
        YearMonth::new(year, month)
    }
}

impl TryFrom<String> for YearMonth {
    type Error = EngineError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<YearMonth> for String {
    fn from(ym: YearMonth) -> String {
        ym.to_string()
    }
}

impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&format!("{:04}-{:02}", self.year, self.month))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_arithmetic() {
        let ym: YearMonth = "2026-11".parse().unwrap();
        assert_eq!(ym.add_months(2).to_string(), "2027-01");
        assert_eq!(ym.add_months(-11).to_string(), "2025-12");
        assert_eq!("2030-01".parse::<YearMonth>().unwrap().months_since(ym), 38);
        assert!("2026-13".parse::<YearMonth>().is_err());
        assert!("2026-1".parse::<YearMonth>().is_err());
    }
}
