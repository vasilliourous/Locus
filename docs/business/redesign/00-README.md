# Locus — Terms, Customers and Device Binding: Redesign

```
audience:    human-operator
status:      design-record
authoritative-for: the design rationale for terms, renewal and device binding
verified-against: docs/STATE.md
```

> **Largely built** (`9a91da1`; written against client `3.2.5`). The term model,
> renewal, one-code-per-device and search-by-name are implemented and verified
> below the level of a live hub. The fingerprint-durability change is
> **deliberately deferred** until it can be tested on real hardware — see
> [`10-open-questions.md`](10-open-questions.md) §10.2.
>
> **What is built:** `term_days` on codes with the expiry materialised into
> `expires_at`; `codes.renew`, `codes.rebind`, `codes.set-term`; the
> `device_bindings` collection with a unique fingerprint index; the console's
> Renew button and name search; the client's 409 handling; the backfill script.
>
> **What is not:** the live migration (it needs the deployed hub and the
> operator's eyes), a renewal actually reaching a running client, and the
> fingerprint fix.
>
> The live plan in `docs/business/` still needs its per-file updates — see
> [`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md).
>
> **Where this directory and the code disagree, the code wins.** Every checkable
> claim below was read out of the tree. File and line references are given so a
> successor can re-verify rather than trust.

---

## Why this directory exists

Four problems were reported by the operator, from real use:

1. **Codes expire on a fixed calendar date**, not a period measured from
   activation — and there is **no way to renew one** when a middleman reports
   payment.
2. **Nothing stops one device binding itself to many codes.**
3. **A device can silently take over a code** — the operator was forced to
   unbind their own test machine after it stopped being recognised as the
   originally-bound device, with nothing having changed.
4. **Students throw the cards away.** The card *is* the credential, so a lost
   card is an unrecoverable account.

Investigation found these are not four bugs. They are four symptoms of **one
structural absence**, described next. The consequences reach further than the
four reports, which is why this is a redesign and not a patch series.

---

## The root cause: there is no customer

the `collections` list in `server/scripts/seed-pb.py` defines exactly five collections:

```
codes · code_events · tier_configs · activation_attempts · update_config
```

There is **no `customers`, no `subscriptions`, no `devices`, no `payments`, no
`middlemen`.** The entire business is expressed through two objects:

* a **code** — a bearer token that is also a physical card
  ([`../03-product.md`](../03-product.md) §3.3), and
* a **tier** — a row of connection configuration.

Everything the business needs to *know* is either crammed into the single
`codes` row or is not knowable at all. The four reported problems are what that
looks like from the outside:

> **The table below is the DIAGNOSIS — the state of the tree before `9a91da1`.**
> It is kept in the present tense because it is what each report was traced to,
> and because a successor needs to know what the shape of the problem was. What
> has since been built is in "What was built" below; the line and file
> references here describe the pre-change code.

| Reported | Actual mechanism | Why it happens |
|---|---|---|
| Expiry is a fixed date | `expires_at` is an instant set at mint time by `codes.generate` (the `codes.generate` action in `admin_console.pb.js`); `activated_at` is written at bind (the bind path in `activation.pb.js`) but **never read to compute anything** (verified by grep — its only other uses are a console display field and a sort key) | The tree has no vocabulary for "a term". It can only store an instant. |
| No renew button | There is nothing to renew *to*. Renewal means "extend an entitlement", and an entitlement does not exist as a row. | **The business's primary revenue operation is not implementable in the current model.** |
| A device can bind to many codes | `bound_fingerprint` is a *field on the code*. There is no reverse index, and the bind path in `activation.pb.js` never asks whether this fingerprint already holds a binding elsewhere. | Binding is a property of the code, so the code cannot know it is one of several. |
| A code can silently change hands | Binding is **overwrite** semantics — `rec.set("bound_fingerprint", fp)` (`rec.set("bound_fingerprint", fp)` in `activation.pb.js`) with no check that the existing value is empty or equal. | A second device with a different fingerprint simply replaces the first, and nothing records that it happened. |
| Cards get thrown away | The code *is* the credential and there is no identity behind it. | A lost card leaves nothing to recover from. |

### The commercial inversion

This is the sharpest consequence, and it is a business problem rather than a
technical one:

* [`../07-billing.md`](../07-billing.md) §7.1 names the **term pass** as the
  default paid product ("**Recommendation:** lead with the term pass") and
  monthly as the trial-friendly fallback.
* [`../18-open-items.md`](../18-open-items.md) P2 prices term passes at
  **$10 / $19**.
* The implementation stores **one absolute `expires_at` per code**, which can
  express a fixed end date but not "10 weeks".

> **So the business plan recommends a product the system cannot sell, while the
> model it recommends against — monthly with an absolute date — is the only one
> that works.**

The docs are not wrong about the market. They describe an operation the code
cannot perform.

Two further consequences follow, and any redesign must confront them:

* [`../16-metrics.md`](../16-metrics.md) §16.2 defines **"Retention = renewals ÷
  expiries"** and **"Active subscribers"**. Neither is computable: renewal does
  not exist, and "subscriber" is not an entity. That file is measuring something
  the schema does not have.
* [`../10-lifecycle.md`](../10-lifecycle.md) §10.2 warns *"Never bulk-delete
  codes"* because the live hub holds **real paid codes in active use** (count and
  distributor deliberately not recorded — [`../../operate/CLAIMS.md`](../../operate/CLAIMS.md)
  §4). Any schema change must therefore be additive and must never shorten a
  paying student's remaining time ([`05-migration-and-live-data.md`](05-migration-and-live-data.md)).

---

## The decisions already taken

These are settled by the operator and are not open questions here. The **state**
column says whether the decision is now in the code.

| Decision | Value | State | Recorded in |
|---|---|---|---|
| Expiry representation | **Term length, relative to activation** | **BUILT** — `term_days`, materialised into `expires_at` | [`02-term-and-renewal.md`](02-term-and-renewal.md) |
| Renewal arithmetic | **Extend from the existing expiry** (paying early never loses days) | **BUILT** — `codes.renew` | [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4 |
| A second code on a bound device | **Refuse outright**, with the audited unbind path as the escape hatch | **BUILT** — 409 + `device_bindings` unique index | [`03-device-binding.md`](03-device-binding.md) §3.2 |
| Customer identity | **No PII in the hub.** Paper records plus the console's `label`/`notes` fields are the system of record | **BUILT** — and `codes.list` now searches them | [`01-identity-without-pii.md`](01-identity-without-pii.md) |
| Fingerprint stability across reinstall | Persist it somewhere durable; make re-binding explicit | **DEFERRED** — needs real Windows/macOS hardware | [`03-device-binding.md`](03-device-binding.md) §3.5 |
| Card vs credential | Fingerprint-anchored entitlement; operator recovers the rest | **BUILT** (the parts the hub owns) | [`04-card-and-credential.md`](04-card-and-credential.md) |

The identity decision has a consequence worth stating plainly, because it
shapes the whole design: **because the hub holds no identity, every renewal is
an operator action against a code the operator has looked up.** There is no
self-service renewal and no automatic renewal. That is coherent for a cash
business run through middlemen, but it makes **finding the right code fast** the
load-bearing part of the feature — see
[`01-identity-without-pii.md`](01-identity-without-pii.md) §1.3 and the verified
gap it exposes.

---

## The files

This directory is the **design record** — why the model is shaped this way.
For **how it actually works**, see [`implementation/`](implementation/), which
covers the data model, the arithmetic, the binding lifecycle, operator
runbooks, and an explicit list of what has not been verified.

| # | File | What it specifies |
|---|---|---|
| 0 | this file | The brief, the root cause, and the settled decisions |
| 1 | [`01-identity-without-pii.md`](01-identity-without-pii.md) | How the hub identifies a customer with no PII, and why lookup-by-name is load-bearing |
| 2 | [`02-term-and-renewal.md`](02-term-and-renewal.md) | The term model, exact renewal arithmetic, and the renew operation |
| 3 | [`03-device-binding.md`](03-device-binding.md) | Uniqueness, explicit re-binding, and fingerprint stability |
| 4 | [`04-card-and-credential.md`](04-card-and-credential.md) | Separating proof-of-purchase from proof-of-entitlement |
| 5 | [`05-migration-and-live-data.md`](05-migration-and-live-data.md) | The safety file: changing schema around live paying codes |
| 6 | [`06-console-and-operator.md`](06-console-and-operator.md) | The operator surface, including the renew button and search |
| 7 | [`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md) | What must not change on the wire, and why |
| 8 | [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md) | Making the metrics in `16-metrics.md` computable |
| 9 | [`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md) | Per-file delta against the 20 live business files |
| 10 | [`10-open-questions.md`](10-open-questions.md) | What is still undecided, and what needs real hardware |
| 11 | [`11-phased-implementation.md`](11-phased-implementation.md) | Build order and verification gates |

**Where the design and the implementation diverge, the implementation wins.**
Two known divergences, both recorded in
[`implementation/01-data-model.md`](implementation/01-data-model.md) §1.6:

* No separate `subscriptions` entity was built. Once the expiry is materialised
  and the binding is indexed, nothing needed it.
* `codes.set-term` was added — not in the original design, but the migration
  needed an audited way to set a term without moving an expiry.

---

## What was built (commit `9a91da1`)

Recorded here so a successor can tell design from code without diffing.

| Area | What exists now | Where |
|---|---|---|
| Schema | `term_days`, `term_kind` on `codes`; new `device_bindings` with a **unique** `fingerprint` | `server/scripts/seed-pb.py` |
| Term model | Expiry computed from the term at activation and **materialised** into `expires_at`, so every existing reader is unchanged | `server/pb_hooks/activation.pb.js` |
| One code per device | 409 refusal, checked against the binding index | `server/pb_hooks/activation.pb.js` |
| Renewal | `codes.renew` — extends from `max(now, expiry)`, audited with before→after, records `price`/`middleman` in the detail | `server/pb_hooks/admin_console.pb.js` |
| Re-bind | `codes.rebind` — moves a device without touching `expires_at` | same |
| Term admin | `codes.set-term` — sets a term, never moves an expiry | same |
| Search | `codes.list` matches `label`/`notes`/`middleman`, not just the code | same |
| Migration | `backfill-terms.py` with `--dry-run`, never writes `expires_at` | `server/scripts/` |
| Client | 409 → its own `DeviceAlreadyActivated` outcome, pinned by a test | `client/src-tauri/src/locus/activation.rs` |
| Console | Renew (row + detail), Set term, Move to another device, term at mint, search placeholder | `server/console/src/views/Codes.vue` |

**Verification actually performed** (not assumed):

* `check-consistency.sh` green, with **40** hook-written fields reconciled
  against the schema — up from 35, which is the guard confirming the new
  columns are declared.
* 518 Rust tests; `clippy -D warnings` clean; 49 frontend tests; eslint and
  `tsc` clean; the console typechecks and builds, with all four new actions
  present in the emitted bundle.
* The **term arithmetic** and the **renewal arithmetic** were exercised directly
  in node against the extracted helpers — including that a `0`/absent/negative/
  garbage term yields `null` (never a date), that a string `"30"` works, and
  that renewal is provably unable to shorten an existing expiry.
* The 409 test was **verified to fail** when its `match` arm is removed.
* The binding lifecycle was simulated across five scenarios, including that a
  force-deleted bound code frees its device (a bug this work introduced and
  fixed — without it a deletion would brick a device permanently).
* `backfill-terms.py` was run against the real PocketBase space-separator date
  format and four safety cases.

**Not verified, and not claimed:** the live migration against the deployed hub
(which holds real paying codes), a renewal reaching a running client, and the
fingerprint change. See [`10-open-questions.md`](10-open-questions.md) §10.2.

## The one bug this work introduced, and fixed

Worth recording because it is the kind of thing that only shows up in
production: `codes.delete` can force-delete a **bound** code. Under the pre-term
model that was harmless — the code was gone, so the binding was gone with it.
With the binding index, the row survived, pointing at a code that no longer
existed, and the uniqueness check would then refuse **every** future activation
on that device — with no code left on the hub for an operator to unbind. A
deletion would have permanently bricked the machine. `codes.delete` now
releases the binding first.



* **Read 00 → 01 → 02 → 03** for the design itself.
* **Read 05 before touching any code.** It is the file that prevents a live
  student losing time.
* **Read 07 before changing any field name.** The wire names are frozen
  contracts with deployed clients.
* **Read 09 if you are adopting this into `docs/business/`.**

## The rules this directory follows

Inherited from `docs/README.md`, and applied strictly here:

1. **The code wins.** Every claim was checked against the tree; where the
   legacy prose disagrees, the prose is the bug.
2. **Numbers are marked as measured or estimated.** Estimates say so.
3. **A mechanism that does not exist says NOT BUILT**, rather than being
   described as if it worked. This is the rule
   [`../17-not-built.md`](../17-not-built.md) exists to enforce, and this
   directory is the main offender's replacement.
4. **Nothing here changes a live system.** These are specifications; the
   implementation is sequenced in [`11-phased-implementation.md`](11-phased-implementation.md).

---

*Design basis: tree at `a056cac`, client `3.2.5`, 2026-09-29. Supersedes nothing
yet; supersedes `docs/business/` file-by-file as each is accepted.*
