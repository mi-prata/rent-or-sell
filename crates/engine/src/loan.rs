//! Month-by-month loan schedules.

use serde::Serialize;

use crate::money::Nok;
use crate::scenario::{Loan, LoanKind};
use crate::time::YearMonth;

/// Balances below this are treated as fully repaid (floating-point dust).
const PAID_OFF: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LoanMonth {
    pub month: YearMonth,
    /// Annual nominal rate in effect this month.
    pub nominal_rate: f64,
    /// Balance at the start of the month.
    pub opening_balance: Nok,
    pub interest: Nok,
    pub principal: Nok,
    pub fee: Nok,
    pub closing_balance: Nok,
}

/// Fixed payment that repays `balance` over `n` months at `monthly_rate`.
pub fn annuity_payment(balance: f64, monthly_rate: f64, n: u32) -> f64 {
    if n == 0 {
        return balance;
    }
    if monthly_rate == 0.0 {
        return balance / n as f64;
    }
    balance * monthly_rate / (1.0 - (1.0 + monthly_rate).powi(-(n as i32)))
}

/// The loan's schedule for `months` months from `start`. Payments start in the
/// `start` month; the payment is re-computed each month over the remaining term,
/// so a rate change re-amortises the annuity. Up to `interest_only_until` only
/// interest is paid, and the loan is then amortised over the remaining term.
/// After the end the rows are zero.
pub fn schedule(loan: &Loan, start: YearMonth, months: usize) -> Vec<LoanMonth> {
    let rates = loan.nominal_rate.resolve(start, months);
    let mut balance = loan.balance.0;
    let end = loan.end;
    (0..months)
        .map(|m| {
            let month = start.add_months(m as i64);
            let opening = balance;
            let r = rates[m] / 12.0;
            let interest_only = loan.interest_only_until.is_some_and(|io| month <= io);
            let remaining = end.months_since(month) + 1;
            let (interest, principal, fee) = if balance <= PAID_OFF || remaining <= 0 {
                (0.0, 0.0, 0.0)
            } else {
                let interest = balance * r;
                let principal = if interest_only {
                    0.0
                } else if remaining == 1 {
                    balance
                } else {
                    match loan.kind {
                        LoanKind::Annuity => {
                            (annuity_payment(balance, r, remaining as u32) - interest).min(balance)
                        }
                        LoanKind::Serial => balance / remaining as f64,
                    }
                };
                (interest, principal, loan.monthly_fee.0)
            };
            balance -= principal;
            if balance < PAID_OFF {
                balance = 0.0;
            }
            LoanMonth {
                month,
                nominal_rate: rates[m],
                opening_balance: Nok(opening),
                interest: Nok(interest),
                principal: Nok(principal),
                fee: Nok(fee),
                closing_balance: Nok(balance),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;
    use crate::timepath::{Point, TimePath};

    fn ym(s: &str) -> YearMonth {
        s.parse().unwrap()
    }

    fn loan(kind: LoanKind, rate: TimePath) -> Loan {
        Loan {
            balance: Nok(3_000_000.0),
            kind,
            end: ym("2046-12"), // 240 payments from 2027-01
            monthly_fee: Nok(50.0),
            nominal_rate: rate,
            interest_only_until: None,
        }
    }

    #[test]
    fn annuity_matches_closed_form() {
        // 3 000 000 over 240 months at 5% nominal: payment = B·r / (1 − (1+r)^−n)
        let r: f64 = 0.05 / 12.0;
        let expected = 3_000_000.0 * r / (1.0 - (1.0 + r).powf(-240.0));
        assert_relative_eq!(expected, 19_798.67, epsilon = 0.01); // cross-checked in Python

        let s = schedule(
            &loan(LoanKind::Annuity, TimePath::Constant(0.05)),
            ym("2027-01"),
            240,
        );
        for row in &s {
            assert_relative_eq!((row.interest + row.principal).0, expected, epsilon = 1e-6);
            assert_eq!(row.fee, Nok(50.0));
        }
        assert_relative_eq!(s[0].interest.0, 12_500.0, epsilon = 1e-9);
        assert_eq!(s[239].closing_balance, Nok::ZERO);
    }

    #[test]
    fn serial_has_constant_principal() {
        let s = schedule(
            &loan(LoanKind::Serial, TimePath::Constant(0.05)),
            ym("2027-01"),
            240,
        );
        for row in &s {
            assert_relative_eq!(row.principal.0, 12_500.0, epsilon = 1e-6);
        }
        assert_eq!(s[239].closing_balance, Nok::ZERO);
    }

    #[test]
    fn zero_rate_and_rows_after_end() {
        let s = schedule(
            &loan(LoanKind::Annuity, TimePath::Constant(0.0)),
            ym("2027-01"),
            250,
        );
        assert_relative_eq!(s[0].principal.0, 12_500.0, epsilon = 1e-6);
        assert_eq!(s[245].principal, Nok::ZERO);
        assert_eq!(s[245].fee, Nok::ZERO);
    }

    #[test]
    fn rate_change_reamortises_over_remaining_term() {
        let rate = TimePath::Steps(vec![
            Point {
                from: ym("2027-01"),
                value: 0.05,
            },
            Point {
                from: ym("2028-01"),
                value: 0.07,
            },
        ]);
        let s = schedule(&loan(LoanKind::Annuity, rate), ym("2027-01"), 240);
        let balance = s[11].closing_balance.0;
        let expected = annuity_payment(balance, 0.07 / 12.0, 228);
        assert_relative_eq!(
            (s[12].interest + s[12].principal).0,
            expected,
            epsilon = 1e-6
        );
        assert_eq!(s[239].closing_balance, Nok::ZERO);
    }

    #[test]
    fn interest_only_then_amortised_over_the_remaining_term() {
        let mut l = loan(LoanKind::Annuity, TimePath::Constant(0.06));
        l.interest_only_until = Some(ym("2027-12"));
        let rows = schedule(&l, ym("2027-01"), 24);
        for r in &rows[..12] {
            assert_eq!(r.principal, Nok(0.0));
            assert_relative_eq!(r.interest.0, 15_000.0, epsilon = 1e-9);
        }
        // From 2028-01: an annuity over the remaining 228 months.
        let payment = annuity_payment(3_000_000.0, 0.005, 228);
        assert_relative_eq!(
            (rows[12].interest + rows[12].principal).0,
            payment,
            epsilon = 1e-6
        );
        assert!(
            payment > 15_000.0 + 3_000.0,
            "the payment jumps after the period"
        );
    }
}
