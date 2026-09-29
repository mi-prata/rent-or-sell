// Number formatting in the CLI's style: space-grouped thousands, a real
// minus sign, millions with two decimals.

const MINUS = "−";

/** 1234567 → "1 234 567"; negatives get a minus sign. */
export function grouped(n: number): string {
  const s = Math.round(Math.abs(n))
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return n < -0.5 ? MINUS + s : s;
}

/** 1234567 → "1.23M". */
export function millions(n: number): string {
  const s = (Math.abs(n) / 1e6).toFixed(2) + "M";
  return n < 0 ? MINUS + s : s;
}

/** "1.23M kr" from a million up, else "63 321 kr". */
export function kr(n: number): string {
  return Math.abs(n) >= 1e6 ? `${millions(n)} kr` : `${grouped(n)} kr`;
}

/** 0.0468 → "4.68%". */
export function pct(r: number): string {
  return `${(r * 100).toFixed(2)}%`;
}

/** A rate as short as it can be: 0.06 → "6%", 0.085 → "8.5%". */
export function rate(r: number): string {
  return `${Number((r * 100).toFixed(2))}%`;
}

/** 0.054 → "+5.4%": a difference as a share. */
export function share(x: number): string {
  return `${x < 0 ? MINUS : "+"}${(Math.abs(x) * 100).toFixed(1)}%`;
}

/** 15 → "15 years", 1 → "1 year". */
export function years(n: number): string {
  return `${n} year${n === 1 ? "" : "s"}`;
}

/** With an explicit sign: "+9 620", "−140 458". */
export function signed(n: number): string {
  return n > 0.5 ? `+${grouped(n)}` : grouped(n);
}
