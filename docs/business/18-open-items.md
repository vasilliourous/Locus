# 18. Open items

Proposals awaiting a decision, or work already decided but not yet built.
Each names what it is and where it is justified.

---

## 18.1 Decided, not yet built

### P1 — Implement the free tier

The free tier is **decided** ([`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full)) but not
shipped. Concrete work:

- Drop the Eco slot's cap to 1 Mbps and stop selling Eco codes
  ([`04-tiers.md`](04-tiers.md#45-the-deploy-time-choice-renaming-eco--free)).
- Add client-side counting of the 5 GB allowance, **read as a server-provided
  value on heartbeat** so it can change without a release
  ([`15-continuity.md`](15-continuity.md#152-the-free-tiers-continuity-question-new)).
- Add the usage bar, the 80% warning, the persistent throttle banner, and the
  one-tap upgrade CTA ([`04-tiers.md`](04-tiers.md#444-what-the-free-user-sees)).
- Add progressive throttling after the allowance
  ([`04-tiers.md`](04-tiers.md#445-what-throttled-further-means)).

### P2 — Confirm the price ladder

**$0 / $4 / $7** monthly, **$10 / $19** term passes
([`05-pricing.md`](05-pricing.md)). This one number set propagates
everywhere; confirm or adjust, then treat it as fixed.

**Now a real product, not a plan.** The term pass was unsellable until
`9a91da1` — a code could only carry one absolute date set at mint. It is now a
code minted with `term_days ≈ 70`. Confirming the ladder therefore has a
concrete consequence: it becomes the default the console's mint form offers.

### P2.1 — Run the term migration on the live hub

**Decided and built, but not executed.** `server/scripts/backfill-terms.py`
derives a term for existing codes where their own history proves one, **never
touches `expires_at`**, and has a `--dry-run`.

What remains is the operator's part: read the dry-run diff, confirm the live
paying codes are untouched, then apply
([`redesign/05-migration-and-live-data.md`](redesign/05-migration-and-live-data.md)).
Until this runs, codes minted before `9a91da1` have no term — which is safe
(they keep their exact current behaviour) but means no renewal is possible for
them without passing a `term_days` explicitly.

---

## 18.2 Proposed, awaiting a decision

### P3 — Tier bundling (Strike → free code)

The one legacy growth mechanic worth reconsidering
([`11-growth.md`](11-growth.md#114-tier-bundling--reconsider-dont-dismiss)): a Strike buyer gets a free code to give
away, via a `label`ed batch. Decide whether to trial it; cost is free-user
bandwidth ([`06-unit-economics.md`](06-unit-economics.md#63-the-free-tiers-cost)).

### P4 — Server-side quota (only if the soft cap becomes a problem)

Per-user SS2022 credentials plus a usage table would make the quota
tamper-proof ([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)). Real work; do it only if
client-side enforcement proves inadequate.

### P5 — Remote protocol swap

The real continuity project, and the one that closes four of the six risks
([`14-risks.md`](14-risks.md#146-renewal-is-manual--the-risk-the-redesign-introduces)): a second protocol deployed, settable in
`tier_configs`, and a **client update that has actually been installed** —
the last part is unproven (`docs/reference/STILL-OPEN.md`).

### P6 — Instrument free → paid conversion

The metric that validates the entire pricing strategy
([`16-metrics.md`](16-metrics.md#163-the-one-metric-that-now-matters-most)) is currently unmeasurable beyond
hand-counting. Worth a small piece of tooling before scaling free
distribution.

---

## 18.3 Facts still needed from the operator

These do not gate any written decision, but they would sharpen several files:

| Fact | Would sharpen |
|---|---|
| Current paying-user count | [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md) |
| Active middleman count | [`08-distribution.md`](08-distribution.md#84-middleman-economics) |
| Any churn / renewal data | [`05-pricing.md`](05-pricing.md#541-how-this-compares-to-the-old-ladder) |
| Whether Macleans College is still the only market | [`02-market.md`](02-market.md#25-what-the-market-does-not-contain) |
| Real concurrent-user ceiling | [`06-unit-economics.md`](06-unit-economics.md#64-the-estimate-that-must-stay-labelled) |
