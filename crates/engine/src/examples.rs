//! Example scenarios shipped with the engine (fictional numbers). They
//! document every field and are the starting point for your own.

/// A selveier flat with a loan, an investment and wealth tax.
pub const BASIC: &str = include_str!("../scenarios/basic.toml");

/// A borettslag unit with fellesgjeld and an interest-only period.
pub const BORETTSLAG: &str = include_str!("../scenarios/borettslag.toml");
