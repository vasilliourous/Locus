# 10. Open questions

> What is still undecided, and what cannot be resolved without real hardware or
> a deployed hub. Kept separate from the design so the design does not pretend
> to be settled where it is not.

---

## 10.1 Decisions the operator must make

### Q1 — Does the free tier get a term?

The free tier ([`../04-tiers.md`](../04-tiers.md) §4.4) is currently planned as a
5 GB/month client-side allowance. The term model introduces a competing way to
bound free access in time.

[`../10-lifecycle.md`](../10-lifecycle.md) §10.1 notes both options:

> *"A free code with a `expires_at` far in the future is a standing free tier; a
> short one is a trial."*

With terms, this becomes explicit: a free code could be given `term_days = 90`
(a term-length trial) or `term_days = 0` (never expires).

**Why it matters:** it changes what free *is*. A never-expiring free code plus a
data cap is a standing product; a 90-day free code is a trial that ends on a
date. The two have different conversion dynamics and different support loads.

**Recommendation:** decide this when the free tier is actually built
([`../18-open-items.md`](../18-open-items.md) P1). The term model supports both,
so it does not block this redesign.

### Q2 — Default term at mint time

What should `codes.generate` default `term_days` to? Options:

* **A term** (~10 weeks / 70 days) — matches `07-billing.md`'s recommendation to
  lead with term passes.
* **A month** (~30 days) — the lower-commitment entry offer.
* **No default** — the operator types it every time.

**Recommendation:** default to a **term**, matching the stated product
strategy, but make it a visible, editable field with the day count shown. Note
the current console already defaults expiry to *one year out*
(`defaultExpiry` in the console's `Codes.vue`: `d.setFullYear(d.getFullYear() + 1)`), which is a
different strategy — that default and this decision should be reconciled
deliberately rather than by accident.

### Q3 — Is a manual renewal routine acceptable?

[`01-identity-without-pii.md`](01-identity-without-pii.md) §1.2 establishes that
**nothing prompts the operator to renew**, and that this is the design's largest
operational risk. Accepted deliberately, but it should be accepted *knowingly*.

The alternative, if it proves unworkable:

* A console **queue** of terms ending within 7 days (already specified,
  [`06-console-and-operator.md`](06-console-and-operator.md) §6.3) — cheap, no
  privacy cost. **Recommend this regardless**, as the minimum mitigation.
* An **email/notification to the operator** — requires an operator contact
  address, which is not student PII, so it is compatible with the no-PII rule.
  Not specified; raise if the queue proves insufficient.

### Q4 — Should a second code stack, or refuse?

**Decided: refuse** ([`03-device-binding.md`](03-device-binding.md) §3.2). The
consequence is restated here because it is a live support burden: a student with
two legitimately-bought codes, or a student who replaces a laptop, needs an
operator. The escape hatch (`codes.rebind`, searchable by name) is therefore not
optional polish.

**Open sub-question:** if a student buys a *second code mid-term*, is the
refusal the right answer, or should the console offer "add this code's term to
the existing subscription"? The operator chose refuse, and this design
implements refuse — but it is worth revisiting after the first real occurrence,
because the honest answer depends on how often it happens.

### Q5 — Committing to a rolling-tag release process for the redesign

Every change here touches the hub, which is deployed by `scp` + `setup.sh`
([`05-migration-and-live-data.md`](05-migration-and-live-data.md) §5.1). The
redesign does not change that. Worth confirming the operator is content with
hub changes being manual, or whether a staging VPS is worth the cost now that
changes are more than one-liners.

## 10.2 What cannot be verified here

Stated because each is easy to assume working and none can be closed in this
environment.

| # | Unverifiable here | What it needs | Risk if assumed working |
|---|---|---|---|
| V1 | **Fingerprint survives a real reinstall** on Windows/macOS | A real machine, a real installer, an uninstall/reinstall cycle | The reported defect persists, and students keep needing unbinds |
| V2 | **A machine-scoped store is writable without elevation** on each platform | Real Windows, macOS, Linux installs | Fingerprint persistence silently falls back to nothing |
| V3 | **`machine_id`-based identity is stable** in the VM/dual-boot setups students actually have | Real hardware | Drift continues on a subset of machines |
| V4 | **A renewal propagates to a running client** | A deployed hub and a running client | Renewal appears to work in tests and not in the field |
| V5 | **The migration preserves the paying codes** | The live hub | **A paying customer loses time — the worst outcome in this design** |
| V6 | **The updater installs anything at all** | A signed release and a running old client | New client behaviours cannot reach the field |

V5 is the one that must be checked by hand and recorded, per
[`05-migration-and-live-data.md`](05-migration-and-live-data.md) §5.8.

## 10.3 Questions the design deliberately does not answer

* **Should there be a per-code price record?** [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md)
  §8.4 recommends recording `price` and `middleman` in the *renewal event
  detail* rather than as columns. If a real ledger is ever wanted, that becomes
  a schema question — deferred until there is a renewal to model.
* **Should suspensions auto-clear on renewal?** **No** — decided
  ([`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4). Recorded here only
  so the reasoning is not rediscovered and reversed.
* **Should a code's term be editable after mint?** Effectively yes, via the
  renewal override and `codes.expire`. A dedicated "change the term" action is
  not proposed; if it is wanted, it needs its own audit semantics.

## 10.4 Facts that would sharpen the design

Not blocking, but each would improve a specific file:

| Fact | Would sharpen |
|---|---|
| How many codes are on the live hub today | [`05-migration-and-live-data.md`](05-migration-and-live-data.md) — whether the backfill is a 5-row or 500-row operation |
| How many are bound | `03-device-binding.md` — the real impact of uniqueness enforcement |
| Whether any paying code is near expiry | The urgency of the term model: if a live code lapses during the migration window, that is a customer lost to a design change |
| How often the operator has had to unbind | `03-device-binding.md` — whether fingerprint drift is common or rare |
| The actual price each code was sold at | [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md) §8.4 |

## 10.5 Related reading

* The design's accepted risks → [`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md) §9.3
* The migration that must not damage live data → [`05-migration-and-live-data.md`](05-migration-and-live-data.md)
* The build sequence these gate → [`11-phased-implementation.md`](11-phased-implementation.md)
