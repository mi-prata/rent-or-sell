# Calculation model

What the engine computes and the rules behind it. Parameter values (rates, thresholds) and their sources live in [`crates/engine/tax-rules/2026.toml`](../crates/engine/tax-rules/2026.toml); the simplifications shown with every result come from `crates/engine/src/assumptions.rs`, in plain words and filtered to the scenario. This file explains how the pieces fit and why.

## Timeline

- The simulation runs monthly from `start` for `horizon_years`; results are valued at the **horizon**, the month just after the last simulated one.
- A sale "at month *s*" happens at the **start** of that month: *s* = 0 is selling now, *s* = horizon is renting throughout. The months before it were rented out.
- Rent and cost growth step once a year, on the anniversary of `start`, using the rate in effect that month. House prices compound monthly.
- Vacancy is expected weeks a year without a tenant (`vacancy_weeks`): weeks ÷ 52 of the rent is lost, spread evenly over every month.
- **Ownership share:** property-level amounts (value, purchase price and costs, improvements, rent, running costs, fixed sale costs) are entered for the whole property and scaled by `ownership_share`. Your loan (and your share of fellesgjeld) is entered as yours and not scaled.

## Renting out: cash flow and tax

- **Loan:** the first payment falls in the `start` month; `loan.end` is the month of the final payment.
  - Annuity payments are recomputed each month over the remaining term, so a rate change re-amortises automatically. Serial loans repay balance ÷ remaining months. Monthly rate = nominal ÷ 12.
  - `interest_only_until`: no principal up to and including that month, then amortised over the remaining term.
- **Rental tax (marginal):** 22% × (rent collected − management fee − deductible costs − interest − loan fees), per calendar year.
  - Deductible: maintenance (restoring the previous standard), municipal fees, property tax, insurance, felleskostnader. Not deductible: improvements (påkostninger), loan and fellesgjeld principal. Other cost lines say `deductible = true/false`.
  - Rent collected up to the tax-free threshold in a year is untaxed, and its costs are then not deducted; **interest is still deducted**.
  - Tax is booked in the year it arises (not as restskatt the year after). A negative tax is a saving, which assumes other income to offset it.
- **Out-of-pocket/mo** is the average monthly pre-tax shortfall; **after-tax/mo** is the year's after-tax cash flow ÷ months.
- **Cash-flow phases** (`Headline.cash`): the after-tax cash flow in stretches of months, shown as "paid in" or "paid out".
  - Each month carries its pre-tax cash flow plus an even share of its year's tax and wealth tax, so the phases add up to renting's cumulative cash (tested).
  - A phase ends at a debt's final payment (loan or fellesgjeld), or before a full year whose cash flow changes direction, unless a payoff between the two years already explains it.
  - Each phase has its first full year's monthly amount (the phase average without one), its total, and a trend: flat when the last full year is within 5% of the first, otherwise rising or falling. A rising payout also carries the rent growth when that is constant; it tracks the rent only approximately (fixed costs like fellesgjeld don't grow).

## Selling

- **Sale costs** = broker rate × price + fixed costs (grown with `costs.growth`).
- **Gain** = (price − sale costs) − (purchase price + purchase costs + improvements). A taxable gain is taxed at 22%; a taxable loss is deductible; a loss on a sale that would have been tax-free is not.
- **Tax-free rule:** owned for more than 12 months *and* lived in it for at least 12 of the 24 months before the sale. The law counts days; the engine counts whole months and resolves every edge conservatively (ownership must exceed 12 whole months; only fully lived months count, from `moved_in + 1` to `moved_out − 1`; only the 23 full months before the sale month). The derived deadline is labelled approximate.
- **Equity after a sale** = price − sale costs − loan − fellesgjeld − gain tax.

## Comparing renting and selling fairly

- **Cash-flow parity:** your pocket sees plain renting's cash flows in every option.
  - After a sale, what renting would have cost each month is paid into the investment, and a surplus is taken out. Once renting pays out (typically after the loan is paid off), selling pays the same amounts out of the investment, so it grows less than it would untouched; that is the income a sale gives up, counted once. Renting's rental tax is mirrored at each year end (in the sale year, only the part not already in the cash before the sale).
  - So every option's **wealth** (its net position; "wealth" in the CLI and on the page) = property equity after a sale (if still held) + investment account after its cashing-out tax + the same cumulative cash.
- **The investment** (`[alternative.invest]`): a return path compounded monthly, and a tax mode:
  - `after_tax` — the return is already net;
  - `yearly` — 22% of each calendar year's return;
  - `deferred` — the gain is taxed on cashing out at 22% × the share-income factor 1.72 = 37.84% (like an ASK). Skjermingsfradrag is ignored, so this slightly overstates the tax.
  - The account is **marginal**: a negative balance means holding less of the investment (borrowing at the same return), and the deferred-tax basis is the net money paid in, not clamped at zero.
- **Break-even returns** are found by bisection over −50% … +100%:
  - per sale option, the constant after-tax return the proceeds need to match renting;
  - per year end, what selling now must earn to match renting until then.

## Strategies at one horizon

The headline comparison (CLI summary, web page) values every option at the **same** horizon, so the options compare directly:

- Every strategy is "sell at month *s*": *s* = 0 is selling now, *s* = horizon is renting throughout, anything between is renting, then selling and investing. `by_sale` holds every month.
- A strategy's value is plain renting's equity at the horizon plus that sale's comparison-row difference. The values therefore match the `--full` comparison table exactly.
- The named strategies are: sell now; sell in the last tax-free month (when the window is open); sell in the best month (when it is strictly between now and the horizon, isn't the tax-free month, and beats both selling now and the tax-free sale by more than the near-tie margin); rent to the horizon.
- **Near-tie margin:** differences under 0.5% of a value count as "about the same". Without it, a one-month blip can win: renting a single month keeps that year's rent under the tax-free rental threshold while the interest is still deducted, worth about 2 000 kr today in one example, which then compounds into a headline "win".
- **Equivalent return:** for each strategy, the constant yearly return, **taxed like the configured investment**, at which selling now ends up worth the same. It is in the same units as the return you enter (gross for `deferred`/`yearly`): selling now's is your own return, renting's is the break-even. For an `after_tax` investment it equals the after-tax break-even.
- `by_return` values the strategies at constant returns from 0 to 12% (0.5 pp steps). Renting is flat; where selling now crosses it is renting's equivalent return.

**Verdict** (`Headline.verdict`, shared by the CLI and the page):
- The winner between **selling now** and **renting for the whole period** (the two ends of the choice), with the gap in kr and as a share of the loser's value. Within the near-tie margin it is "about even".
- The break-even return (renting's equivalent return), worded from the winner's side: the return the investment would need for selling to come out ahead, or how far it can fall before selling stops being ahead. It follows the verdict line.
- Timing: an in-between strategy that beats the winner by more than the margin ("Better still: …"), or else the tax-free sale when it is within the margin ("gives about the same").
- Table rows within the margin of the winner are marked "≈ same".

## Wealth tax (optional `[wealth]`)

- **Marginal:** `W(other + option's holdings) − W(other)`, valued at 31 December and booked that year. `other_net_wealth` is an input, so crossing the threshold is exact. `persons = 2` (spouses assessed together) doubles the thresholds.
- **Valuation:**
  - The property at `assessed_market_value` (Skatteetaten's *beregnet markedsverdi*, whole property, at `start`; defaults to `market_value`), grown with `price_growth` and scaled by `ownership_share`. Secondary home at 100%. The primary-home rates (25% / 70% above the step) are implemented but unreachable, since scenarios require `moved_out ≤ start`.
  - The investment: its equity part (`equity_share`) at the share discount (80%), the rest at 100%. The deferred gain tax is not deductible.
  - Debts are deductible, reduced by the debt-reduction rule: `debt × (discount on shares/funds) / gross assets before discounts`. Only this option's assets and debts are counted, not your other wealth.
- **Parity:** your pocket pays plain renting's wealth tax (part of its after-tax cash flow); in every other option the investment account pays the difference at each year end.

## Borettslag

- `property.kind = "borettslag"` with a `[fellesgjeld]` section: your share of the fellesgjeld as a second loan (annuity/serial, rate path, `interest_only_until`, `at_purchase`).
- **Felleskost invoice:** `costs.felleskostnader` is the whole monthly invoice. The engine splits off the fellesgjeld payment (interest + principal in the `start` month, per whole unit) and treats the rest as operating costs growing with `costs.growth`; the fellesgjeld part follows its own schedule. A note shows the split.
- **Tax:** fellesgjeld interest is deductible like mortgage interest (also under the rent threshold); its principal is not.
- **Sale:** `market_value` is the total price including fellesgjeld; you receive it minus the fellesgjeld the buyer takes over. The broker fee is charged on the total price (**unconfirmed**).
- **Gain:** `(price + fellesgjeld at sale) − (purchase_price + fellesgjeld at purchase) − costs − improvements`, i.e. corrected for fellesgjeld paid down while you owned it (FSFIN § 9-5-1 (2)).
- **Wealth tax:** the taxable value includes the fellesgjeld share, and the fellesgjeld counts as your debt.
- **Rental limits** (warned, not enforced): renting out the whole unit needs board approval and is limited to 3 years, and you must have lived there at least 12 of the 24 months before renting (borettslagsloven §§ 5-5, 5-6).
- A sameie share is covered by `ownership_share`; an eierseksjon is plain selveier.

## Sensitivity

`rent-or-sell sensitivity` re-runs the scenario with one bump at a time: interest rate ±1 pp (loan and fellesgjeld, floored at 0), price growth ±1 pp, rent ±10%, vacancy +1 month a year, running costs +20% (for a borettslag, only the operating part of the invoice). It reports renting's wealth, sell-now vs renting and the break-even return per bump. Deliberately fixed and small; custom what-ifs belong in the web UI.

## Legal sources for rules implemented in code

- Rental income and deductions: [skatteetaten: utleie](https://www.skatteetaten.no/person/skatt/hjelp-til-riktig-skatt/bolig-og-eiendeler/bolig-eiendom-tomt/utleie/)
- Sale of own home: [skatteetaten: salg av bolig](https://www.skatteetaten.no/person/skatt/hjelp-til-riktig-skatt/bolig-og-eiendeler/bolig-eiendom-tomt/salg/)
- Share income factor: [skatteetaten: oppjusteringsfaktor](https://www.skatteetaten.no/en/rates/factor-for-upward-adjustment-of-gainloss-or-dividend-on-shares/)
- Wealth tax rates and thresholds: [Forskuddsutskrivingen 2026](https://www.skatteetaten.no/en/rettskilder/type/uttalelser/uttalelser/forskuddsutskrivingen-2026/); the primary-home step is **14 M** after the revised 2026 budget, although that page still says 10 M ([formuesverdi av bolig](https://www.skatteetaten.no/en/person/taxes/get-the-taxes-right/property-and-belongings/houses-property-and-plots-of-land/tax-value/taxable-value-of-residential-properties/))
- Secondary home and borettslag valuation: [skatteetaten: sekundærbolig](https://www.skatteetaten.no/en/person/taxes/get-the-taxes-right/property-and-belongings/houses-property-and-plots-of-land/tax-value/secondary-dwelling/how-much-tax-will-you-have-to-pay/)
- Debt reduction: [skatteetaten: verdsettingsrabatt](https://www.skatteetaten.no/en/person/taxes/get-the-taxes-right/valuation-discount-in-connection-with-assessment-of-wealth/) (worked example: debt 1.8 M, wealth 6 M of which 1 M equity funds → 60 k reduction; a unit test reproduces it)
- Borettslag gain: [FSFIN § 9-5-1](https://lovdata.no/dokument/SF/forskrift/1999-11-22-1160/%C2%A79-5-1); fellesgjeld as debt: [tredjepartsopplysninger boligselskap](https://www.skatteetaten.no/en/business-and-organisation/reporting-and-industries/third-part-data/eiendom-og-bolig/boligselskap/rettledning/)
- Borettslag renting: [borettslagsloven §§ 5-5, 5-6](https://lovdata.no/lov/2003-06-06-39/§5-6)

**Unconfirmed:** loan fees being deductible (secondary source only), and the broker fee being charged on the borettslag total price.
