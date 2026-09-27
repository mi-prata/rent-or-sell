//! How a number was derived: a tree of labelled values and formulas.

use serde::Serialize;

use crate::money::Nok;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Explained {
    pub label: String,
    /// `None` for purely informational nodes (e.g. a rule check).
    pub value: Option<Nok>,
    /// How the value follows from `parts` or from inputs, e.g. "A − B".
    pub formula: Option<String>,
    /// Tax rule reference, e.g. "tax-rules 2026: capital_income_rate".
    pub source: Option<String>,
    pub parts: Vec<Explained>,
}

impl Explained {
    pub fn value(label: impl Into<String>, value: Nok) -> Explained {
        Explained {
            label: label.into(),
            value: Some(value),
            formula: None,
            source: None,
            parts: Vec::new(),
        }
    }

    pub fn info(label: impl Into<String>) -> Explained {
        Explained {
            label: label.into(),
            value: None,
            formula: None,
            source: None,
            parts: Vec::new(),
        }
    }

    pub fn formula(mut self, formula: impl Into<String>) -> Explained {
        self.formula = Some(formula.into());
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Explained {
        self.source = Some(source.into());
        self
    }

    pub fn parts(mut self, parts: Vec<Explained>) -> Explained {
        self.parts = parts;
        self
    }

    /// Indented plain-text rendering, one node per line.
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out, "", "");
        out
    }

    fn render_into(&self, out: &mut String, first_prefix: &str, rest_prefix: &str) {
        out.push_str(first_prefix);
        out.push_str(&self.label);
        if let Some(v) = self.value {
            out.push_str(&format!(": {v}"));
        }
        if let Some(f) = &self.formula {
            out.push_str(&format!("  [{f}]"));
        }
        if let Some(s) = &self.source {
            out.push_str(&format!("  ({s})"));
        }
        out.push('\n');
        for (i, part) in self.parts.iter().enumerate() {
            let last = i + 1 == self.parts.len();
            let (branch, cont) = if last {
                ("└─ ", "   ")
            } else {
                ("├─ ", "│  ")
            };
            part.render_into(
                out,
                &format!("{rest_prefix}{branch}"),
                &format!("{rest_prefix}{cont}"),
            );
        }
    }
}
