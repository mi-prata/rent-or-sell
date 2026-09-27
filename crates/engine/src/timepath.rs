//! Values that change over time: interest rates, growth rates, etc.
//!
//! In TOML a path is written as one of
//! `{ constant = 0.05 }`,
//! `{ steps = [{ from = "2027-01", value = 0.06 }, ...] }` (holds each value until the next point),
//! `{ linear = [{ from = "2027-01", value = 0.06 }, ...] }` (interpolates between points).
//! Before the first point the first value applies; after the last point, the last value.

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, invalid};
use crate::time::YearMonth;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TimePath {
    Constant(f64),
    Steps(Vec<Point>),
    Linear(Vec<Point>),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
#[serde(deny_unknown_fields)]
pub struct Point {
    #[serde(alias = "at")]
    pub from: YearMonth,
    pub value: f64,
}

impl Default for TimePath {
    fn default() -> Self {
        TimePath::Constant(0.0)
    }
}

impl TimePath {
    pub fn validate(&self, what: &str, min: f64, max: f64) -> Result<(), EngineError> {
        let values: Vec<f64> = match self {
            TimePath::Constant(v) => vec![*v],
            TimePath::Steps(points) | TimePath::Linear(points) => {
                if points.is_empty() {
                    return Err(invalid(format!("{what}: needs at least one point")));
                }
                if points.windows(2).any(|w| w[0].from >= w[1].from) {
                    return Err(invalid(format!(
                        "{what}: points must be in increasing month order"
                    )));
                }
                points.iter().map(|p| p.value).collect()
            }
        };
        if let Some(v) = values
            .iter()
            .find(|v| !v.is_finite() || **v < min || **v > max)
        {
            return Err(invalid(format!("{what}: value {v} outside [{min}, {max}]")));
        }
        Ok(())
    }

    pub fn value_at(&self, month: YearMonth) -> f64 {
        match self {
            TimePath::Constant(v) => *v,
            TimePath::Steps(points) => {
                points
                    .iter()
                    .rev()
                    .find(|p| p.from <= month)
                    .unwrap_or(&points[0])
                    .value
            }
            TimePath::Linear(points) => {
                let first = points[0];
                let last = points[points.len() - 1];
                if month <= first.from {
                    return first.value;
                }
                if month >= last.from {
                    return last.value;
                }
                let i = points.iter().rposition(|p| p.from <= month).unwrap();
                let (a, b) = (points[i], points[i + 1]);
                let t = month.months_since(a.from) as f64 / b.from.months_since(a.from) as f64;
                a.value + t * (b.value - a.value)
            }
        }
    }

    /// The same path with `f` applied to every value.
    pub fn map(&self, f: impl Fn(f64) -> f64) -> TimePath {
        match self {
            TimePath::Constant(v) => TimePath::Constant(f(*v)),
            TimePath::Steps(points) | TimePath::Linear(points) => {
                let points = points
                    .iter()
                    .map(|p| Point {
                        from: p.from,
                        value: f(p.value),
                    })
                    .collect();
                match self {
                    TimePath::Steps(_) => TimePath::Steps(points),
                    _ => TimePath::Linear(points),
                }
            }
        }
    }

    /// The path's value for each of `months` months starting at `start`.
    pub fn resolve(&self, start: YearMonth, months: usize) -> Vec<f64> {
        (0..months)
            .map(|m| self.value_at(start.add_months(m as i64)))
            .collect()
    }

    /// Treats the path as an annual growth rate and returns a price index per month,
    /// starting at 1.0. Growth is applied in annual steps on each anniversary of
    /// `start`, using the rate in effect in that month (rents in Norway are
    /// typically adjusted at most once a year).
    pub fn annual_step_index(&self, start: YearMonth, months: usize) -> Vec<f64> {
        let mut index = 1.0;
        (0..months)
            .map(|m| {
                if m > 0 && m % 12 == 0 {
                    index *= 1.0 + self.value_at(start.add_months(m as i64));
                }
                index
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ym(s: &str) -> YearMonth {
        s.parse().unwrap()
    }

    #[test]
    fn steps_hold_values() {
        let p = TimePath::Steps(vec![
            Point {
                from: ym("2027-01"),
                value: 0.05,
            },
            Point {
                from: ym("2028-01"),
                value: 0.04,
            },
        ]);
        assert_eq!(p.value_at(ym("2026-06")), 0.05);
        assert_eq!(p.value_at(ym("2027-12")), 0.05);
        assert_eq!(p.value_at(ym("2028-01")), 0.04);
        assert_eq!(p.value_at(ym("2040-01")), 0.04);
    }

    #[test]
    fn linear_interpolates() {
        let p = TimePath::Linear(vec![
            Point {
                from: ym("2027-01"),
                value: 0.06,
            },
            Point {
                from: ym("2028-01"),
                value: 0.03,
            },
        ]);
        assert_eq!(p.value_at(ym("2026-01")), 0.06);
        assert!((p.value_at(ym("2027-05")) - 0.05).abs() < 1e-12);
        assert_eq!(p.value_at(ym("2029-01")), 0.03);
    }

    #[test]
    fn annual_step_index_steps_on_anniversaries() {
        let idx = TimePath::Constant(0.10).annual_step_index(ym("2026-10"), 25);
        assert_eq!(idx[0], 1.0);
        assert_eq!(idx[11], 1.0);
        assert!((idx[12] - 1.1).abs() < 1e-12);
        assert!((idx[24] - 1.21).abs() < 1e-12);
    }

    #[test]
    fn parses_from_toml() {
        #[derive(Deserialize)]
        struct W {
            a: TimePath,
            b: TimePath,
        }
        let w: W = toml::from_str(
            r#"
            a = { constant = 0.05 }
            b = { steps = [{ from = "2027-01", value = 0.06 }] }
            "#,
        )
        .unwrap();
        assert_eq!(w.a, TimePath::Constant(0.05));
        assert_eq!(w.b.value_at(ym("2027-02")), 0.06);
    }
}
