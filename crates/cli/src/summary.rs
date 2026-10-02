//! The default `run` output: a short verdict with a couple of bar charts.

use std::fmt::Write;
use std::io::IsTerminal;

use rent_or_sell_engine::Nok;
use rent_or_sell_engine::StrategyKind;
use rent_or_sell_engine::headline::{
    Bound, CashPhase, Debt, Headline, StrategyRow, TaxFreeStatus, Timing, Trend,
};

/// ANSI styling, off when stdout is not a terminal or `NO_COLOR` is set.
#[derive(Clone, Copy)]
pub struct Style {
    color: bool,
}

impl Style {
    pub fn detect() -> Style {
        Style {
            color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    pub fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
    pub fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
    pub fn green(&self, s: &str) -> String {
        self.paint("32", s)
    }
    pub fn red(&self, s: &str) -> String {
        self.paint("31", s)
    }
    pub fn yellow(&self, s: &str) -> String {
        self.paint("33", s)
    }
    pub fn cyan(&self, s: &str) -> String {
        self.paint("36", s)
    }
}

const BAR_WIDTH: f64 = 20.0;

/// A horizontal bar `value / max` of `BAR_WIDTH` characters, with eighth blocks.
fn bar(value: f64, max: f64) -> String {
    if max <= 0.0 || value <= 0.0 {
        return String::new();
    }
    let eighths = ((value / max).min(1.0) * BAR_WIDTH * 8.0).round() as usize;
    let partial = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"][eighths % 8];
    format!("{}{partial}", "█".repeat(eighths / 8))
}

fn millions(n: Nok) -> String {
    format!("{:.2}M", n.0 / 1e6)
}

/// "1.16M kr" from a million up, else "63 321 kr".
fn kr(n: Nok) -> String {
    if n.0.abs() >= 1e6 {
        format!("{} kr", millions(n))
    } else {
        format!("{n} kr")
    }
}

fn pct(r: f64) -> String {
    format!("{:.2}%", r * 100.0)
}

/// "15 years", "1 year".
pub fn period(n: u32) -> String {
    if n == 1 {
        "1 year".into()
    } else {
        format!("{n} years")
    }
}

/// "Selling and investing", "Rent, sell in 2027-07 (last tax-free)", "Renting" (the whole period).
pub fn strategy_label(s: &StrategyRow) -> String {
    match s.kind {
        StrategyKind::SellNow => "Selling and investing".into(),
        StrategyKind::SellLastTaxFree => format!("Rent, sell in {} (last tax-free)", s.sale_month),
        StrategyKind::SellBest => format!("Rent, sell in {} (best month)", s.sale_month),
        StrategyKind::Rent => "Renting".into(),
    }
}

/// "+5.4%": a difference as a share.
fn share(x: f64) -> String {
    let sign = if x < 0.0 { "−" } else { "+" };
    format!("{sign}{:.1}%", x.abs() * 100.0)
}

/// Why a sale month is one of the strategies.
fn why(s: &StrategyRow) -> &'static str {
    match s.kind {
        StrategyKind::SellLastTaxFree => "last tax-free month",
        _ => "best month",
    }
}

/// One cash-flow phase in three columns: when ("Until 2037-12 (loan paid off)"),
/// the monthly amount ("6 546 kr a month paid in") and the rest ("0.88M in
/// total, rising 3% a year with the rent").
pub fn cash_phase(phases: &[CashPhase], i: usize) -> [String; 3] {
    let p = &phases[i];
    let last = i + 1 == phases.len();
    let mut when = match (phases.len(), i, last) {
        (1, _, _) => "Over the whole period".to_string(),
        (_, 0, _) => format!("Until {}", p.end),
        (_, _, true) => format!("From {}", p.start),
        _ => format!("{} to {}", p.start, p.end),
    };
    if !last {
        match p.ends_with {
            Some(Debt::Loan) => when.push_str(" (loan paid off)"),
            Some(Debt::Fellesgjeld) => when.push_str(" (fellesgjeld paid off)"),
            None => {}
        }
    }
    let way = if p.monthly < 0.0 {
        "paid in"
    } else {
        "paid out"
    };
    let trend = match (p.trend, p.rent_growth) {
        (Trend::Rising, Some(g)) => format!(", rising {} a year with the rent", pct_short(g)),
        (Trend::Rising, None) => ", rising each year".to_string(),
        (Trend::Falling, _) => ", falling each year".to_string(),
        (Trend::Flat, _) => String::new(),
    };
    [
        when,
        format!("{} kr a month {way}", Nok(p.monthly.abs())),
        format!("{} in total{trend}", millions(Nok(p.total.abs()))),
    ]
}

/// 0.03 → "3%", 0.025 → "2.5%".
fn pct_short(r: f64) -> String {
    let s = format!("{:.2}", r * 100.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}%")
}

/// Pads `s` (which may contain ANSI codes) to `width` visible characters.
fn pad(s: &str, visible: usize, width: usize) -> String {
    format!("{s}{}", " ".repeat(width.saturating_sub(visible)))
}

pub fn render(h: &Headline, st: Style) -> String {
    let mut out = String::new();
    let years = h.horizon_years;
    let _ = writeln!(
        out,
        "{} {}\n",
        st.bold(&h.name),
        st.dim(&format!(
            "— rent or sell? {}, {} → {}",
            period(years),
            h.start,
            h.horizon
        ))
    );

    // Verdict: selling now vs renting for the whole period, how safe that is,
    // and whether a month in between changes it.
    if let Some(v) = &h.verdict {
        let w = &h.strategies[v.winner];
        let rent_label = format!("renting for {}", period(h.horizon_years));
        let pc = v
            .gap_share
            .map_or(String::new(), |x| format!(" ({})", share(x)));
        let line = if v.tie {
            format!(
                "ABOUT EVEN: selling now and {rent_label} are within {}{pc}.",
                kr(Nok(v.gap))
            )
        } else if w.kind == StrategyKind::SellNow {
            format!("SELL NOW: {}{pc} more than {rent_label}.", kr(Nok(v.gap)))
        } else {
            format!(
                "RENT FOR {}: {}{pc} more than selling now.",
                period(h.horizon_years).to_uppercase(),
                kr(Nok(v.gap))
            )
        };
        let colored = match (v.tie, w.kind) {
            (true, _) => st.yellow(&line),
            (false, StrategyKind::SellNow) => st.red(&line),
            _ => st.green(&line),
        };
        let _ = writeln!(out, "  {}", st.bold(&colored));

        match &v.timing {
            Some(Timing::Better {
                index,
                gain,
                gain_share,
            }) => {
                let x = &h.strategies[*index];
                let _ = writeln!(
                    out,
                    "  {}",
                    st.bold(&format!(
                        "Better still: rent, then sell in {} ({}): {} more ({}).",
                        x.sale_month,
                        why(x),
                        kr(Nok(*gain)),
                        share(*gain_share)
                    ))
                );
            }
            Some(Timing::AboutSame { index }) => {
                let x = &h.strategies[*index];
                let _ = writeln!(
                    out,
                    "  Selling in {} ({}) gives about the same.",
                    x.sale_month,
                    why(x)
                );
            }
            None => {}
        }
    }

    // The equity at stake, and the cash flow every option shares.
    let _ = writeln!(
        out,
        "\n  Selling today frees {} of equity (after sale costs, loans and tax).",
        millions(Nok(h.proceeds_now))
    );
    if !h.cash.is_empty() {
        let _ = writeln!(out, "\n  {}", st.cyan("Monthly cash flow"));
        let _ = writeln!(
            out,
            "    {}",
            st.dim("What the owner pays in or takes out each month while the property is rented, after tax.")
        );
        let _ = writeln!(
            out,
            "    {}",
            st.dim("It is the same in both scenarios: after a sale, the same amounts go into or come out of the investment.")
        );
        // Aligned columns; bold only on the monthly amount.
        let rows: Vec<[String; 3]> = (0..h.cash.len()).map(|i| cash_phase(&h.cash, i)).collect();
        let width = |c: usize| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0);
        let (w0, w1) = (width(0), width(1));
        for [when, amount, rest] in &rows {
            let _ = writeln!(
                out,
                "    {}   {}   {rest}",
                pad(when, when.chars().count(), w0),
                pad(&st.bold(amount), amount.chars().count(), w1),
            );
        }
    }

    // The break-even return, worded from the winner's side.
    if let Some(v) = &h.verdict {
        let basis = if h.returns_before_tax {
            "before tax"
        } else {
            "after tax"
        };
        let sell = h.strategies[v.winner].kind == StrategyKind::SellNow;
        let line = match (v.break_even, v.break_even_bound) {
            (Some(b), _) => {
                let r = st.bold(&format!("{} a year", pct(b)));
                if v.tie {
                    format!("The break-even return is {r} ({basis}).")
                } else if sell {
                    format!(
                        "Selling keeps outperforming as long as the investment returns more than {r} ({basis})."
                    )
                } else {
                    format!(
                        "Selling outperforms only if the investment returns more than {r} ({basis})."
                    )
                }
            }
            (None, Some(Bound::Below)) => "Selling outperforms even with a poor investment.".into(),
            (None, _) => "Renting outperforms any realistic investment.".into(),
        };
        let _ = writeln!(out, "\n  {line}");
    }

    // What each strategy ends up with.
    if !h.strategies.is_empty() {
        let _ = writeln!(out, "\n  {}", st.cyan("Wealth at the end"));
        // The tax-free sale is a timing detail (the verdict and `--full` show it).
        let rows: Vec<&StrategyRow> = h
            .strategies
            .iter()
            .filter(|x| x.kind != StrategyKind::SellLastTaxFree)
            .collect();
        let labels: Vec<String> = rows.iter().map(|x| strategy_label(x)).collect();
        let width = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let max = h.strategies.iter().map(|x| x.value).fold(0.0, f64::max);
        for (x, label) in rows.iter().zip(&labels) {
            let b = bar(x.value, max);
            let b = pad(&st.cyan(&b), b.chars().count(), BAR_WIDTH as usize);
            let same = if x.about_same {
                st.dim("  ≈ same")
            } else {
                String::new()
            };
            let _ = writeln!(
                out,
                "    {label:width$}  {:>6}  {b}{same}",
                millions(Nok(x.value)),
            );
        }
    }

    // Also.
    let mut also = Vec::new();
    match (h.tax_free.status, &h.tax_free.month) {
        (TaxFreeStatus::Open, Some(m)) => {
            also.push(format!("Tax-free sale possible until {m} (keep a margin)."))
        }
        (TaxFreeStatus::Closed, Some(m)) => {
            also.push(format!("The tax-free sale window closed after {m}."))
        }
        _ => {}
    }
    if !also.is_empty() {
        let _ = writeln!(out, "\n  {}", st.cyan("Also"));
        for a in also {
            let _ = writeln!(out, "    {a}");
        }
    }

    for w in &h.warnings {
        let _ = writeln!(out, "\n  {}", st.yellow(&format!("⚠ {w}")));
    }
    let _ = writeln!(
        out,
        "\n  {}",
        st.dim("More: run --full (all tables, every sale month, other returns) · sensitivity · explain --option now|rent")
    );
    out
}
