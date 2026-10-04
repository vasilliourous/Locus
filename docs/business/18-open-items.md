# 18. Open items

```
audience:    human-operator
status:      live
authoritative-for: what is decided but unbuilt, what is proposed, and what the operator must supply
verified-against: docs/STATE.md
```

Proposals awaiting a decision, or work already decided but not yet built.
Each names what it is and where it is justified.

---

## 18.1 Decided, not yet built

### P1 — Implement the free tier

**Largely built.** The enforcement works end to end; the purchase does not. What
is done, and what is left:

**Built:**
- The Eco/8443 slot is capped at 1 Mbps in `04-tc.sh` — both the applied class
  and the reboot unit, which is the pair that silently disagree if you edit one
  ([`04-tiers.md`](04-tiers.md#45-the-deploy-time-choice-renaming-eco--free)).
- A `free` `tier_configs` row is seeded beside the legacy `eco` one, so codes
  minted now and codes already in the field both resolve.
- The allowance is sent on every heartbeat (`free_allowance_mb`,
  `free_throttle_mbps`), so it can change without a client release
  ([`15-continuity.md`](15-continuity.md#152-the-free-tiers-continuity-question-new)).
- Client-side counting of the 5 GB window, the 80% line and the classification —
  one pure module, `client/src-tauri/src/locus/usage.rs`, fake-clock tested.
- The usage bar, the warning and the throttle state on the connection screen.
- `check-consistency.sh` §23 holds all of it in agreement, and was observed
  failing against each half-deployed state.

- **The upgrade route** (shipped 3.2.26). The throttle state is followed by a
  sentence naming the action — contact the middleman about Stealth. There is no
  self-serve checkout to link to; a code is a physical card
  ([`08-distribution.md`](08-distribution.md)).
- **Safety guards** (shipped 3.2.26): an absurd allowance from the hub is clamped
  rather than saturating into a permanent throttle; a future-dated window is
  repaired rather than underflowing the elapsed-time maths; a malformed advisory
  field no longer fails the whole heartbeat; a corrupt stored counter resets and
  **logs** rather than resetting silently.

**Still to do:**
- **Applying the throttle to a running Core.** `free_throttle_mbps` is parsed and
  stored, and **nothing yet lowers a live Core's speed from it**. The decision to
  throttle works; the act does not exist. This is the largest remaining gap and
  the one whose absence a student would notice — see
  [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md).
- **Operator control from the console.** The allowance is a hook value today;
  making it console-editable is what lets the operator change it without a
  deploy.
- **Verification.** That the client actually applies the slower cap to a live Core
  is untested — the classification is tested, the effect on a running tunnel is
  not.
- **Deployment.** All of the hub half is inert until `setup.sh` re-runs.

**Before a middleman sells a free card**, the throttle application and a hub
deploy are the two items that matter.

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
