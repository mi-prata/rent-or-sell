# rent-or-sell

Should you rent out a Norwegian residential property or sell it and invest the money? This tool compares the two over a chosen period, after Norwegian tax and sale costs.

Runs as a static page, try it [here](https://mi-prata.github.io/rent-or-sell/).

## What it answers

- **Which wins:** selling now, renting for the whole period, or renting and then selling in a given month (including the last tax-free month). Wealth is compared at the same end date.
- **The break-even return:** what the investment must earn for selling to win.
- **Monthly cash flow** while the property is rented, after tax.
- **Sensitivity** to interest rates, house prices, rent, vacancy and costs, one change at a time.

It covers rental tax, the tax-free sale rule, gain tax, investment tax (yearly or deferred, ASK-style), wealth tax, and borettslag units with fellesgjeld. Tax parameters are versioned data with sources: [`crates/engine/tax-rules/`](crates/engine/tax-rules/). The model is described in [docs/model.md](docs/model.md).

## CLI

```sh
cargo run -q --bin rent-or-sell -- run crates/engine/scenarios/basic.toml           # summary
cargo run -q --bin rent-or-sell -- run crates/engine/scenarios/basic.toml --full    # every table (--monthly, --csv out.csv)
cargo run -q --bin rent-or-sell -- sensitivity crates/engine/scenarios/basic.toml   # what flips the decision
cargo run -q --bin rent-or-sell -- explain crates/engine/scenarios/basic.toml --at 2030-01   # how a figure is derived
```

A scenario is a TOML file. [`basic.toml`](crates/engine/scenarios/basic.toml) documents every field, and [`borettslag.toml`](crates/engine/scenarios/borettslag.toml) is a borettslag example. Put real scenarios in `scenarios/private/`, which git ignores.

## Web page

Requires Node, `wasm-pack` and the `wasm32-unknown-unknown` target.

```sh
cd web && npm install
npm run wasm   # build the engine (again after any engine change)
npm run dev    # http://localhost:5173
```

## Docs

- [architecture.md](docs/architecture.md): crates, data flow, conventions, testing
- [model.md](docs/model.md): what is calculated and how
- [roadmap.md](docs/roadmap.md): later ideas and open modelling questions

## Disclaimer

This is a planning aid, not tax or financial advice. Check the tax parameters against [skatteetaten.no](https://www.skatteetaten.no) before relying on the results; the unconfirmed rules are listed at the end of [model.md](docs/model.md).
