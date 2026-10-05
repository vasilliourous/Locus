# 5. What is not yet true

```
audience:    builder
status:      design-record
authoritative-for: what has not been run, tested, or seen
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **Read this before trusting anything else in this directory.** Every other
> file describes what the code does; this one describes what has **not** been
> run, tested, or seen. It is deliberately specific, because "mostly tested" is
> not a status anyone can act on.
>
> **⚠️ PARTLY SUPERSEDED 2026-10.** Everything below that depends on **device
> binding** (one-code-per-device, the 409, `device.get`/`devices.list`, the
> unique index, the fingerprint-durability fix) describes a mechanism that was
> **removed**, not finished — see
> [`../../../reference/DEVICE-IDENTITY.md`](../../../reference/DEVICE-IDENTITY.md).
> Those items are struck or marked below. What remains genuinely open (the
> **term model, renewal, the migration, the single-use stamp**) is still accurate
> and is restated in [`../../../reference/STILL-OPEN.md`](../../../reference/STILL-OPEN.md)
> §"The term model, renewal and the single-use rule have never met a database".

---

## 5.1 Nothing has been deployed

**No part of this work has run against the live hub.** The hub is production,
there is no sandbox, and deployment is manual (`scp` + `setup.sh`).

Concretely, this means:

| Claim | Status |
|---|---|
| The columns and the collection can be added to the live database | **Untested.** The reconciler is additive-only and has been used before, but not for these fields. |
| The hooks run correctly on the deployed PocketBase | **Untested.** They are syntax-checked and the goja traps are guarded by `check-consistency.sh`, but they have never executed against PB 0.22.21. |
| The migration preserves the paying codes | **Untested, and this is the highest-consequence unknown.** |
| Clients are unaffected | **Reasoned, not observed.** |

## 5.2 The fingerprint fix is not done — MOOT (the mechanism it served is gone)

> **Superseded 2026-10.** This section describes the fingerprint-durability fix
> for **device binding**, and device binding was removed. Nothing depends on the
> fingerprint surviving a reinstall any more: a code is not tied to a device, so
> a reinstalling student is **not** refused and needs **no** operator. The
> consequence stated below ("a reinstalling student is refused with a 409 and
> needs an operator") **is no longer true** — re-activating a redeemed code
> succeeds. Kept as the record of why the fix was deferred, and of what a code
> durability guarantee *would* have required.

**The client's fingerprint can still change across a reinstall**, because it
still falls back to a value persisted only in the client's own config
(`locus/device.rs`).

**Consequence, now that the binding rule is enforced:** a reinstalling student
is refused with a 409 and **needs an operator**. Before this work they would
have silently taken over their own code; now they are correctly refused and
correctly stuck.

The fix (persist the fingerprint somewhere that survives a reinstall) was
**deliberately deferred** rather than shipped, because its core property —
"this survives a reinstall" — cannot be verified without real Windows and macOS
hardware, and shipping an unverified storage-location change could make drift
**worse** in the field.

What is needed to close it:

* A real Windows and a real macOS machine.
* An install → uninstall → reinstall cycle per platform.
* Confirmation that a machine-scoped location is writable **without elevation**
  in each deployment shape.

Until then, the mitigation is the operator escape hatch
([`03-device-binding.md`](03-device-binding.md) §3.7), which depends on
`label` being filled in at sale.

## 5.3 Nothing has been exercised end-to-end on real state

Every claim below is about *runtime behaviour*, and none of it has been
observed:

| # | Not observed | Why it matters |
|---|---|---|
| U1 | A **renewal reaching a running client** and moving the displayed window | The whole propagation path (hook → heartbeat wire → `record_expiry` → UI) is reasoned and unit-tested in pieces, never run as a chain |
| U2 | ~~**The 409 refusal** on a real second activation~~ | **REMOVED 2026-10** — there is no 409; a redeemed code re-activates successfully |
| U3 | ~~`device.get` / `devices.list` running against a populated index~~ | **REMOVED 2026-10** — the collections are kept but read by nothing |
| U4 | ~~The **unique index actually rejecting** a duplicate~~ | **REMOVED 2026-10** — no uniqueness constraint is in play |
| U5 | The **worklist filter** against real rows | The logic was simulated in node against synthetic data; the hook has not run |
| U6 | A **release propagating**, so a client change can reach the field at all | Pre-existing, unchanged, and still the ceiling on how fast any client-side fix can land |

~~U4 is the one to check first after deploying: it is the entire guarantee behind
one-code-per-device, and the schema layer is the backstop that cannot be
forgiven for being wrong.~~ **U4 is moot** — the guarantee it described was
removed. The first thing to check after deploying is now **U1** (a renewal
reaching a running client) and the single-use stamp, both of which are still
unrun — see [`../../../reference/STILL-OPEN.md`](../../../reference/STILL-OPEN.md).

## 5.4 The verification that *was* performed

Stated so the gap above is measurable rather than vague. Full commands in
[`06-verification.md`](06-verification.md).

* The whole Rust and frontend suite passes (`cd client && cargo test && pnpm test`);
  `clippy -D warnings` clean; eslint and `tsc` clean. **Read the counts from the
  runners, not from a sentence** — they are derived and move every commit.
* `check-consistency.sh` green, reconciled against the schema (the guard
  confirms every hook-written field is declared). Run it and read the count:
  `bash server/scripts/check-consistency.sh | grep 'hook-written'`.
* Every hook and script parses (node / `bash -n` / `ast.parse`).
* The console typechecks and **builds**, with all four new actions and both new
  pages present in the emitted bundle.
* The term arithmetic and the renewal arithmetic were **extracted and executed**
  against boundary cases, including that `0`/absent/garbage yields `null` and
  that renewal cannot shorten an expiry.
* The window-filter logic was executed against the boundary and the three
  exclusion cases.
* The 409 classification test was **verified to fail** when its match arm is
  removed.
* The binding lifecycle was simulated across five scenarios, which is how the
  force-delete bug was found.
* The backfill's date parsing was run against the real PocketBase
  space-separator format and four safety cases.

**What that does not establish:** that any of it works when PocketBase is
involved.

## 5.5 Known limitations that are not bugs

These are deliberate, and should not be "fixed" without re-reading the
reasoning:

| Limitation | Why it is intentional |
|---|---|
| **Nothing prompts a renewal** | The hub stores no identity by decision. The worklist is the prompt. This is the largest operational risk in the design — recorded in `../../14-risks.md` §14.6. |
| **A refused student cannot self-serve** | One code per device, enforced. The escape hatch is an audited operator rebind. |
| **Renewal does not clear a suspension** | A payment is not an abuse pardon. |
| **"Active subscribers" counts codes, not people** | No identity. With one code per device it is a close approximation — and the console should not imply precision it lacks. |
| **Free → paid conversion remains unmeasurable** | No cohort, no time series, and the link from a paid code to a free one would be an identity link, which the no-PII decision rules out. |
| **No middleman ledger** | Renewal detail records `price`/`middleman`, so the raw material now exists; nothing aggregates it yet. |

## 5.6 The honest one-line summary

> The model is built, the arithmetic is verified, and the guards are green.
> **None of it has met a database.** The first deployment should be treated as
> the real test, and the migration in particular should be dry-run and read
> before it is applied.
