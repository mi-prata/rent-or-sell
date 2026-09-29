// Helpers for editing a Scenario (the engine's input, typed from Rust).
import type { Fellesgjeld, Invest, Loan, Savings, Scenario, TimePath, Wealth } from "./engine";

/** A copy of `s` with `edit` applied (plain objects, so a deep clone is cheap). */
export function produce(s: Scenario, edit: (draft: Scenario) => void): Scenario {
  const draft = structuredClone(s);
  edit(draft);
  return draft;
}

/** "2026-10" + months. */
export function addMonths(ym: string, months: number): string {
  const [y, m] = ym.split("-").map(Number);
  const total = y * 12 + (m - 1) + months;
  return `${Math.floor(total / 12)}-${String((total % 12) + 1).padStart(2, "0")}`;
}

/** The single value of a constant path; `undefined` when it varies over time. */
export function constantOf(tp: TimePath | undefined): number | undefined {
  if (tp === undefined) return 0;
  return "constant" in tp ? tp.constant : undefined;
}

/** Number of points of a path that varies over time. */
export function pointsOf(tp: TimePath | undefined): number {
  if (tp === undefined || "constant" in tp) return 0;
  return "steps" in tp ? tp.steps.length : tp.linear.length;
}

/** The first value of any path, to start a single value from. */
export function firstValueOf(tp: TimePath | undefined): number {
  if (tp === undefined) return 0;
  if ("constant" in tp) return tp.constant;
  const points = "steps" in tp ? tp.steps : tp.linear;
  return points[0]?.value ?? 0;
}

// Defaults used when an optional section is switched on.
export const newLoan = (s: Scenario): Loan => ({
  balance: 2_000_000,
  kind: "annuity",
  end: addMonths(s.start, 25 * 12 - 1),
  monthly_fee: 50,
  nominal_rate: { constant: 0.05 },
  interest_only_until: undefined,
});

export const newFellesgjeld = (s: Scenario): Fellesgjeld => ({
  balance: 0,
  at_purchase: undefined,
  kind: "annuity",
  end: addMonths(s.start, 30 * 12 - 1),
  nominal_rate: { constant: 0.05 },
  interest_only_until: undefined,
});

/** The investment a scenario without one gets: the page always shows a return to edit. */
export const newInvest = (): Invest => ({
  annual_return: { constant: 0.06 },
  tax: "deferred",
  equity_share: 1,
});

export const newSavings = (): Savings => ({ amount: 100_000, at: undefined });

export const newWealth = (): Wealth => ({
  persons: 1,
  other_net_wealth: { constant: 0 },
  assessed_market_value: undefined,
});

export type SectionKey =
  | "general"
  | "property"
  | "fellesgjeld"
  | "loan"
  | "rental"
  | "costs"
  | "sale"
  | "owner"
  | "investment"
  | "savings"
  | "wealth";

/**
 * Which form section an engine error belongs to. The engine's messages name
 * the field ("loan.end is before start", "cost \"insurance\": …").
 */
export function sectionOfError(message: string): SectionKey {
  const m = message.toLowerCase();
  const rules: [RegExp, SectionKey][] = [
    [/fellesgjeld/, "fellesgjeld"],
    [/\bloan\./, "loan"],
    [/\bproperty\.|improvement/, "property"],
    [/\brental\./, "rental"],
    [/\bcosts?\b|\bcost "/, "costs"],
    [/\bsale\./, "sale"],
    [/\bowner\b|moved_(in|out)/, "owner"],
    [/\bsavings/, "savings"],
    [/alternative\.invest|\binvest/, "investment"],
    [/\bwealth\./, "wealth"],
  ];
  return rules.find(([re]) => re.test(m))?.[1] ?? "general";
}
