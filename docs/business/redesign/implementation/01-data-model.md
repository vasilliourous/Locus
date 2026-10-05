# 1. Data model — what was added, and who touches it

```
audience:    builder
status:      design-record
authoritative-for: the schema and who writes each field
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> Every field below exists in `server/scripts/seed-pb.py` and is written by a
> hook. The consistency guard (`server/scripts/check-consistency.sh`, group 3)
> asserts that every field a hook writes exists in the schema. If that count
> drops, a field was lost; if a hook writes a field the schema lacks, the guard
> fails. **Read the current count from the guard's own output rather than from
> this sentence** — it is a derived fact and it changes as hooks are added:
> `bash server/scripts/check-consistency.sh | grep 'hook-written'`.
> The five this work added are exactly: `term_days`, `term_kind` (on `codes`)
> and `bound_at`, `released_at`, `release_reason` (on `device_bindings`).
> **2026-10 note:** the `device_bindings` half is removed — that collection is
> kept on an existing hub but read and written by nothing. The two `codes` fields
> (`term_days`, `term_kind`) remain live.
> Re-deriving the delta: diff the `.set("…")` calls in `server/pb_hooks/*.js`
> against the `{"name": …}` entries in `seed-pb.py`.

---

## 1.1 `codes` — two added fields

| Field | Type | Written by | Read by |
|---|---|---|---|
| `term_days` | number | `codes.generate`, `codes.set-term` | `expiryFromTerm` (activation), `codes.renew` |
| `term_kind` | text | `codes.generate`, `codes.set-term` | **Nothing.** Display only |

### `term_days` — how long one purchase lasts

Measured from **activation**, not from mint. `0` or absent means **never
expires**, which is a legitimate product and also the state every un-migrated
row is in.

**The invariant that matters:** a code with `term_days` of `0`/absent must
behave exactly as it did before this feature existed. It is not a broken row,
it is a permanent code. Verified directly — see
[`06-verification.md`](06-verification.md) §6.3.

### `term_kind` — a label, deliberately not a number

`"term"` / `"month"` / `"year"` / `""`. **It is never used in arithmetic.**

This is deliberate and worth preserving. A conversion like "month = 30 days"
living in two places (the label and the arithmetic) is a rule that will
disagree with itself in one of them, and the failure would be silent — a
renewal that gives 30 days when the operator meant a term. `term_days` is the
number; `term_kind` is what the operator reads.

## 1.2 `codes` — the existing fields this work now depends on

These were already present; what changed is that something now *reads* them.

| Field | New significance |
|---|---|
| `activated_at` | Was written and never used for anything but display and sorting. It is now the basis a derived term is proven from during migration. **Nothing computes an expiry from it at runtime** — the expiry is materialised instead (see §1.4). |
| `expires_at` | **Unchanged in type and meaning.** Still the single enforcement field. |
| `bound_fingerprint` | Still the code's record of its device. Now also mirrored into the index (§1.3). |
| `label`, `notes` | Now **searched** by `codes.list`, not merely returned. This is what makes finding a student by name possible. |

## 1.3 `device_bindings` — the new collection

```
fingerprint   text, required, UNIQUE
code          text
tier          text
bound_at      date
released_at   date
release_reason text
```

### Why the `fingerprint` field is UNIQUE

This is the enforcement of "one code per device". It is a **schema
constraint, not an application check**, and that choice is the point: this
project has already been burned twice by application-level checks that silently
did not run — a rate limit that returned zero rows for months because
`findRecordsByFilter` is broken on this PocketBase build, and an audit column
PocketBase discarded because it did not exist. A constraint the database
enforces cannot be forgotten by a future hook.

### Why rows are kept rather than deleted

A released binding sets `released_at` and keeps the row, the same append-only
reasoning as `code_events`. "Has this device ever been bound, and to what" stays
answerable, and a released-then-reused device revives its own row rather than
inserting a second one (which the unique index would refuse anyway).

### Why `code` is a plain string and not a relation

A relation would **null out** when the code row is deleted, silently freeing
the device to bind again — which is the exact failure this collection exists to
prevent. A plain string survives the deletion, and the disagreement is then
*visible* (`code_missing`) rather than silent.

## 1.4 The one design decision that shaped everything: materialise, don't derive

A term could have been applied at read time — every consumer computing
`activated_at + term_days`. It is not.

> **The expiry is computed once, when the entitlement changes, and written into
> `expires_at`. The term is only how the value is produced.**

There are four readers of `expires_at` on the hub
(`activation.pb.js`, `heartbeat.pb.js`, `code_lookup.pb.js`, and the console),
plus the client's entire `expiry.rs`, plus the wire format. Deriving at read
time would have meant teaching all of them the arithmetic, and any one of them
could then disagree with the others — the failure mode
`check-consistency.sh` exists to catch.

Because the value is materialised:

| Path | Changed by this work? |
|---|---|
| The 410 expiry gates in `activation.pb.js` / `heartbeat.pb.js` | **No** |
| `expiryForWire` → the client's `expires_at` | **No** — still an instant, or `null` |
| `locus/expiry.rs` (parse, `is_lapsed`, `ExpiryState`) | **No** |
| The connect gate `subscription_allows_connect` | **No** |
| The `rename_all_fields` wire-shape test | **No** |

The one place the two meet is `codes.set-term`, which sets a term and
**deliberately does not move the expiry** — so a code's term can be corrected
without silently rewriting the date a paying student is running on.

## 1.5 The `code_events` trail — new event names

`code_events` is unchanged; this work adds writers to it.

| `event` | Written by | Detail recorded |
|---|---|---|
| `renewed` | `codes.renew` | `old → new (term_days=N, price=P, middleman=M)` |
| `term-set` | `codes.set-term` | `old → new — expires_at NOT changed` |
| `rebound` | `codes.rebind` | `old_fp → new_fp (reason)` |
| `unbound` | `codes.unbind` | the reason (pre-existing) |
| `deleted` | `codes.delete` | now also releases the binding index |

The `renewed` detail carries `price` and `middleman` **in the string, not as
columns**. Cash is collected by middlemen, and once a code can be renewed
repeatedly, "how much did this middleman collect" stops being derivable from a
code count. Recording it now costs nothing; it cannot be reconstructed later.
Promoting it to real columns is a schema change that should wait until there
are renewals to model.

## 1.6 What was *not* added, on purpose

| Not added | Why |
|---|---|
| A `customers` collection | The hub stores **no PII** by decision. Identity lives on paper and in `label`/`notes`. See [`../01-identity-without-pii.md`](../01-identity-without-pii.md). |
| A `subscriptions` table | The design record proposed one; the implementation did not need it. An entitlement is a code plus a binding, and adding a third entity would have created a second source of truth for the same facts. |
| Any new field on `tier_configs` / `update_config` | Untouched. This work is orthogonal. |

The second row is worth noting as a *design record versus implementation*
divergence: `../02-term-and-renewal.md` sketched a separate subscription
entity. Once the expiry was materialised and the binding was indexed, nothing
needed it. **The code wins**, and the extra table would have been complexity
with no reader.
