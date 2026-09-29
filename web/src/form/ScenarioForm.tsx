import type { Cost, LoanKind, Scenario, TimePath } from "../engine";
import { grouped, pct } from "../format";
import {
  constantOf,
  newFellesgjeld,
  newLoan,
  newSavings,
  newWealth,
  produce,
} from "../scenario";
import type { SectionKey } from "../scenario";
import { Checkbox, Choice, MonthField, NumberField, Section, TextField, TimePathField } from "./fields";

type Props = {
  scenario: Scenario;
  onChange: (s: Scenario) => void;
  errors: Partial<Record<SectionKey, string[]>>;
};

const rateText = (tp: TimePath | undefined) => {
  const c = constantOf(tp);
  return c === undefined ? "varies" : pct(c);
};

const monthlyOf = (c: Cost | undefined) => (c ? (c.monthly ?? (c.annual ?? 0) / 12) : 0);

const LOAN_KINDS: [LoanKind, string][] = [
  ["annuity", "Annuity"],
  ["serial", "Serial"],
];

/** The whole scenario as a form, one section per part of the scenario file. */
export default function ScenarioForm({ scenario: s, onChange, errors }: Props) {
  const edit = (f: (d: Scenario) => void) => onChange(produce(s, f));
  const borettslag = s.property.kind === "borettslag";

  return (
    <div className="form">
      <Section
        title="Scenario"
        summary={`${s.horizon_years} years from ${s.start}`}
        errors={errors.general}
      >
        <TextField label="Name" value={s.name} onChange={(v) => edit((d) => (d.name = v))} />
        <div className="grid-2">
          <MonthField label="Start" value={s.start} onChange={(v) => v && edit((d) => (d.start = v))} />
          <NumberField
            label="Horizon"
            unit="plain"
            suffix="years"
            value={s.horizon_years}
            onChange={(v) => v !== undefined && edit((d) => (d.horizon_years = Math.round(v)))}
          />
        </div>
      </Section>

      <Section
        title="Property"
        defaultOpen
        summary={`${grouped(s.property.market_value)} kr`}
        errors={errors.property}
        more={
          <>
            <div className="grid-2">
              <NumberField
                label="Ownership share"
                unit="percent"
                value={s.property.ownership_share ?? 1}
                onChange={(v) => edit((d) => (d.property.ownership_share = v ?? 1))}
              />
              <NumberField
                label="Purchase costs"
                value={s.property.purchase_costs ?? 0}
                onChange={(v) => edit((d) => (d.property.purchase_costs = v ?? 0))}
              />
            </div>
            <ListHead
              title="Improvements (påkostninger)"
              onAdd={() =>
                edit((d) => {
                  d.property.improvements = [
                    ...(d.property.improvements ?? []),
                    { date: d.property.purchase_date, amount: 0, description: undefined },
                  ];
                })
              }
            />
            {(s.property.improvements ?? []).map((imp, i) => (
              <div className="list-row" key={i}>
                <MonthField
                  label="Date"
                  value={imp.date}
                  onChange={(v) => v && edit((d) => (d.property.improvements![i].date = v))}
                />
                <NumberField
                  label="Amount"
                  value={imp.amount}
                  onChange={(v) => edit((d) => (d.property.improvements![i].amount = v ?? 0))}
                />
                <RemoveButton onClick={() => edit((d) => d.property.improvements!.splice(i, 1))} />
              </div>
            ))}
          </>
        }
      >
        <Choice
          label="Type"
          value={s.property.kind ?? "selveier"}
          options={[
            ["selveier", "Selveier"],
            ["borettslag", "Borettslag"],
          ]}
          onChange={(kind) =>
            edit((d) => {
              d.property.kind = kind;
              d.fellesgjeld = kind === "borettslag" ? (d.fellesgjeld ?? newFellesgjeld(d)) : undefined;
            })
          }
        />
        <div className="grid-2">
          <NumberField
            label={borettslag ? "Market value (incl. fellesgjeld)" : "Market value"}
            value={s.property.market_value}
            onChange={(v) => edit((d) => (d.property.market_value = v ?? 0))}
          />
          <TimePathField
            label="Price growth / yr"
            value={s.property.price_growth}
            onChange={(v) => edit((d) => (d.property.price_growth = v))}
          />
          <MonthField
            label="Purchased"
            value={s.property.purchase_date}
            onChange={(v) => v && edit((d) => (d.property.purchase_date = v))}
          />
          <NumberField
            label={borettslag ? "Purchase price (excl. fellesgjeld)" : "Purchase price"}
            value={s.property.purchase_price}
            onChange={(v) => edit((d) => (d.property.purchase_price = v ?? 0))}
          />
        </div>
      </Section>

      {borettslag && s.fellesgjeld && (
        <Section
          title="Fellesgjeld"
          defaultOpen
          summary={`${grouped(s.fellesgjeld.balance)} kr · ${rateText(s.fellesgjeld.nominal_rate)}`}
          errors={errors.fellesgjeld}
          more={
            <div className="grid-2">
              <Choice
                label="Type"
                value={s.fellesgjeld.kind}
                options={LOAN_KINDS}
                onChange={(v) => edit((d) => (d.fellesgjeld!.kind = v))}
              />
              <MonthField
                label="Interest-only until"
                optional
                placeholder="none"
                value={s.fellesgjeld.interest_only_until}
                onChange={(v) => edit((d) => (d.fellesgjeld!.interest_only_until = v))}
              />
            </div>
          }
        >
          <div className="grid-2">
            <NumberField
              label="Your share now"
              value={s.fellesgjeld.balance}
              onChange={(v) => edit((d) => (d.fellesgjeld!.balance = v ?? 0))}
            />
            <NumberField
              label="When you bought"
              optional
              placeholder="same as now"
              value={s.fellesgjeld.at_purchase}
              onChange={(v) => edit((d) => (d.fellesgjeld!.at_purchase = v))}
            />
            <TimePathField
              label="Interest rate"
              value={s.fellesgjeld.nominal_rate}
              onChange={(v) => edit((d) => (d.fellesgjeld!.nominal_rate = v))}
            />
            <MonthField
              label="Final payment"
              value={s.fellesgjeld.end}
              onChange={(v) => v && edit((d) => (d.fellesgjeld!.end = v))}
            />
          </div>
        </Section>
      )}

      <Section
        title="Loan"
        defaultOpen
        enabled={s.loan !== undefined}
        onToggle={(on) => edit((d) => (d.loan = on ? newLoan(d) : undefined))}
        summary={s.loan && `${grouped(s.loan.balance)} kr · ${rateText(s.loan.nominal_rate)} · to ${s.loan.end}`}
        errors={errors.loan}
        more={
          s.loan && (
            <div className="grid-2">
              <Choice
                label="Type"
                value={s.loan.kind}
                options={LOAN_KINDS}
                onChange={(v) => edit((d) => (d.loan!.kind = v))}
              />
              <NumberField
                label="Fee per payment"
                value={s.loan.monthly_fee ?? 0}
                onChange={(v) => edit((d) => (d.loan!.monthly_fee = v ?? 0))}
              />
              <MonthField
                label="Interest-only until"
                optional
                placeholder="none"
                value={s.loan.interest_only_until}
                onChange={(v) => edit((d) => (d.loan!.interest_only_until = v))}
              />
            </div>
          )
        }
      >
        {s.loan && (
          <div className="grid-2">
            <NumberField
              label="Balance"
              value={s.loan.balance}
              onChange={(v) => edit((d) => (d.loan!.balance = v ?? 0))}
            />
            <MonthField
              label="Final payment"
              value={s.loan.end}
              onChange={(v) => v && edit((d) => (d.loan!.end = v))}
            />
            <TimePathField
              label="Interest rate"
              value={s.loan.nominal_rate}
              onChange={(v) => edit((d) => (d.loan!.nominal_rate = v))}
            />
          </div>
        )}
      </Section>

      <Section
        title="Rental"
        defaultOpen
        summary={`${grouped(s.rental.monthly_rent)} kr/mo`}
        errors={errors.rental}
        more={
          <div className="grid-2">
            <NumberField
              label="Management fee"
              unit="percent"
              value={s.rental.management_fee_rate ?? 0}
              onChange={(v) => edit((d) => (d.rental.management_fee_rate = v ?? 0))}
            />
          </div>
        }
      >
        <div className="grid-3">
          <NumberField
            label="Rent / month"
            value={s.rental.monthly_rent}
            onChange={(v) => edit((d) => (d.rental.monthly_rent = v ?? 0))}
          />
          <NumberField
            label="Vacancy"
            unit="plain"
            suffix="weeks/yr"
            value={s.rental.vacancy_weeks ?? 0}
            onChange={(v) => edit((d) => (d.rental.vacancy_weeks = v ?? 0))}
          />
          <TimePathField
            label="Rent growth / yr"
            value={s.rental.rent_growth}
            onChange={(v) => edit((d) => (d.rental.rent_growth = v))}
          />
        </div>
      </Section>

      <CostsSection s={s} edit={edit} errors={errors.costs} borettslag={borettslag} />

      <Section
        title="Sale"
        summary={`${pct(s.sale?.broker_rate ?? 0)} broker · ${grouped(s.sale?.fixed_costs ?? 0)} kr`}
        errors={errors.sale}
      >
        <div className="grid-2">
          <NumberField
            label="Broker fee"
            unit="percent"
            value={s.sale?.broker_rate ?? 0}
            onChange={(v) => edit((d) => (d.sale = { ...d.sale, broker_rate: v ?? 0 }))}
          />
          <NumberField
            label="Other sale costs"
            value={s.sale?.fixed_costs ?? 0}
            onChange={(v) => edit((d) => (d.sale = { ...d.sale, fixed_costs: v ?? 0 }))}
          />
        </div>
      </Section>

      <Section
        title="Owner"
        summary={`moved out ${s.owner?.moved_out ?? s.start}`}
        errors={errors.owner}
      >
        <div className="grid-2">
          <MonthField
            label="Moved in"
            optional
            placeholder={s.property.purchase_date}
            value={s.owner?.moved_in}
            onChange={(v) => edit((d) => (d.owner = { ...d.owner, moved_in: v }))}
            hint="Empty: the purchase month"
          />
          <MonthField
            label="Moved out"
            optional
            placeholder={s.start}
            value={s.owner?.moved_out}
            onChange={(v) => edit((d) => (d.owner = { ...d.owner, moved_out: v }))}
            hint="Empty: the start month"
          />
        </div>
      </Section>

      <Section
        title="Savings"
        enabled={s.savings !== undefined}
        onToggle={(on) => edit((d) => (d.savings = on ? newSavings() : undefined))}
        summary={s.savings && `${grouped(s.savings.amount)} kr`}
        errors={errors.savings}
      >
        {s.savings && (
          <div className="grid-2">
            <NumberField
              label="Amount"
              value={s.savings.amount}
              onChange={(v) => edit((d) => (d.savings!.amount = v ?? 0))}
            />
            <MonthField
              label="Available from"
              optional
              placeholder={s.start}
              value={s.savings.at}
              onChange={(v) => edit((d) => (d.savings!.at = v))}
            />
          </div>
        )}
      </Section>

      <Section
        title="Wealth tax"
        enabled={s.wealth !== undefined}
        onToggle={(on) => edit((d) => (d.wealth = on ? newWealth() : undefined))}
        summary={s.wealth && `${s.wealth.persons === 2 ? "couple" : "single"}`}
        errors={errors.wealth}
      >
        {s.wealth && (
          <>
            <Choice
              label="Assessed as"
              value={s.wealth.persons ?? 1}
              options={[
                [1, "Single"],
                [2, "Couple"],
              ]}
              onChange={(v) => edit((d) => (d.wealth!.persons = v))}
            />
            <div className="grid-2">
              <TimePathField
                label="Other net wealth"
                unit="kr"
                value={s.wealth.other_net_wealth}
                onChange={(v) => edit((d) => (d.wealth!.other_net_wealth = v))}
              />
              <NumberField
                label="Assessed value"
                optional
                placeholder="market value"
                value={s.wealth.assessed_market_value}
                onChange={(v) => edit((d) => (d.wealth!.assessed_market_value = v))}
                hint="Beregnet markedsverdi in your tax return (whole property)"
              />
            </div>
          </>
        )}
      </Section>
    </div>
  );
}

const STANDARD_COSTS: ["felleskostnader" | "municipal_fees" | "property_tax" | "insurance" | "maintenance", string][] = [
  ["felleskostnader", "Felleskostnader"],
  ["municipal_fees", "Municipal fees"],
  ["property_tax", "Property tax"],
  ["insurance", "Insurance"],
  ["maintenance", "Maintenance"],
];

function CostsSection({
  s,
  edit,
  errors,
  borettslag,
}: {
  s: Scenario;
  edit: (f: (d: Scenario) => void) => void;
  errors?: string[];
  borettslag: boolean;
}) {
  const costs = s.costs ?? {};
  const total =
    STANDARD_COSTS.reduce((sum, [k]) => sum + monthlyOf(costs[k]), 0) +
    (costs.other ?? []).reduce((sum, c) => sum + monthlyOf(c), 0);
  const count = STANDARD_COSTS.filter(([k]) => costs[k]).length + (costs.other?.length ?? 0);
  return (
    <Section
      title="Running costs"
      summary={`${count} items · ${grouped(total)} kr/mo`}
      errors={errors}
      more={
        <>
          <ListHead
            title="Other costs"
            onAdd={() =>
              edit((d) => {
                d.costs = {
                  ...d.costs,
                  other: [...(d.costs?.other ?? []), { name: "Other", monthly: 0, annual: undefined, deductible: true }],
                };
              })
            }
          />
          {(costs.other ?? []).map((c, i) => (
            <div className="list-row" key={i}>
              <TextField label="Name" value={c.name} onChange={(v) => edit((d) => (d.costs!.other![i].name = v))} />
              <CostInput
                label="Amount"
                cost={c}
                onChange={(next) =>
                  edit((d) => {
                    if (next) d.costs!.other![i] = { ...d.costs!.other![i], ...next };
                  })
                }
              />
              <RemoveButton onClick={() => edit((d) => d.costs!.other!.splice(i, 1))} />
            </div>
          ))}
        </>
      }
    >
      <TimePathField
        label="Cost growth / yr"
        value={costs.growth}
        onChange={(v) => edit((d) => (d.costs = { ...d.costs, growth: v }))}
      />
      {STANDARD_COSTS.map(([key, label]) => (
        <CostInput
          key={key}
          label={key === "felleskostnader" && borettslag ? `${label} (whole invoice)` : label}
          cost={costs[key]}
          onChange={(next) => edit((d) => (d.costs = { ...d.costs, [key]: next }))}
        />
      ))}
    </Section>
  );
}

/** An amount per month or per year, and whether it's tax-deductible. Empty = none. */
function CostInput({
  label,
  cost,
  onChange,
}: {
  label: string;
  cost: Cost | undefined;
  onChange: (c: Cost | undefined) => void;
}) {
  const perYear = cost?.annual !== undefined && cost.monthly === undefined;
  const amount = perYear ? cost?.annual : cost?.monthly;
  const deductible = cost?.deductible ?? true;
  const set = (value: number | undefined, yearly: boolean, ded: boolean) =>
    onChange(
      value === undefined
        ? undefined
        : { monthly: yearly ? undefined : value, annual: yearly ? value : undefined, deductible: ded },
    );
  return (
    <div className="cost-row">
      <NumberField label={label} optional placeholder="none" value={amount} onChange={(v) => set(v, perYear, deductible)} />
      <div className="segmented small" role="radiogroup" aria-label={`${label}: period`}>
        {[
          [false, "/ mo"],
          [true, "/ yr"],
        ].map(([yearly, text]) => (
          <button
            key={String(yearly)}
            type="button"
            role="radio"
            aria-checked={perYear === yearly}
            className={perYear === yearly ? "on" : ""}
            onClick={() => set(amount ?? 0, yearly as boolean, deductible)}
          >
            {text as string}
          </button>
        ))}
      </div>
      <Checkbox label="Deductible" checked={deductible} onChange={(v) => amount !== undefined && set(amount, perYear, v)} />
    </div>
  );
}

function ListHead({ title, onAdd }: { title: string; onAdd: () => void }) {
  return (
    <div className="list-head">
      <span>{title}</span>
      <button type="button" onClick={onAdd}>
        + Add
      </button>
    </div>
  );
}

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="remove" aria-label="Remove" onClick={onClick}>
      ×
    </button>
  );
}
