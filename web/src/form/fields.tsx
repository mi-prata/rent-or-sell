// Form building blocks. Inputs are controlled but keep their own text while
// being edited, so "5." or "1 2" don't get reformatted under the cursor.
import { useEffect, useId, useState } from "react";
import type { ReactNode } from "react";
import type { TimePath } from "../engine";
import { constantOf, firstValueOf, pointsOf } from "../scenario";

type Unit = "kr" | "percent" | "plain";

const format = (v: number | undefined, unit: Unit): string => {
  if (v === undefined || Number.isNaN(v)) return "";
  if (unit === "percent") return String(Math.round(v * 100 * 10000) / 10000);
  if (unit === "kr") return Math.round(v).toString().replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return String(v);
};

/** Parses "1 234 567", "5,2" or "5.2"; `null` when not a number. */
const parse = (text: string, unit: Unit): number | null => {
  const t = text.replace(/\s/g, "").replace(",", ".");
  if (t === "" || !/^-?\d*\.?\d*$/.test(t) || t === "-" || t === ".") return null;
  const n = Number(t);
  return unit === "percent" ? n / 100 : n;
};

type NumberFieldProps = {
  label: string;
  value: number | undefined;
  onChange: (v: number | undefined) => void;
  unit?: Unit;
  suffix?: string;
  /** Empty input means "not set" (e.g. an optional amount). */
  optional?: boolean;
  placeholder?: string;
  hint?: string;
  /** Sits inside a sentence: the label is for screen readers only. */
  inline?: boolean;
};

export function NumberField({
  label,
  value,
  onChange,
  unit = "kr",
  suffix,
  optional,
  placeholder,
  hint,
  inline,
}: NumberFieldProps) {
  const id = useId();
  const [text, setText] = useState(format(value, unit));
  const [focused, setFocused] = useState(false);
  // Follow outside changes (import, example) while not typing.
  useEffect(() => {
    if (!focused) setText(format(value, unit));
  }, [value, unit, focused]);
  const invalid = text.trim() !== "" && parse(text, unit) === null;
  const shownSuffix = suffix ?? (unit === "percent" ? "%" : unit === "kr" ? "kr" : undefined);
  const input = (
    <input
      id={id}
      inputMode="decimal"
      value={text}
      placeholder={placeholder}
      aria-invalid={invalid}
      size={inline ? Math.max(text.length, 2) : undefined}
      onFocus={() => setFocused(true)}
      onBlur={() => {
        setFocused(false);
        setText(format(value, unit));
      }}
      onChange={(e) => {
        setText(e.target.value);
        const n = parse(e.target.value, unit);
        if (n !== null) onChange(n);
        else if (e.target.value.trim() === "" && optional) onChange(undefined);
      }}
    />
  );
  if (inline) {
    return (
      <span className={invalid ? "inline-input invalid" : "inline-input"}>
        <label htmlFor={id} className="sr-only">
          {label}
        </label>
        {input}
        {shownSuffix}
      </span>
    );
  }
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <div className={invalid ? "input invalid" : "input"}>
        {input}
        {shownSuffix && <span className="suffix">{shownSuffix}</span>}
      </div>
      {hint && <span className="hint">{hint}</span>}
    </div>
  );
}

const MONTH = /^\d{4}-(0[1-9]|1[0-2])$/;

type MonthFieldProps = {
  label: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  optional?: boolean;
  placeholder?: string;
  hint?: string;
};

/** A "YYYY-MM" month. Plain text so it behaves the same in every browser. */
export function MonthField({ label, value, onChange, optional, placeholder, hint }: MonthFieldProps) {
  const id = useId();
  const [text, setText] = useState(value ?? "");
  const [focused, setFocused] = useState(false);
  useEffect(() => {
    if (!focused) setText(value ?? "");
  }, [value, focused]);
  const invalid = text !== "" && !MONTH.test(text);
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <div className={invalid ? "input invalid" : "input"}>
        <input
          id={id}
          value={text}
          placeholder={placeholder ?? "YYYY-MM"}
          aria-invalid={invalid}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onChange={(e) => {
            setText(e.target.value);
            if (MONTH.test(e.target.value)) onChange(e.target.value);
            else if (e.target.value === "" && optional) onChange(undefined);
          }}
        />
      </div>
      {hint && <span className="hint">{hint}</span>}
    </div>
  );
}

export function TextField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const id = useId();
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <div className="input">
        <input id={id} value={value} onChange={(e) => onChange(e.target.value)} />
      </div>
    </div>
  );
}

/** A small set of options as a segmented control. */
export function Choice<T extends string | number>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
}) {
  return (
    <div className="field">
      <span className="field-label">{label}</span>
      <div className="segmented" role="radiogroup" aria-label={label}>
        {options.map(([v, text]) => (
          <button
            key={String(v)}
            type="button"
            role="radio"
            aria-checked={v === value}
            className={v === value ? "on" : ""}
            onClick={() => onChange(v)}
          >
            {text}
          </button>
        ))}
      </div>
    </div>
  );
}

/**
 * A rate or amount that may vary over time; the form edits a single value.
 * A time-varying value (from a file) is shown, not edited: "Use one value"
 * replaces it with its first value.
 */
export function TimePathField({
  label,
  value,
  onChange,
  unit = "percent",
  inline,
}: {
  label: string;
  value: TimePath | undefined;
  onChange: (v: TimePath) => void;
  unit?: Unit;
  inline?: boolean;
}) {
  const single = constantOf(value);
  if (single === undefined && inline) {
    return (
      <span className="varies-inline">
        a return that varies over time ({pointsOf(value)} points){" "}
        <button type="button" onClick={() => onChange({ constant: firstValueOf(value) })}>
          Use one value
        </button>
      </span>
    );
  }
  if (single === undefined) {
    return (
      <div className="field">
        <span className="field-label">{label}</span>
        <div className="varies">
          <span>Varies over time ({pointsOf(value)} points)</span>
          <button type="button" onClick={() => onChange({ constant: firstValueOf(value) })}>
            Use one value
          </button>
        </div>
      </div>
    );
  }
  return (
    <NumberField
      label={label}
      unit={unit}
      value={single}
      onChange={(v) => onChange({ constant: v ?? 0 })}
      inline={inline}
    />
  );
}

export function Checkbox({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="checkbox">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

type SectionProps = {
  title: string;
  /** One-line summary shown when collapsed. */
  summary?: string;
  /** For optional sections: whether it's switched on. */
  enabled?: boolean;
  onToggle?: (on: boolean) => void;
  /** Why the switch can't be used, if it can't. */
  toggleDisabled?: string;
  errors?: string[];
  defaultOpen?: boolean;
  children: ReactNode;
  /** Rarely used fields, behind "More". */
  more?: ReactNode;
};

export function Section({
  title,
  summary,
  enabled,
  onToggle,
  toggleDisabled,
  errors = [],
  defaultOpen = false,
  children,
  more,
}: SectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  const [showMore, setShowMore] = useState(false);
  const optional = enabled !== undefined;
  const off = optional && !enabled;
  const expanded = (open || errors.length > 0) && !off;
  const bodyId = useId();
  return (
    <section className={off ? "section off" : "section"}>
      <div className="section-head">
        <button
          type="button"
          className="section-title"
          aria-expanded={expanded}
          aria-controls={bodyId}
          disabled={off}
          onClick={() => setOpen(!open)}
        >
          <span className="chevron" aria-hidden="true">
            {expanded ? "▾" : "▸"}
          </span>
          <h2>{title}</h2>
          {!expanded && (
            <span className="section-summary">{off ? "Off" : summary}</span>
          )}
        </button>
        {errors.length > 0 && <span className="badge-error">{errors.length === 1 ? "1 issue" : `${errors.length} issues`}</span>}
        {optional && (
          <label className="switch" title={toggleDisabled}>
            <span className="sr-only">{title}</span>
            <input
              type="checkbox"
              role="switch"
              checked={enabled}
              disabled={toggleDisabled !== undefined && !enabled}
              onChange={(e) => {
                onToggle?.(e.target.checked);
                if (e.target.checked) setOpen(true);
              }}
            />
            <span className="track" aria-hidden="true" />
          </label>
        )}
      </div>
      {optional && !enabled && toggleDisabled && <p className="hint section-hint">{toggleDisabled}</p>}
      {expanded && (
        <div className="section-body" id={bodyId}>
          {errors.map((e) => (
            <p key={e} className="field-error" role="alert">
              {e}
            </p>
          ))}
          {children}
          {more && (
            <>
              <button type="button" className="more" onClick={() => setShowMore(!showMore)} aria-expanded={showMore}>
                {showMore ? "Less" : "More"}
              </button>
              {showMore && <div className="more-body">{more}</div>}
            </>
          )}
        </div>
      )}
    </section>
  );
}
