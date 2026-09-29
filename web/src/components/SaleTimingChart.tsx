import { useState } from "react";
import type { Headline, SalePoint } from "../engine";
import { grouped, millions, signed } from "../format";
import { useTweened } from "../useTweened";
import { STRATEGY_COLOR, nearest, spread, strategyLabel, svgX, ticks } from "./chartKit";

const W = 940;
const H = 320;
const PAD = { left: 64, right: 24, top: 28, bottom: 44 };

/**
 * Rent until a month, then sell and invest: the wealth each sale month gives at
 * the horizon, compared with selling now. Above zero, renting first wins.
 */
export default function SaleTimingChart({ h }: { h: Headline }) {
  const [hover, setHover] = useState<number | null>(null);
  const pts = h.by_sale;
  // Drawn from the values as shown, so the curve glides to a new result.
  const vals = useTweened(pts.map((p) => p.vs_sell_now));
  if (pts.length < 2) return null;

  const n = pts[pts.length - 1].offset;
  const shown = (p: SalePoint) => vals[pts.indexOf(p)] ?? p.vs_sell_now;
  const yTicks = ticks(Math.min(0, ...vals), Math.max(0, ...vals));
  const [lo, hi] = [yTicks[0], yTicks[yTicks.length - 1]];
  const x = (offset: number) => PAD.left + ((W - PAD.left - PAD.right) * offset) / n;
  const y = (v: number) => H - PAD.bottom - ((v - lo) / (hi - lo || 1)) * (H - PAD.top - PAD.bottom);
  const zero = y(0);

  const line = pts.map((p) => `${x(p.offset)},${y(shown(p))}`).join(" ");
  const area = `M${x(0)},${zero} L${line.replaceAll(" ", " L")} L${x(n)},${zero} Z`;

  // The tax-free months (contiguous from now when the window is open).
  const free = pts.filter((p) => p.tax_free && p.offset < n);
  const freeEnd = free.length > 0 ? free[free.length - 1].offset : null;

  // Year labels on Januaries, thinned to about a dozen.
  const januaries = pts.filter((p) => p.month.endsWith("-01"));
  const every = Math.ceil(januaries.length / 12);

  const pointOf = (month: string) => pts.find((q) => q.month === month) ?? pts[pts.length - 1];
  // The tax-free sale is a dashed marker, not a labelled dot.
  const marks = h.strategies
    .filter((s) => s.kind !== "sell_last_tax_free")
    .map((s) => ({ s, p: pointOf(s.sale_month) }));
  const taxFree = h.strategies.find((s) => s.kind === "sell_last_tax_free");
  const taxFreePoint = taxFree && pointOf(taxFree.sale_month);
  // Labels above their dot (selling now's below the zero line), pushed apart
  // vertically where they would collide.
  const labelYs = spread(
    marks.map(({ s, p }) => (s.kind === "sell_now" ? zero + 18 : y(shown(p)) - 12)),
    18,
    H - PAD.bottom - 4,
  );
  // Thousands up to a million, then millions.
  const big = Math.max(Math.abs(lo), Math.abs(hi)) >= 1e6;
  const tick = (t: number) => {
    if (t === 0) return "0";
    const sign = t > 0 ? "+" : "−";
    return big ? `${sign}${(Math.abs(t) / 1e6).toFixed(1)}M` : `${sign}${grouped(Math.abs(t) / 1000)}k`;
  };

  const hp = hover !== null ? pts[hover] : null;
  const summary = `Wealth at the end by sale month, compared with selling now: ${marks
    .map(({ s, p }) => `${strategyLabel(s)} ${signed(p.vs_sell_now)} kr`)
    .join(", ")}.`;

  return (
    <figure className="chart">
      <figcaption className="chart-title">Sale timing</figcaption>
      <p className="muted small">
        Wealth at the end for each sale month, compared with selling now.
      </p>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={summary}
        onMouseMove={(e) => setHover(nearest(pts.map((p) => x(p.offset)), svgX(e, W)))}
        onMouseLeave={() => setHover(null)}
      >
        <defs>
          <clipPath id="sale-above">
            <rect x={0} y={0} width={W} height={zero} />
          </clipPath>
          <clipPath id="sale-below">
            <rect x={0} y={zero} width={W} height={H} />
          </clipPath>
        </defs>
        {freeEnd !== null && (
          <g>
            <rect className="band" x={x(0)} y={PAD.top} width={x(freeEnd) - x(0)} height={H - PAD.top - PAD.bottom} />
            {!taxFreePoint && (
              <text className="annotation" x={x(0) + 6} y={PAD.top - 8}>
                Tax-free sale until {pts[freeEnd].month}
              </text>
            )}
          </g>
        )}
        {taxFreePoint && (
          <g>
            <line
              className="marker"
              x1={x(taxFreePoint.offset)}
              x2={x(taxFreePoint.offset)}
              y1={PAD.top}
              y2={H - PAD.bottom}
            />
            <text className="annotation" x={x(taxFreePoint.offset) + 6} y={PAD.top - 8}>
              Last tax-free sale {taxFreePoint.month}: {signed(shown(taxFreePoint))}
            </text>
          </g>
        )}
        {yTicks.map((t) => (
          <g key={t}>
            <line className="grid" x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} />
            <text className="tick" x={PAD.left - 8} y={y(t) + 4} textAnchor="end">
              {tick(t)}
            </text>
          </g>
        ))}
        {januaries.map((p, i) =>
          i % every === 0 ? (
            <text key={p.offset} className="tick" x={x(p.offset)} y={H - PAD.bottom + 18} textAnchor="middle">
              {p.month.slice(0, 4)}
            </text>
          ) : null,
        )}
        <text className="axis-title" x={(PAD.left + W - PAD.right) / 2} y={H - 6} textAnchor="middle">
          sale month; kr compared with selling now
        </text>
        {h.loan_paid_off && h.loan_paid_off.offset < n && (
          <g>
            <line className="marker" x1={x(h.loan_paid_off.offset)} x2={x(h.loan_paid_off.offset)} y1={PAD.top} y2={H - PAD.bottom} />
            <text className="annotation" x={x(h.loan_paid_off.offset) + 6} y={PAD.top + 12}>
              Loan paid off {h.loan_paid_off.month}
            </text>
          </g>
        )}
        <path d={area} className="area-better" clipPath="url(#sale-above)" />
        <path d={area} className="area-worse" clipPath="url(#sale-below)" />
        <line className="zero" x1={PAD.left} x2={W - PAD.right} y1={zero} y2={zero} />
        <polyline className="series" points={line} />
        {marks.map(({ s, p }, i) => (
          <g key={s.kind}>
            <circle className="dot" cx={x(p.offset)} cy={y(shown(p))} r={5} fill={STRATEGY_COLOR[s.kind]} />
            <text
              className="point-label"
              x={x(p.offset)}
              y={labelYs[i]}
              textAnchor={p.offset === 0 ? "start" : p.offset === n ? "end" : "middle"}
            >
              {s.kind === "sell_now" ? "Sell now" : `${strategyLabel(s)}: ${signed(shown(p))}`}
            </text>
          </g>
        ))}
        {hp && (
          <g pointerEvents="none">
            <line className="hover-line" x1={x(hp.offset)} x2={x(hp.offset)} y1={PAD.top} y2={H - PAD.bottom} />
            <circle className="dot" cx={x(hp.offset)} cy={y(shown(hp))} r={4} fill="var(--ink-2)" />
            <g transform={`translate(${x(hp.offset) > W - 300 ? x(hp.offset) - 270 : x(hp.offset) + 12}, ${PAD.top + 8})`}>
              <rect className="tooltip" width={258} height={80} rx={6} />
              <text className="tooltip-title" x={10} y={20}>
                {hp.offset === n ? "Renting" : hp.offset === 0 ? "Sell now" : `Rent, sell in ${hp.month}`}
              </text>
              <text className="tooltip-row" x={10} y={40}>
                Wealth at the end: {millions(hp.value)}
              </text>
              <text className="tooltip-row" x={10} y={58}>
                vs selling now: {signed(hp.vs_sell_now)} kr
              </text>
              <text className="tooltip-row muted-fill" x={10} y={74}>
                {hp.offset === n ? "not sold" : hp.tax_free ? "tax-free sale" : "gain taxed"}
              </text>
            </g>
          </g>
        )}
      </svg>
    </figure>
  );
}
