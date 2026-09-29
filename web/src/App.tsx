import { useEffect, useMemo, useRef, useState } from "react";
import { evaluate, example, parse, toToml } from "./engine";
import type { Headline, Scenario } from "./engine";
import { newInvest, produce, sectionOfError } from "./scenario";
import type { SectionKey } from "./scenario";
import { useDebounced } from "./useDebounced";
import ScenarioForm from "./form/ScenarioForm";
import Results from "./components/Results";
import ErrorBoundary from "./components/ErrorBoundary";

// The scenario is kept in this browser only (localStorage). Nothing is sent anywhere.
const STORAGE_KEY = "rent-or-sell:scenario";
const OLD_TOML_KEY = "rent-or-sell:scenario-toml";
const INPUTS_KEY = "rent-or-sell:inputs";

/** The web page always compares with an investment (its return is edited with the results). */
function withInvestment(s: Scenario): Scenario {
  return s.alternative?.invest
    ? s
    : produce(s, (d) => {
        d.alternative = { ...d.alternative, invest: newInvest() };
      });
}

/** Scenarios saved before vacancy was entered in weeks a year. */
function upgrade(s: Scenario): Scenario {
  const rental = s.rental as Scenario["rental"] & { vacancy_rate?: number };
  if (rental.vacancy_rate !== undefined) {
    rental.vacancy_weeks ??= Math.round(rental.vacancy_rate * 52 * 100) / 100;
    delete rental.vacancy_rate;
  }
  return s;
}

function loadSaved(): Scenario | null {
  try {
    const json = localStorage.getItem(STORAGE_KEY);
    if (json) return upgrade(JSON.parse(json) as Scenario);
    // Step 1 kept the TOML text; carry it over once.
    const toml = localStorage.getItem(OLD_TOML_KEY);
    const parsed = toml ? parse(toml) : null;
    return parsed?.ok ? parsed.value : null;
  } catch {
    return null;
  }
}

function save(s: Scenario) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {
    // Storage unavailable (private window, blocked): editing still works.
  }
}

export default function App() {
  const [scenario, setScenario] = useState<Scenario>(() => withInvestment(loadSaved() ?? example()));
  const [importError, setImportError] = useState<string | null>(null);
  const [inputsOpen, setInputsOpen] = useState(() => {
    try {
      return localStorage.getItem(INPUTS_KEY) !== "closed";
    } catch {
      return true;
    }
  });
  const toggleInputs = () => {
    setInputsOpen(!inputsOpen);
    try {
      localStorage.setItem(INPUTS_KEY, inputsOpen ? "closed" : "open");
    } catch {
      // Not remembered; the toggle still works.
    }
  };
  const fileInput = useRef<HTMLInputElement>(null);
  const debounced = useDebounced(scenario, 150);
  const result = useMemo(() => evaluate(debounced), [debounced]);

  // While the scenario has an error, keep showing the last valid result.
  const [lastGood, setLastGood] = useState<{ h: Headline; s: Scenario } | null>(null);
  useEffect(() => {
    if (result.ok) setLastGood({ h: result.value, s: debounced });
  }, [result, debounced]);
  useEffect(() => save(scenario), [scenario]);

  const errors: Partial<Record<SectionKey, string[]>> = {};
  if (!result.ok) (errors[sectionOfError(result.error)] ??= []).push(result.error);

  const open = async (file: File | undefined) => {
    if (!file) return;
    // The file is parsed into the scenario, so its comments are not kept on Download.
    const parsed = parse(await file.text());
    if (parsed.ok) {
      setScenario(withInvestment(parsed.value));
      setImportError(null);
    } else {
      setImportError(`${file.name}: ${parsed.error}`);
    }
  };

  const download = () => {
    const toml = toToml(scenario);
    if (!toml.ok) return setImportError(toml.error);
    const url = URL.createObjectURL(new Blob([toml.value], { type: "application/toml" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${scenario.name.trim().replace(/[^\w-]+/g, "-") || "scenario"}.toml`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const shown = result.ok ? { h: result.value, s: debounced } : lastGood;

  return (
    <div className="app">
      <header className="topbar">
        <button
          type="button"
          className="pane-toggle"
          aria-expanded={inputsOpen}
          aria-controls="inputs"
          onClick={toggleInputs}
        >
          {inputsOpen ? "Hide scenario" : "Edit scenario"}
        </button>
        <div className="brand-block">
          <div className="brand">Rent or Sell?</div>
          <div className="tagline">Sell a property and invest the proceeds, or rent it.</div>
        </div>
        <div className="topbar-spacer" />
        <div className="topbar-actions">
          <button type="button" onClick={() => fileInput.current?.click()}>
            Open file…
          </button>
          <button type="button" onClick={download}>
            Download
          </button>
          <button type="button" onClick={() => setScenario(withInvestment(example()))}>
            Example
          </button>
          <input
            ref={fileInput}
            type="file"
            accept=".toml,text/plain"
            hidden
            onChange={(e) => {
              void open(e.target.files?.[0]);
              e.target.value = "";
            }}
          />
        </div>
      </header>
      {importError && (
        <div className="import-error" role="alert">
          {importError}
          <button type="button" onClick={() => setImportError(null)} aria-label="Dismiss">
            ×
          </button>
        </div>
      )}
      <div className={inputsOpen ? "panes" : "panes collapsed"}>
        <aside className="inputs" id="inputs" aria-label="Scenario" hidden={!inputsOpen}>
          <ScenarioForm scenario={scenario} onChange={setScenario} errors={errors} />
          <p className="inputs-note">Kept in this browser only. Download saves a file the CLI reads too.</p>
        </aside>
        <main className="results" aria-live="polite">
          {shown ? (
            <ErrorBoundary what="results">
              <Results
                h={shown.h}
                scenario={shown.s}
                stale={!result.ok}
                edit={(f) => setScenario((s) => produce(s, f))}
                investmentError={errors.investment?.[0]}
              />
            </ErrorBoundary>
          ) : (
            <p className="empty">Fix the issues on the left to see results.</p>
          )}
        </main>
      </div>
    </div>
  );
}
