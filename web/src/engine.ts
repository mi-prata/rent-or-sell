// The Rust engine, compiled to WebAssembly (see crates/wasm). `npm run wasm`
// rebuilds `wasm-pkg/` after engine changes.
import init, {
  evaluateScenario,
  exampleScenario,
  parseScenario,
  scenarioToToml,
} from "./wasm-pkg/rent_or_sell_wasm";
import type { Headline, Scenario } from "./wasm-pkg/rent_or_sell_wasm";

export type {
  CashPhase,
  Cost,
  Fellesgjeld,
  Headline,
  Invest,
  InvestTax,
  Loan,
  LoanKind,
  NamedCost,
  SalePoint,
  Savings,
  Scenario,
  StrategyKind,
  StrategyRow,
  TimePath,
  Wealth,
} from "./wasm-pkg/rent_or_sell_wasm";

export type Result<T> = { ok: true; value: T } | { ok: false; error: string };

function attempt<T>(f: () => T): Result<T> {
  try {
    return { ok: true, value: f() };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** Loads the WebAssembly module; call once before anything else. */
export async function loadEngine(): Promise<void> {
  await init();
}

export const evaluate = (s: Scenario): Result<Headline> => attempt(() => evaluateScenario(s));
export const parse = (toml: string): Result<Scenario> => attempt(() => parseScenario(toml));
export const toToml = (s: Scenario): Result<string> => attempt(() => scenarioToToml(s));
export const example = (): Scenario => parseScenario(exampleScenario());

