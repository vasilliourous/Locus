# 11. Phased implementation

```
audience:    builder
status:      design-record
authoritative-for: the phased plan (phases 0–3,5 built; 4 deferred)
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **STATUS `983a254`: phases 0–3 and 5 are BUILT. Phase 4 (the client
> fingerprint fix) is NOT, and was deferred deliberately — see below.**
>
> Built in `9a91da1` (schema, term model, renewal, rebind, uniqueness, search,
> backfill, client 409) and `6a422bc` (the renewal worklist, `device.get`,
> `devices.list`, the Devices page). The design specified more than the code
> needed in two places; where they differ, the implementation is recorded in
> [`implementation/`](implementation/).
>
> **Phase 4 was deferred on purpose.** Its core property — "the fingerprint
> survives a reinstall" — cannot be verified without real Windows and macOS
> hardware, and shipping an unverified storage-location change risks making
> fingerprint drift *worse* in the field. Until it lands, a reinstalling
> student is refused and needs an operator; that trade is recorded in
> [`implementation/05-not-yet-true.md`](implementation/05-not-yet-true.md) §5.2.

> The build order, with the verification gate for each phase. Ordered so that
> **every phase is independently safe** and nothing depends on a phase after it.

---

## 11.1 The ordering principle

Three constraints determine the sequence:

1. **The server model must land first.** The console button and the client
   tests both depend on the term arithmetic existing. Building the button first
   would mean inventing a second implementation of the arithmetic.
2. **Nothing may shorten a live student's term at any point.**
   ([`05-migration-and-live-data.md`](05-migration-and-live-data.md) §5.2.)
3. **A deployed client cannot be updated**
   ([`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md) §7.1),
   so the wire shape must not move.

| Phase | What | Depends on | Risk if skipped |
|---|---|---|---|
| 0 | Safety net | — | No rollback |
| 1 | Server: schema + term model | 0 | — |
| 2 | Server: renew, rebind, uniqueness | 1 | The revenue operation missing |
| 3 | Console: the operator surface | 2 | Renewal is unusable |
| 4 | Client: fingerprint durability + tests | — (parallel) | The reported defect persists |
| 5 | Docs adoption | 1–4 | The plan keeps describing something unsellable |

Phases 1–3 are server-only and can be deployed to the hub. Phase 4 is a client
release. They are independent beyond the model in phase 1.

---

## Phase 0 — Safety net

**Goal:** be able to undo everything.

- [ ] Back up `pb_data` on the live hub. The machinery exists
      (`server/modules/07-backups.sh`, `server/restore.sh`); confirm a restore
      actually works rather than assuming it.
- [ ] Confirm the current hooks are in sync with the repo copy, using the
      `md5sum` loop in `docs/reference/STILL-OPEN.md`
      (`activation admin_console admin_unbind code_lookup heartbeat release update`).
      If they are already drifting, fix that **before** layering changes on top.

**Gate:** a backup exists and a restore has been demonstrated.

---

## Phase 1 — Server: schema and the term model

**Goal:** the term exists in the database, and nothing behaves differently yet.

- [ ] Add `term_days` (number) and `term_kind` (text) to `codes` in
      `seed-pb.py`'s `collections` list. **Additive only** — the reconciler at
      the schema reconciler in `seed-pb.py` adds missing columns and never retypes or removes.
- [ ] Add the `device_bindings` collection (`seed-pb.py`) with a **unique**
      fingerprint index ([`03-device-binding.md`](03-device-binding.md) §3.1).
- [ ] Write the **backfill script** as a separate file, with a `--dry-run` that
      prints `old_expires_at → new_expires_at` per code
      ([`05-migration-and-live-data.md`](05-migration-and-live-data.md) §5.4).
      It must write `term_days` only where derivable, never touch `expires_at`.
- [ ] Teach `activation.pb.js` the bind path to compute
      `expires_at = now + term_days` **only when `term_days > 0`**, and to leave
      the existing value untouched otherwise.

**Gate:**
- `check-consistency.sh` green — including group 3, which fails if a hook writes
  a field the schema lacks.
- The dry-run diff reviewed by the operator.
- **Every bound code's `expires_at` and `bound_fingerprint` are byte-identical
  after the migration.** Recorded, by hand, before moving on. (Capture each one
  from the console; no code is named here — [`../../operate/CLAIMS.md`](../../operate/CLAIMS.md) §4.)
- `codes` with no `term_days` behave exactly as today (invariant I4).

---

## Phase 2 — Server: renew, rebind, uniqueness

**Goal:** the three operator operations exist and are audited.

- [ ] `codes.renew` — arithmetic per [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.3
      (`base = max(now, future expires_at)`), optional `term_days` override,
      `code_events` row with before→after and, per
      [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md) §8.4,
      `price` and `middleman` in the detail. **Must not clear `suspended`**,
      change the binding, or change the tier.
- [ ] `codes.rebind` — audited, reason required, **must not clear
      `expires_at`** ([`03-device-binding.md`](03-device-binding.md) §3.2 C).
- [ ] Uniqueness at activation — a new **409** (not 403) with a
      student-legible message that does **not** contain "suspended"
      ([`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md) §7.4).
- [ ] Maintain `device_bindings` on every bind, unbind and rebind.
- [ ] Change the grace-period path so it clears the **entitlement, not the
      binding** ([`04-card-and-credential.md`](04-card-and-credential.md) §4.4).

**Gate:**
- `check-consistency.sh` green.
- `activation_contract_test` updated in the same commit and passing. (Added
  2026-10-01 to `client/src-tauri/tests/activation_contract.rs`; until then this
  gate named a test that did not exist.)
- A renewal on a **test** code extends from its existing expiry, not from now —
  demonstrated, not asserted.
- An unbind does **not** change `expires_at`.

---

## Phase 3 — Console

**Goal:** the operator can renew in two clicks.

- [ ] Search: extend `codes.list`'s `q` match
      (the `codes.list` query filter) to `label`/`notes`/`middleman`
      ([`06-console-and-operator.md`](06-console-and-operator.md) §6.4), and
      update the query box's placeholder.
- [ ] **Renew** on the code detail panel, with a **before → after** preview and
      an explicit "tier/binding unchanged" display.
- [ ] **Renew** inline on the renewal worklist; make the Dashboard's
      "Expiring in 30 days" card clickable through to it.
- [ ] A 7-day worklist view, matching `EXPIRY_WARNING_DAYS`.
- [ ] A device view backed by `device_bindings`.

**Gate:**
- The whole flow performed against a **test** code: mint with a term →
  activate → renew → the code's expiry extends and the `code_events` row shows
  the before/after.
- No existing action's behaviour changed — particularly
  `codes.expire`, which is an absolute override and must keep working.

---

## Phase 4 — Client: fingerprint durability

**Goal:** close the reported defect. **Independent of phases 1–3** and can run
in parallel.

- [ ] Persist the fingerprint to a **machine-scoped, reinstall-surviving**
      location ([`03-device-binding.md`](03-device-binding.md) §3.2 A).
- [ ] Widen the degradation chain and make the random fallback durable
      (§3.2 B).
- [ ] Add a test pinning "the fingerprint is stable across a simulated restart
      with the app's own config removed" — the property that is currently
      untestable-by-construction and is the actual defect.
- [ ] Add a test that a heartbeat carrying a **later** `expires_at` moves the
      displayed window without a restart
      ([`02-term-and-renewal.md`](02-term-and-renewal.md) §2.5).
- [ ] Handle the new `409` as a distinct `ActivationOutcome` arm with its own
      sentence.

**Gate:**
- `cargo test`, `cargo clippy -- -D warnings`, `pnpm typecheck`, `pnpm test`,
  `pnpm lint` all green.
- Version bumped in all three manifests (guarded by
  `version_consistency.rs`).
- **V1–V3 in [`10-open-questions.md`](10-open-questions.md) §10.2 remain open
  until tested on real hardware.** This phase cannot be closed from here.

---

## Phase 5 — Documentation adoption

**Goal:** the live plan stops describing something the system cannot do.

Follow the order in [`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md) §9.5,
beginning with `17-not-built.md`. That file goes first because it is the
anti-drift guard, and the design is not yet code.

**Gate:** no live business file contradicts another, and
[`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md)'s table has no
unresolved **REWRITE** rows.

---

## 11.2 The verification commands, collected

```bash
# Repo-wide guards (run before every commit)
server/scripts/check-consistency.sh

# Server scripts parse
for f in server/scripts/*.sh server/modules/*.sh; do bash -n "$f"; done

# Client
cd client
cargo test
cargo clippy --all-targets --features clippy -- -D warnings
pnpm typecheck && pnpm test && pnpm lint
```

## 11.3 What "done" cannot mean yet

Recorded explicitly so nobody claims completion prematurely:

* The fingerprint properties are **unverifiable here** (V1–V3).
* A renewal propagating to a running client is **unverifiable here** (V4).
* The migration preserving paying codes is **unverifiable here** (V5) — it
  requires the live hub and the operator's eyes.
* The updater installing anything remains **unproven** (V6), which bounds how
  fast any client change can reach the field.

Everything else is testable, and the gates above are the definition of done.

## 11.4 Related reading

* The safety constraints the phases respect → [`05-migration-and-live-data.md`](05-migration-and-live-data.md)
* The wire rules they must not break → [`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md)
* The unresolved items → [`10-open-questions.md`](10-open-questions.md)
