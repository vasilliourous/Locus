# 9. Impact on the live plan

> A per-file delta against the twenty files in `docs/business/`.
>
> **STATUS `33b3901`: ALL ROWS BELOW HAVE BEEN APPLIED.** The live plan in
> `docs/business/` has been brought in line with the implementation. This file
> is now the **record of what changed and why**, not a to-do list — it is kept
> so a future agent can see which claims were amended, rather than re-deriving
> the delta by diffing an old commit.

---

## 9.1 How to use this

Each row says what the redesign does to one live file:

| Verdict | Meaning |
|---|---|
| **UNCHANGED** | The redesign does not contradict this file. No edit needed. |
| **AMEND** | The file stays, but specific claims need updating. The change is named. |
| **REWRITE** | The file's premise is overturned; it should be rewritten from the redesign. |
| **NEW FILE** | Content the live plan has no home for. |

## 9.2 The deltas

| File | Verdict | What changes |
|---|---|---|
| `README.md` | **AMEND** | Add a pointer to this directory (as the design record) and to `10-lifecycle.md`'s rewrite. Keep the "code wins" rule; this directory is held to it. |
| `01-overview.md` | **AMEND** | The "three sentences" are unaffected. Add one line: access is sold in **terms**, renewed by the operator. |
| `02-market.md` | **UNCHANGED** | Market analysis does not depend on the entitlement model. |
| `03-product.md` | **AMEND** | §3.3 defines a code as *"a bearer token that is also a physical card... activated once."* **This is the premise the redesign changes.** A code becomes a *voucher*; the entitlement lives in a subscription; a card is spent after first activation. The Luhn/charset details are unchanged. |
| `04-tiers.md` | **UNCHANGED** | Tiers, caps, UoT and the free tier are orthogonal. (Note: §4.4's free-tier work is still NOT BUILT and unaffected.) |
| `05-pricing.md` | **AMEND** | §5.5 already prices term passes ($10/$19) — **the redesign is what makes them sellable.** Add that the ladder's terms are now expressible in code, and note the renewal arithmetic (extend-from-existing). |
| `06-unit-economics.md` | **AMEND (minor)** | Cost per user is unaffected. The *renewal* operation adds operator effort per renewal — worth one line, since §7.1 argues term passes reduce collection frequency. |
| `07-billing.md` | **REWRITE** | **The central commercial fix.** §7.1 recommends leading with the term pass — which the code could not sell. Every claim about expiry-as-an-absolute-date is now wrong. §7.4's "middleman field is the only record of who owes what" breaks once renewal creates repeating revenue — see [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md) §8.4. |
| `08-distribution.md` | **AMEND** | §8.3 "attribution is set at mint time" stays true for the *first* sale. Renewal moves attribution to a per-renewal event. Add the operating routine: **write the student's name into `label`**, because that is the recovery path ([`04-card-and-credential.md`](04-card-and-credential.md) §4.6). |
| `09-console.md` | **AMEND** | §9.1's action table gains `codes.renew` and `codes.rebind`; §9.3's admitted gaps (no ledger) gain a path. §9.2's "attribution and expiry are set at mint time" becomes only half true. |
| `10-lifecycle.md` | **REWRITE** | The lifecycle is now *mint → sell → activate → renew…* with renewal a first-class, repeated operation. §10.1's table is right about the *operations* but missing the important one. §10.2's warning (never bulk-delete; live paid codes) becomes the migration's central constraint — cross-link [`05-migration-and-live-data.md`](05-migration-and-live-data.md). |
| `11-growth.md` | **AMEND (minor)** | §11.2 rejects referral. Unaffected — and note the redesign's `label` field is *not* a referral mechanism, it is an operator note. Worth one clarifying line to prevent conflation. |
| `12-scale-and-ceiling.md` | **UNCHANGED** | Ambition and ceiling do not move. §12.5's "measured conversion data would change this file" stays true and stays unmeasurable ([`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md) §8.3). |
| `13-competition.md` | **UNCHANGED** | The moat argument is unaffected. |
| `14-risks.md` | **AMEND** | Add two risks the redesign *introduces*, not just mitigates: (a) **renewal is manual, so revenue depends on the operator noticing** ([`01-identity-without-pii.md`](01-identity-without-pii.md) §1.2); (b) **paper identity depends on a habit nobody enforces** — a code sold without a `label` cannot be found by name, and the system cannot require it. On fingerprint drift the risk *trades shape*: the piracy hole is closed, but a false positive now strands a student until an operator acts. Recorded as built in [`implementation/03-device-binding.md`](implementation/03-device-binding.md) §3.7. |
| `15-continuity.md` | **AMEND (minor)** | Renewal propagates on heartbeat like any config change, so continuity improves marginally: a renewal no longer needs a client update or a restart. |
| `16-metrics.md` | **AMEND** | §16.2's "Active subscribers" and "Retention" become computable ([`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md)). Free→paid conversion explicitly *stays* unmeasurable — the file should say so rather than imply it is coming. |
| `17-not-built.md` | **AMEND** | The anti-drift file, and the one most likely to go stale the moment the design is written. It must be updated **with** the change, not after: the term model, renewal, uniqueness, `device_bindings` and search moved *out* of it (now built); the **live migration, renewal prompting, the ledger and fingerprint durability** remain in it, with the reason each is still absent. |
| `18-open-items.md` | **AMEND** | P2 (price ladder) gains a dependency: the term model must land for term passes to exist. Add the redesign's open questions as pointers to [`10-open-questions.md`](10-open-questions.md). |
| `19-cross-reference.md` | **AMEND (minor)** | Add "How does renewal work?" and "How is a device bound?" rows pointing into this directory. |

## 9.3 The three most consequential changes

Named separately because they are the ones a reader should check first:

**1. `03-product.md` §3.3 — the code is no longer the credential.**

The single conceptual change everything else follows from. The card becomes a
voucher; the entitlement becomes a subscription keyed by device. If this file is
not amended, every other document inherits a false premise.

**2. `07-billing.md` — the recommended product becomes sellable.**

This is the fix for the inversion described in [`00-README.md`](00-README.md):
the plan recommends term passes, the code cannot express them, and the redesign
closes that gap. Until this lands, the business plan is describing something it
cannot sell.

**3. `10-lifecycle.md` — renewal is a repeating operation.**

Currently the lifecycle is a straight line. With renewal it becomes a cycle,
and that changes what the operator's job *is* — from "mint batches and hand them
out" to "mint batches, hand them out, and then keep track of who is due to pay".

## 9.4 What the redesign does *not* touch

Stated explicitly so the adoption is scoped:

* **Pricing numbers.** $0/$4/$7 and $10/$19 are unchanged by this directory.
* **Tier definitions.** Caps, UoT, BBR, ports — all orthogonal.
* **The free tier's build status.** Still NOT BUILT
  ([`../17-not-built.md`](../17-not-built.md) §17.1); this redesign neither
  implements nor blocks it.
* **The middleman commission rate.** 20–30% is untouched; only its *calculation
  base* becomes more complex ([`06-console-and-operator.md`](06-console-and-operator.md) §6.6).
* **Client UI for expiry display.** Unchanged, deliberately — that is the
  benefit of keeping `expires_at` an instant ([`02-term-and-renewal.md`](02-term-and-renewal.md) §2.2).

## 9.5 Recommended adoption order

If the operator accepts this, apply in this order so that no document is
internally inconsistent at any point:

1. **`17-not-built.md`** — record what is now *planned but unbuilt*. Do this
   first, because it is the anti-drift file and the design is not yet code.
2. **`03-product.md`** — the premise change. Everything else depends on it.
3. **`07-billing.md`** — the commercial fix.
4. **`10-lifecycle.md`** — the operational cycle.
5. **`09-console.md`, `08-distribution.md`** — the surfaces and routines.
6. **`05-pricing.md`, `16-metrics.md`, `14-risks.md`** — the consequences.
7. **Everything else** — the AMEND (minor) and UNCHANGED rows need no action.

Then, and only then, delete this directory's superseded claims — or keep it as
the design record, the way `client/docs/ARCHITECTURE.md` is kept as
*"the design record, not a to-do"*.

## 9.6 Related reading

* The design itself → [`00-README.md`](00-README.md)
* What is still undecided → [`10-open-questions.md`](10-open-questions.md)
* The build sequence → [`11-phased-implementation.md`](11-phased-implementation.md)
