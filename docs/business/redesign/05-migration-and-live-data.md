# 5. Migration, and the live data it must not damage

```
audience:    human-operator
status:      design-record
authoritative-for: the migration plan against live data — gating, unexecuted
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **This is the gating file.** Read it before any code change. It exists because
> the hub is **live, holds paying customers, and has no sandbox.**

---

## 5.1 What is at stake

Two things make this migration different from a normal schema change:

1. **There is a real customer.** [`../10-lifecycle.md`](../10-lifecycle.md) §10.2
   and `docs/CONTEXT.md` both record that the live hub holds **real paid codes in
   active use, at least one already bound to a device** — count and distributor
   deliberately not recorded ([`../../operate/CLAIMS.md`](../../operate/CLAIMS.md) §4).
   That is one or more people who paid money and expect their VPN to work
   tomorrow.

2. **There is no staging environment.** Both documents also state: *"The live
   hub is production, and there is no sandbox. Stage changes on the VPS copy
   under `/root/server/`, never experiment on the live hooks."*
   Deployment is via `scp` + `setup.sh` (`server/setup.sh:4-5`).

Any change that shortens a paying student's remaining term is a **refund or a
lost customer**, and it is the failure mode most likely to be caused by a
careless implementation of [`02-term-and-renewal.md`](02-term-and-renewal.md).

## 5.2 The five invariants

Every migration step must preserve all five. They are the acceptance criteria.

| # | Invariant | Why |
|---|---|---|
| **I1** | **No existing `expires_at` value is ever shortened, cleared, or recomputed.** | This is the paying-student protection. A code with a valid future expiry keeps that exact instant. |
| **I2** | **No existing `bound_fingerprint` is cleared or changed.** | A device that is bound must stay bound. Clearing it means that student's client is refused at the next beat. |
| **I3** | **No collection or column is removed or retyped.** | The seed script's own rule (the reconciler's add-only rule in `seed-pb.py`) and the only thing keeping this reversible. |
| **I4** | **A code with no `term_days` behaves exactly as today.** | The fallback path must be the current behaviour, so an un-migrated row is not a broken row. |
| **I5** | **The deployed client keeps working unchanged.** | §5.5. No release can be forced on the installed fleet. |

## 5.3 The mechanism — already correct, extend it

Good news: the deployment tooling already implements additive-only schema
changes, deliberately. the schema reconciler in `seed-pb.py`:

```python
# Only ever ADD missing fields; never remove or retype,
# so this cannot destroy existing data.
cur = api("GET", f"/api/collections/{name}")
have = {f.get("name") for f in cur_schema}
missing = [f for f in schema if f["name"] not in have]
if missing:
    merged = cur_schema + missing
    resp = api("PATCH", f"/api/collections/{name}", {"schema": merged})
```

> **Therefore: new fields go into the `collections` list in `seed-pb.py`, and a
> `setup.sh` re-run adds them. Nothing else is required to make the columns
> exist.**

The new fields, all **additive and optional**:

| Collection | Field | Type | Purpose |
|---|---|---|---|
| `codes` | `term_days` | number | Term length. Absent/`0` = never expires. |
| `codes` | `term_kind` | text | Display label only (`month`/`term`/`year`). Never arithmetic. |
| *(new)* | `device_bindings` | collection | fingerprint → code index for uniqueness ([`archive/03-device-binding.md`](archive/03-device-binding.md) §3.1) |

`device_bindings` is the only *new collection*, and a new collection is the
safest possible change: it touches no existing row.

## 5.4 The backfill — the step that can hurt someone

The columns being added are empty on existing rows. The question is what to
**backfill**.

### The one rule

> **Derive a term where one is provable. Never touch `expires_at`. Leave
> anything ambiguous alone.**

Concretely, for each existing code:

| Condition | Action | Reasoning |
|---|---|---|
| `expires_at` is set and in the **future** | **Leave `expires_at` untouched.** Optionally set `term_days` = `expires_at − activated_at` **as documentation only**, and only when `activated_at` is also set. | I1. Any bound code keeps its exact end date. Setting the term makes future renewals consistent with the first purchase, without changing anything now. |
| `expires_at` is set and in the **past** | Leave untouched. | A lapsed code stays lapsed. Reactivating it is the operator's explicit decision, via `codes.renew`. |
| `expires_at` empty, `activated_at` set | Leave empty; set `term_days` only if the operator states a policy. | Ambiguous. Guessing a term here would **grant** time on a guess, which is as wrong as removing it. Recommend: leave it, and let the operator decide per code. |
| `expires_at` empty, never activated | Leave empty. | An unsold card with no term is a card that never expires when sold — which may be desirable or not, and is a **business** decision, not a migration's. Flag it in the console. |

**The backfill must be a separate, auditable, re-runnable script** — not part of
`setup.sh`, and not implicit in a hook. It writes a `code_events` row
(`event: "migrated"`) per touched code, so "what did the migration do to my
codes" is answerable from the same trail the operator already uses.

**Strong recommendation: print a dry-run diff first.** A one-liner that lists
every code with `old_expires_at → new_expires_at` (mostly `unchanged`), for the
operator to read before applying. On a live system with one paying customer,
this is cheap insurance and takes minutes to write.

## 5.5 Client compatibility — the constraint everyone forgets

**A deployed client cannot be updated on demand, and the updater has never
successfully installed anything** [`../17-not-built.md`](../17-not-built.md),
`docs/reference/STILL-OPEN.md`. So the migration must assume **old clients keep running**.

Reviewed against the wire ([`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md)):

| Change | Effect on a deployed client |
|---|---|
| New `term_days` / `term_kind` columns | **None.** Never sent to the client. |
| `expires_at` still an instant string or `null` | **None.** The v3.2.4 contract is explicitly additive: *"older clients simply ignore the extra key"* (the `expiryForWire` note in `activation.pb.js`). |
| A possible new `409` for a second code on a bound device | **None for existing behaviour** — a deployed client hitting it would show a `ServerError`. New clients get the proper message. See §5.6. |
| The client's stored expiry being *later* after a renewal | **None.** Already handled; it re-records every beat. |

> **Conclusion: the migration is client-compatible if and only if `expires_at`
> keeps its current type and meaning.** That is the same conclusion
> [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.2 reaches from the
> enforcement side — two independent reasons to keep `expires_at` an instant.

## 5.6 One place old clients get worse, and the mitigation

If a second code is refused with a new status code, an **old** client
(`classify()` in `locus/activation.rs`) hits its default arm:

```rust
code => ActivationOutcome::ServerError { code, message: response.message },
```

and shows the hub's raw message. That is **acceptable, and arguably correct** —
the message can be written for a human ("This device is already activated on
another code"), and `describe_outcome`'s `ServerError` arm already prefers the
hub's own message when non-empty.

**The rule that follows:** whatever message accompanies the new refusal must be
**student-legible on its own**, because the oldest clients will render it
verbatim. Do not send a terse `"conflict"`.

## 5.7 Deployment sequence

Ordered so that every step is independently safe and reversible.

| # | Step | Safe because | Reversible by |
|---|---|---|---|
| 1 | Back up `pb_data` (the existing `07-backups.sh` module / `restore.sh`) | — | Restoring the backup |
| 2 | Dry-run the backfill; **read the diff** | Writes nothing | n/a |
| 3 | Re-run `setup.sh` to add columns + `device_bindings` | Additive only (the reconciler's add-only rule in `seed-pb.py`); I3 | Dropping the new collection |
| 4 | Run the backfill for real | Derives only; touches no `expires_at`; I1 | `code_events` records every change |
| 5 | Deploy the updated hooks (`hooks-sync.sh`) | New actions are additive; existing ones unchanged | Revert the hook files |
| 6 | **Verify** — §5.8 | — | Roll back to 5 |

Note step 3 before step 5: the columns must exist before a hook reads them, or
the goja code throws. This is the same ordering the existing notes warn about
for hook changes.

## 5.8 The verification that must pass before this is "done"

Not a substitute for the live testing only the operator can do, but the
mechanical checks:

1. **`server/scripts/check-consistency.sh`** — must stay green, including its
   group 3 check that **every hook-written field exists in the schema**
   (`check-consistency.sh`, group 3). New fields written by hooks must be added
   to `seed-pb.py` in the same commit or that check fails — which is exactly the
   `unbound_at` bug class it was built for.
2. **Every bound code, explicitly** — after migration, assert by hand that its
   `expires_at` and `bound_fingerprint` are byte-identical to before.
3. **A dry-run diff reviewed by the operator** (§5.4).
4. **The client suites** — `cargo test`, and the wire-shape test
   `the_active_wire_shape_is_camel_case` (`the_active_wire_shape_is_camel_case`) must still
   pass, proving the client contract did not move.

## 5.9 Rollback

Stated because "we can always roll back" should be a plan, not a hope:

* **Hooks** are files; `hooks-sync.sh` copies them. Reverting the files and
  re-running it restores the previous behaviour. No data change is involved.
* **New columns** are additive and unused by old code, so they can be left in
  place inertly. There is no need to drop them, which is the safest rollback —
  **do nothing**.
* **`device_bindings`** can be left in place. It is a new collection with no
  reader if the hooks are reverted.
* **The backfill** is the only step that writes to existing rows, and it writes
  only `term_days`/`term_kind` plus a `code_events` row. Reversing it is
  clearing those two fields; the event rows are the record of what happened.

> **The migration is designed so that the only destructive possibility is a bug
> in the backfill, and the backfill is designed to write two optional columns
> and nothing else.**

## 5.10 Related reading

* The term model being migrated to → [`02-term-and-renewal.md`](02-term-and-renewal.md)
* The binding this must not disturb → [`archive/03-device-binding.md`](archive/03-device-binding.md) §3.4
* The wire names that must not move → [`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md)
* The verification gates → [`11-phased-implementation.md`](11-phased-implementation.md)
