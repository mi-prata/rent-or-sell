# AGENTS.md

A Norwegian rent-or-sell calculator: a pure Rust engine, a CLI, and a static React page running the engine as WebAssembly. Read the doc for the area before changing it:

- [README.md](README.md): what it answers, CLI and web usage
- [docs/architecture.md](docs/architecture.md): crates, data flow, engine conventions, testing
- [docs/model.md](docs/model.md): what is calculated and how (tax, sale, parity, strategies, wealth tax)
- [docs/roadmap.md](docs/roadmap.md): later ideas and open modelling questions
- [crates/engine/scenarios/basic.toml](crates/engine/scenarios/basic.toml): every scenario field, documented

## Setup

- Rust tools live in `~/.cargo/bin`; put it on PATH (`wasm-pack` too).
- After any engine change, run `npm run wasm` in `web/`, or the page keeps running the old engine.
- CLI: `cargo run -q --bin rent-or-sell -- run crates/engine/scenarios/basic.toml [--full]`. Web: `cd web && npm run dev` (http://localhost:5173).

## Before calling a change done

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cd web && npm run typecheck
```

Golden snapshots (`insta`) change only on purpose: review the diff, then `INSTA_UPDATE=always cargo test`.

## Rules

- All calculation lives in `crates/engine` (no I/O). The CLI summary and the web page render only `Headline` (`headline.rs`); TypeScript types are generated from Rust (`tsify`), never hand-written.
- Tax parameters are data in `crates/engine/tax-rules/<year>.toml`, each with `source` and `verified_on`; how a rule works is code. If the law changes a rule's structure, add code gated on `income_year` rather than bending the parameters.
- Scenarios use `deny_unknown_fields`: a new field goes in the Rust type, the example TOMLs and the web form.
- `scenarios/private/` holds real financial data: never commit, quote or copy it elsewhere.
- Model simplifications a reader should know go in `assumptions.rs` (plain words, grouped, only what applies).

## Wording (UI and CLI text)

- Impersonal: no "you"/"your"; name the value instead. The one exception is the web page's intro question ("Should you rent out…").
- "Rent" and "renting", not "keep"; "property", not "flat"; "wealth", not "net worth".
- No space before %: "6.94%", "+5.4%".
- The period is stated once (top of the page, CLI title line); elsewhere say "at the end", except where the number is part of the claim ("Renting for 15 years: …").

## Docs and git

- Docs describe the current state; edit them in place alongside the change. No plan, slice or log files, and no design history (git has it). Rules for how to work belong here, not in `docs/`.
- Commit on `main` with a message that says why; never push unless asked (a push to `main` publishes the page to GitHub Pages).
