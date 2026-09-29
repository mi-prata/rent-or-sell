import type { Headline, Scenario } from "../engine";
import ErrorBoundary from "./ErrorBoundary";
import SaleTimingChart from "./SaleTimingChart";
import { TimePathField } from "../form/fields";
import { newInvest } from "../scenario";
import { grouped, kr, millions, pct, share, years } from "../format";
import { useTweened } from "../useTweened";
import { strategyNote } from "./chartKit";

type Props = {
  h: Headline;
  scenario: Scenario;
  stale: boolean;
  /** Edits the scenario (the results own the investment's return). */
  edit: (f: (d: Scenario) => void) => void;
  /** An engine error about the investment, shown with its return. */
  investmentError?: string;
};

export default function Results({ h, scenario, stale, edit, investmentError }: Props) {
  return (
    <div className={stale ? "results-inner stale" : "results-inner"}>
      {stale && (
        <p className="stale-note">Showing the last valid result while the scenario has an error.</p>
      )}
      <p className="results-meta">{period(h)}</p>
      <Comparison h={h} scenario={scenario} edit={edit} error={investmentError} />
      <CashFlow h={h} />
      <ErrorBoundary what="chart">
        <SaleTimingChart h={h} />
      </ErrorBoundary>
      {h.warnings.map((w) => (
        <p key={w} className="warning">
          {w}
        </p>
      ))}
      <Assumptions h={h} />
    </div>
  );
}

/** The scenario's name and period: the one place the dates are shown; everything below is valued at the end. */
function period(h: Headline) {
  return [h.name.trim(), `${years(h.horizon_years)}, ${h.start} to ${h.horizon}`].filter(Boolean).join(": ");
}

/**
 * Selling and renting side by side, each with its wealth at the end; the
 * balance between them and the verdict below. Selling's side holds the one
 * investment input on the page, its return.
 */
function Comparison({
  h,
  scenario,
  edit,
  error,
}: {
  h: Headline;
  scenario: Scenario;
  edit: (f: (d: Scenario) => void) => void;
  error?: string;
}) {
  const sellNow = h.strategies.find((s) => s.kind === "sell_now");
  const renting = h.strategies.find((s) => s.kind === "rent");
  const v = h.verdict;
  const winner = v && !v.tie ? h.strategies[v.winner].kind : undefined;
  const [sellValue, rentValue] = useTweened([sellNow?.value ?? 0, renting?.value ?? 0]);
  const invest = scenario.alternative?.invest;
  const basis = h.returns_before_tax ? "before tax" : "after tax";
  const tax =
    invest?.tax === "deferred"
      ? "taxed when cashed out, like an ASK"
      : invest?.tax === "yearly"
        ? "taxed every year"
        : "entered after tax";
  return (
    <section className="comparison" aria-label="Selling or renting">
      <div className="sides">
        <div className={winner === "sell_now" ? "side side-sell winner" : "side side-sell"}>
          <h2>Selling</h2>
          <div className="side-text">
            <p>
              Sell today and invest the {millions(h.proceeds_now)} NOK it frees at{" "}
              <TimePathField
                inline
                label={`Investment yearly return, ${basis}`}
                value={invest?.annual_return}
                onChange={(r) =>
                  edit((d) => {
                    d.alternative = {
                      ...d.alternative,
                      invest: { ...(d.alternative?.invest ?? newInvest()), annual_return: r },
                    };
                  })
                }
              />{" "}
              a year {basis}.
            </p>
            <p className="side-note">After sale costs, loans and tax; {tax}.</p>
          </div>
          {sellNow && <Total value={sellValue} />}
        </div>
        <div className={winner === "rent" ? "side side-rent winner" : "side side-rent"}>
          <h2>Renting</h2>
          <div className="side-text">
            <p>Rent the property out for the whole period; the equity stays in the property.</p>
          </div>
          {renting && <Total value={rentValue} />}
        </div>
      </div>
      <div className="balance">
        {error && (
          <p className="field-error" role="alert">
            {error}
          </p>
        )}
        {v && <Balance h={h} />}
        <Verdict h={h} />
        {v && <p className="muted small parity">The model assumes cash-flow parity for both investments (see Assumptions).</p>}
      </div>
    </section>
  );
}

function Total({ value }: { value: number }) {
  return (
    <div className="total">
      <span className="num">{millions(value)}</span>
      <small>wealth at the end</small>
    </div>
  );
}

/** The balance runs from this share with selling better to this share with renting better. */
const BALANCE_RANGE = 0.4;

/** How much better the winner is, as a lean from "about even" towards its side. */
function Balance({ h }: { h: Headline }) {
  const v = h.verdict;
  const sell = v !== undefined && h.strategies[v.winner].kind === "sell_now";
  // Signed (renting better is positive), so a change of winner passes through even.
  const [shown] = useTweened([(sell ? -1 : 1) * (v?.gap_share ?? 0)]);
  if (!v) return null;
  // Without a share (the loser is not positive) the winner is off the scale.
  const lean = ((sell ? -1 : 1) * Math.min(v.gap_share ?? BALANCE_RANGE, BALANCE_RANGE)) / BALANCE_RANGE;
  const x = lean * 50;
  const side = v.tie ? "even" : sell ? "sell" : "rent";
  const label = v.gap_share === undefined ? "" : share(Math.abs(shown));
  return (
    <div className={`balance-scale lean-${side}`}>
      <div className="scale-labels" aria-hidden="true">
        <span>Selling is better</span>
        <span className="scale-mid">about even</span>
        <span>Renting is better</span>
      </div>
      <div className="meter" aria-hidden="true">
        <div className="meter-track" />
        <div className="meter-fill" style={{ left: `${50 + Math.min(x, 0)}%`, width: `${Math.abs(x)}%` }} />
        <div className="meter-centre" />
        {[-30, -20, -10, 10, 20, 30].map((t) => (
          <span key={t} className={t % 20 === 0 ? "meter-tick" : "meter-tick minor"} style={{ left: `${50 + (t / 100 / BALANCE_RANGE) * 50}%` }}>
            {Math.abs(t)}%
          </span>
        ))}
        <div className="meter-knob" style={{ left: `${50 + x}%` }}>
          <b className="num">{label}</b>
          <i />
        </div>
      </div>
    </div>
  );
}

function Verdict({ h }: { h: Headline }) {
  const v = h.verdict;
  const sign = v !== undefined && h.strategies[v.winner].kind === "sell_now" ? -1 : 1;
  // Signed like the balance, shown as sizes.
  const [signedGap, signedShare] = useTweened([sign * (v?.gap ?? 0), sign * (v?.gap_share ?? 0)]);
  const [gap, gapShare] = [Math.abs(signedGap), Math.abs(signedShare)];
  if (!v) {
    return (
      <div className="verdict">
        <h3>Set an investment return to compare renting with selling.</h3>
      </div>
    );
  }
  const renting = `renting for ${years(h.horizon_years)}`;
  const amount = (
    <>
      <span className="verdict-amount">{kr(gap)}</span>
      {v.gap_share !== undefined && <span className="verdict-share"> ({share(gapShare)})</span>}
    </>
  );
  const sell = h.strategies[v.winner].kind === "sell_now";
  const text = v.tie ? (
    <>
      <span className="decision">About even:</span> selling now and {renting} are within {amount}.
    </>
  ) : sell ? (
    <>
      <span className="decision decision-sell">Selling now:</span> {amount} more than {renting}.
    </>
  ) : (
    <>
      <span className="decision decision-rent">Renting for {years(h.horizon_years)}:</span> {amount} more than
      selling now.
    </>
  );
  const t = v.timing;
  const timing =
    t?.kind === "better" ? (
      <p className="verdict-line">
        <strong>Better still:</strong> rent, then sell in {h.strategies[t.index].sale_month} (
        {strategyNote(h.strategies[t.index])}): {kr(t.gain)} more ({share(t.gain_share)}).
      </p>
    ) : t?.kind === "about_same" ? (
      <p className="verdict-line">
        Selling in {h.strategies[t.index].sale_month} ({strategyNote(h.strategies[t.index])}) gives about the same.
      </p>
    ) : null;
  return (
    <div className="verdict">
      <h3 className="num">{text}</h3>
      <BreakEven h={h} />
      {timing}
    </div>
  );
}

/** The return at which selling now and renting tie, worded from the winner's side; part of the verdict. */
function BreakEven({ h }: { h: Headline }) {
  const v = h.verdict;
  if (!v) return null;
  const basis = h.returns_before_tax ? "before tax" : "after tax";
  const sell = h.strategies[v.winner].kind === "sell_now";
  let text;
  if (v.break_even === undefined) {
    text =
      v.break_even_bound === "below"
        ? "Selling outperforms even with a poor investment."
        : "Renting outperforms any realistic investment.";
  } else {
    const r = <strong>{pct(v.break_even)} a year</strong>;
    text = v.tie ? (
      <>The break-even return is {r} ({basis}).</>
    ) : sell ? (
      <>Selling keeps outperforming as long as the investment returns more than {r} ({basis}).</>
    ) : (
      <>Selling outperforms only if the investment returns more than {r} ({basis}).</>
    );
  }
  return <p className="verdict-line">{text}</p>;
}

/**
 * The owner's monthly cash flow, the same in both scenarios: a strip over the
 * period with one segment per phase, split where a debt is paid off.
 */
function CashFlow({ h }: { h: Headline }) {
  if (h.cash.length === 0) return null;
  const months = h.cash.map((p) => monthIndex(p.end) - monthIndex(p.start) + 1);
  const all = months.reduce((a, b) => a + b, 0);
  // Each phase's share of the period, and where each split falls.
  const share = months.map((m) => (100 * m) / all);
  const splitAt = share.map((_, i) => share.slice(0, i + 1).reduce((a, b) => a + b, 0));
  return (
    <section className="cash-flow">
      <h3>Monthly cash flow</h3>
      <p className="muted small">
        What the owner pays in or takes out each month while the property is rented, after tax. It is the same in both
        scenarios: after a sale, the same amounts go into or come out of the investment.
      </p>
      <div className="cash-strip" aria-hidden="true">
        <div className="cash-bar">
          {h.cash.map((p, i) => (
            <div key={p.start} className={p.monthly < 0 ? "cash-seg paid-in" : "cash-seg paid-out"} style={{ width: `${share[i]}%` }}>
              <span>
                {grouped(Math.abs(p.monthly))} kr/mo {p.monthly < 0 ? "paid in" : "paid out"}
              </span>
            </div>
          ))}
        </div>
        <div className="cash-dates">
          <span>{h.start}</span>
          {h.cash.slice(0, -1).map((p, i) => (
            <span
              key={p.end}
              className="cash-split"
              style={{ left: `${splitAt[i]}%` }}
            >
              {p.ends_with === "loan" ? "Loan paid off " : p.ends_with === "fellesgjeld" ? "Fellesgjeld paid off " : ""}
              {p.end}
            </span>
          ))}
          <span>{h.horizon}</span>
        </div>
      </div>
    </section>
  );
}

/** "2037-12" → months since year 0, to measure a phase. */
function monthIndex(month: string): number {
  const [y, m] = month.split("-").map(Number);
  return y * 12 + m - 1;
}

/** The simplifications that apply to this scenario, by topic; collapsed by default. */
function Assumptions({ h }: { h: Headline }) {
  const count = h.assumptions.reduce((n, g) => n + g.items.length, 0);
  return (
    <details className="assumptions">
      <summary>Assumptions ({count})</summary>
      {h.assumptions.map((g) => (
        <section key={g.title} className="assumption-group">
          <h4>{g.title}</h4>
          <ul>
            {g.items.map((a) => (
              <li key={a}>{a}</li>
            ))}
          </ul>
        </section>
      ))}
    </details>
  );
}
