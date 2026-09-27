//! Golden snapshots of the example scenario's yearly ledger and sale options. If a change moves
//! these numbers on purpose, review the diff and update with
//! `INSTA_UPDATE=always cargo test` (or `cargo insta review`).

mod common;

use rent_or_sell_engine::export::{comparison_table, options_table, yearly_table};
use rent_or_sell_engine::{Scenario, TaxRules, evaluate};

#[test]
fn basic_example_yearly_ledger() {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    let scenario = Scenario::from_toml_str(&toml).unwrap();
    let evaluation = evaluate(&scenario, &TaxRules::builtin()).unwrap();
    insta::assert_snapshot!(yearly_table(&evaluation).to_plain_csv());
}

#[test]
fn basic_example_sale_options() {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    let scenario = Scenario::from_toml_str(&toml).unwrap();
    let evaluation = evaluate(&scenario, &TaxRules::builtin()).unwrap();
    insta::assert_snapshot!(options_table(&evaluation).to_plain_csv());
}

#[test]
fn basic_example_keep_vs_sell() {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    let scenario = Scenario::from_toml_str(&toml).unwrap();
    let evaluation = evaluate(&scenario, &TaxRules::builtin()).unwrap();
    insta::assert_snapshot!(comparison_table(&evaluation).to_plain_csv());
}

#[test]
fn borettslag_example() {
    let toml = rent_or_sell_engine::examples::BORETTSLAG.to_string();
    let scenario = Scenario::from_toml_str(&toml).unwrap();
    let evaluation = evaluate(&scenario, &TaxRules::builtin()).unwrap();
    let all = [
        yearly_table(&evaluation),
        options_table(&evaluation),
        comparison_table(&evaluation),
    ]
    .map(|t| t.to_plain_csv())
    .join("\n");
    insta::assert_snapshot!(all);
}

#[test]
fn basic_example_sensitivity() {
    let toml = rent_or_sell_engine::examples::BASIC.to_string();
    let scenario = Scenario::from_toml_str(&toml).unwrap();
    let rows = rent_or_sell_engine::sensitivity(&scenario, &TaxRules::builtin()).unwrap();
    insta::assert_snapshot!(rent_or_sell_engine::export::sensitivity_table(&rows).to_plain_csv());
}
