# Architecture

How the code is organised and why. The calculation itself is described in [model.md](model.md).

## Platform decisions

| Decision | Why |
|---|---|
| **Engine in Rust**, a pure library (`crates/engine`) | Strong types suit the domain (loan kinds, ownership types, `Nok` vs `Rate`), `cargo test` and `proptest` suit financial logic, and one library serves the CLI and the browser. |
| **CLI** (`crates/cli`) | Runs real scenarios without a UI, prints every table and `explain`s any figure. |
| **WASM + thin React UI, static hosting** (`crates/wasm`, `web/`) | A full evaluation takes milliseconds, so no server is needed. Financial data stays in the browser. The UI only does forms, charts and storage; all logic stays in Rust, and TypeScript types are generated from the Rust types (`tsify`) so the two cannot drift. |
| **No backend** | Nothing to compute server-side. Only worth adding for accounts or cross-device sync; file export/import covers that. |
| **Not a spreadsheet** | Time paths and automated tests become unwieldy in one. CSV export gives spreadsheet-level inspection instead. |
| **Not a Rust UI framework** (Leptos, Dioxus, egui) | Their charting and form tooling is much less mature than React's. |
| **Charts are hand-drawn SVG**, no chart library (so far) | Plain SVG has covered every chart built (bands, direct labels, markers, hover). |

## Crates and data flow

```
Scenario (TOML / JSON) ──► evaluate(&Scenario, &TaxRules) ──► Evaluation ──► headline() ──► Headline
                                                                  │                              │
                                                  CLI --full tables, CSV, explain     CLI summary, WASM → web UI
```

- **`crates/engine`** — no I/O: it parses strings and returns data.
  - `evaluate` is the entry point. `Evaluation` holds the full results: monthly and yearly keep ledger, sale outcomes, the comparison rows, strategies, notes and warnings.
  - `Headline` (`headline.rs`) is the view model the CLI summary and the web UI share: plain numbers, months as `"YYYY-MM"` strings.
  - Modules:

    | Module | Contents |
    |---|---|
    | `money`, `time`, `timepath` | `Nok`, `Rate`, `YearMonth`, `TimePath` |
    | `scenario` | the scenario types (serde, validation) |
    | `tax_rules` | versioned tax parameters, `TaxRules::for_year` |
    | `loan`, `keep`, `ledger` | loan schedules, the monthly keep simulation, calendar-year roll-up and rental tax |
    | `sale` | a sale in any month: gain, tax-free rule, equity |
    | `wealth` | wealth-tax valuation and marginal tax |
    | `alternative` | the investment account, cash-flow parity, comparison rows, break-even solver |
    | `strategies` | every sale month valued at the horizon, equivalent returns, return sweep |
    | `sensitivity` | one-at-a-time bumps |
    | `explain` | the `Explained` tree behind any figure |
    | `assumptions` | the simplifications, grouped and filtered to the scenario |
    | `export` | tables for CSV and the golden tests |
    | `headline` | the shared view model |

  - `assumptions(&Scenario)` lists the model's simplifications in plain words, grouped by topic and filtered to what applies (wealth tax only when it's on, borettslag only for one); every output shows them, via `Headline.assumptions`.
  - Example scenarios in `crates/engine/scenarios/` are compiled in (`examples::BASIC`, `examples::BORETTSLAG`).
- **`crates/cli`** — `run` (summary, or `--full` / `--monthly` tables, `--csv`), `sensitivity`, `explain`. The summary renders only from `Headline`.
- **`crates/wasm`** — `parseScenario`, `evaluateScenario` (→ `Headline`), `scenarioToToml`, `exampleScenario`; `wasm-bindgen` with `tsify::Ts`.
- **`web/`** — Vite + React + TypeScript. `npm run wasm` builds the engine into `web/src/wasm-pkg` (git-ignored).

## Engine conventions

- **Monthly simulation, yearly view.** Loans, vacancy and rate changes run per month and roll up to **calendar years**, because Norwegian tax is per calendar year. The first and last years can be partial.
- **Money as `f64` newtypes** — `Nok(f64)` and `Rate(f64)`, with only meaningful operations. Results are projections of an uncertain future, so exact decimals add nothing, and `f64` keeps the annuity formula and the solvers simple. Rounding happens only for display.
- **Time** — `YearMonth` (`"YYYY-MM"`). `TimePath` (`constant`, `steps`, `linear`) is anything that changes over time, resolved to one value per month.

## Scenarios

- A **TOML file** that maps directly onto Rust types via `serde`; the web UI uses the same types as JSON. `crates/engine/scenarios/basic.toml` documents every field.
- `schema_version`, so old files can be migrated.
- `deny_unknown_fields` everywhere: a typo is an error, never a silent default.

## Tax rules as data

- **Parameter values are data**: `crates/engine/tax-rules/<year>.toml`, one per income year. Every parameter has `value`, `source` and `verified_on`. A year without its own file uses the latest earlier file, and the output says so. `--rules` loads your own files.
- **How a rule works is code**, gated on `income_year` where the law changes a rule's structure.

## Transparency

- Full monthly (`MonthRow`) and yearly (`YearRow`) rows with every line item; CSV exports use the same tables.
- `rent-or-sell explain` prints the `Explained` tree (`label`, `value`, `formula`, `source`, `parts`) for a sale, a comparison row or a year's wealth tax, down to the tax-rule keys used. The web UI does not show it.

## Testing

- **Unit tests** against closed-form or independently computed values. Key figures (annuity, a sale, sell-now → invest, wealth tax, the borettslag example) were cross-checked in Python.
- **Hand-worked examples** with the arithmetic in comments (`tests/tax.rs`, `tests/wealth.rs`, `tests/borettslag.rs`).
- **Property tests** (`proptest`), e.g. the loan balance never goes negative, a higher rate never helps, sell outcomes rise with the return.
- **Identity tests** that tie outputs together, e.g. every strategy's value equals the matching comparison row, and investing at renting's equivalent return ties with renting.
- **Golden snapshots** (`insta`) of the example scenarios' tables.
