//! The engine for the browser. The web UI edits a `Scenario` object (its
//! TypeScript type is generated with `tsify`), evaluates it into the
//! `Headline` view model, and imports/exports the same TOML files the CLI
//! reads.

use rent_or_sell_engine::{Headline, Scenario, TaxRules, evaluate, examples, headline};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// The example scenario shipped with the engine, as TOML.
#[wasm_bindgen(js_name = exampleScenario)]
pub fn example_scenario() -> String {
    examples::BASIC.to_string()
}

/// Reads a scenario file (TOML), validating it.
#[wasm_bindgen(js_name = parseScenario)]
pub fn parse_scenario(toml: &str) -> Result<Ts<Scenario>, JsError> {
    let scenario = Scenario::from_toml_str(toml).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(scenario.into_ts()?)
}

/// Writes a scenario as TOML, for download.
#[wasm_bindgen(js_name = scenarioToToml)]
pub fn scenario_to_toml(scenario: Ts<Scenario>) -> Result<String, JsError> {
    let scenario = scenario.to_rust()?;
    scenario
        .to_toml_string()
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Validates and evaluates a scenario with the built-in tax rules.
#[wasm_bindgen(js_name = evaluateScenario)]
pub fn evaluate_scenario(scenario: Ts<Scenario>) -> Result<Ts<Headline>, JsError> {
    let h = run(&scenario.to_rust()?).map_err(|e| JsError::new(&e))?;
    Ok(h.into_ts()?)
}

/// Plain-Rust core of [`evaluate_scenario`], so it can be tested natively.
pub fn run(scenario: &Scenario) -> Result<Headline, String> {
    scenario.validate().map_err(|e| e.to_string())?;
    let evaluation = evaluate(scenario, &TaxRules::builtin()).map_err(|e| e.to_string())?;
    Ok(headline(scenario, &evaluation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_round_trip_through_toml() {
        for toml in [examples::BASIC, examples::BORETTSLAG] {
            let a = Scenario::from_toml_str(toml).unwrap();
            let b = Scenario::from_toml_str(&a.to_toml_string().unwrap()).unwrap();
            assert_eq!(run(&a).unwrap(), run(&b).unwrap());
        }
    }

    #[test]
    fn invalid_scenario_is_an_error() {
        let mut s = Scenario::from_toml_str(examples::BASIC).unwrap();
        s.horizon_years = 0;
        assert!(run(&s).is_err());
    }
}
