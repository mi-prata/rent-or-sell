// Shared pieces of the hand-drawn SVG charts.
import type { StrategyKind, StrategyRow } from "../engine";

/** About `count` round tick values covering [lo, hi]. */
export function ticks(lo: number, hi: number, count = 5): number[] {
  const raw = (hi - lo) / count || 1;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw;
  // From the step at or below `lo` to the step at or above `hi`, so no line
  // leaves the plot.
  const first = Math.floor(lo / step);
  const last = Math.ceil(hi / step);
  const out = [];
  for (let k = first; k <= last; k++) out.push(k * step);
  return out;
}

/**
 * Places labels at their preferred heights, pushed apart by `gap` so they
 * don't overlap, and shifted up if the lowest would fall below `max`.
 */
export function spread(ys: number[], gap: number, max: number): number[] {
  const order = ys.map((y, i) => [y, i] as const).sort((a, b) => a[0] - b[0]);
  const out = new Array<number>(ys.length);
  let last = -Infinity;
  for (const [y, i] of order) {
    last = Math.max(y, last + gap);
    out[i] = last;
  }
  const overflow = last - max;
  return overflow > 0 ? out.map((y) => y - overflow) : out;
}

/** Index of the x closest to `px`. */
export function nearest(xs: number[], px: number): number {
  let best = 0;
  xs.forEach((x, i) => {
    if (Math.abs(x - px) < Math.abs(xs[best] - px)) best = i;
  });
  return best;
}

/** The pointer's x in the SVG's own coordinates (the SVG scales to its box). */
export function svgX(e: React.MouseEvent<SVGSVGElement>, width: number): number {
  const box = e.currentTarget.getBoundingClientRect();
  return ((e.clientX - box.left) / box.width) * width;
}

/** One fixed colour per strategy, the same in the table and both charts. */
export const STRATEGY_COLOR: Record<StrategyKind, string> = {
  sell_now: "var(--sell)",
  sell_last_tax_free: "var(--taxfree)",
  sell_best: "var(--best)",
  rent: "var(--rent)",
};

/** "Selling and investing", "Rent, sell in 2027-07", "Renting" (the whole period). */
export function strategyLabel(s: StrategyRow): string {
  switch (s.kind) {
    case "sell_now":
      return "Selling and investing";
    case "rent":
      return "Renting";
    default:
      return `Rent, sell in ${s.sale_month}`;
  }
}

/** Why a sale month is one of the strategies. */
export function strategyNote(s: StrategyRow): string | undefined {
  if (s.kind === "sell_last_tax_free") return "last tax-free month";
  if (s.kind === "sell_best") return "best month to sell";
  return undefined;
}
