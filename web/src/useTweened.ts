import { useEffect, useRef, useState } from "react";

/** How long a recompute takes to settle; matches `--settle` in styles.css. */
export const SETTLE_MS = 650;

const easeOut = (t: number) => 1 - (1 - t) ** 3;

/**
 * The values as shown: after a change they glide from what is on screen to
 * the new values over `SETTLE_MS`. A change in length, or reduced motion,
 * shows the new values at once.
 */
export function useTweened(target: number[]): number[] {
  const [shown, setShown] = useState(target);
  const shownRef = useRef(target);
  const key = target.join(",");

  useEffect(() => {
    const from = shownRef.current;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    const show = (v: number[]) => {
      shownRef.current = v;
      setShown(v);
    };
    if (reduce || from.length !== target.length) {
      show(target);
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / SETTLE_MS);
      const e = easeOut(t);
      show(target.map((v, i) => from[i] + (v - from[i]) * e));
      if (t < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
    // `key` stands for `target`'s contents: a new array with the same values is no change.
  }, [key]);

  return shown;
}
