# 3. Device binding — the rule, the refusal, and the lifecycle

```
audience:    builder
status:      design-record
authoritative-for: the OLD one-code-per-device rule — REMOVED 2026-10
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> "One code per device" is enforced by a unique index and a 409 refusal. This
> file covers how it works, the state machine it creates, and the one bug this
> work introduced and fixed.
>
> **⚠️ REMOVED 2026-10 — read this before believing any present-tense sentence
> below.** This file describes a design that was **removed, not shipped**. There
> is no one-code-per-device rule, no unique index in play, no 409 refusal, and no
> `codes.rebind` write path. The live model is: a code is **single-use** and tied
> to **no** device (`codes.activated_at` is the whole record), and the *client*
> keeps the code durably so a reinstall does not lose it. See
> [`../../../reference/DEVICE-IDENTITY.md`](../../../reference/DEVICE-IDENTITY.md).
>
> Kept as the **design record of a path deliberately not taken** — the reasoning
> is why the shipped decision is the opposite one. Everything below is
> past-tense in intent, whatever tense it is written in. The two surviving
> fragments are the `409 → DeviceAlreadyActivated` client match arm (harmless,
> never fired now) and the `.pb.js` hooks' comments.

---

## 3.1 The rule

A device (identified by its fingerprint) may hold **one live code**. Activating
a second, different code on the same device is refused.

The rule is enforced **twice, at different levels**, and both layers matter:

| Layer | Mechanism | Cannot be forgotten because |
|---|---|---|
| Schema | `device_bindings.fingerprint` is UNIQUE | The database refuses the write |
| Application | `deviceBoundToOtherCode()` in `activation.pb.js` | — it *can* be wrong, which is why the schema layer exists |

The application check provides the **legible refusal**; the schema provides the
**guarantee**. Neither alone is enough: a schema violation would surface as an
opaque 500 to a student, and an application check has already failed silently
twice in this project's history.

## 3.2 The refusal is 409, not 403

`activation.pb.js` returns **403** for *two* different conditions already:

* `"Code suspended"`
* `"Code bound to another device"`

and the client disambiguates **by substring** — it looks for the word
`suspended` in the 403's message (the 403 classifier in `locus/activation.rs`). That is
fragile, and the code says so: it is the existing contract with deployed
clients, pinned by `activation_contract_test`
(`client/src-tauri/tests/activation_contract.rs`; added 2026-10-01 — the claim
preceded the test).

Adding a third meaning to 403 would have made an old client report the wrong
account state. So the new refusal is:

| | |
|---|---|
| **Status** | `409` |
| **Message** | Must be student-legible on its own, and must **not** contain "suspended" |

Two reasons for the wording rule:
1. The oldest builds in the field have no 409 arm, so the message arrives through
   their generic `ServerError` path and is rendered **verbatim**.
2. A message containing "suspended" would be misread by the 403 classifier if
   the status ever changed.

The client's new arm:

```rust
409 => ActivationOutcome::DeviceAlreadyActivated,
```

and the sentence it shows names the **action**, not the policy: *"contact the
person who sold you this code and they can move it onto this device for you."*

`DeviceAlreadyActivated` is deliberately distinct from `BoundToAnotherDevice`.
They are the same mistake in opposite directions — "this code belongs to
someone else" versus "this machine already belongs to another code" — and the
student's next step differs, so collapsing them would send them to a middleman
with the wrong question.

## 3.3 The binding lifecycle

Every state, and how it is reached:

```
                 activate (new device, no binding)
        ┌────────────────────────────────────────────────┐
        │                                                ▼
   [ no row ] ──────────────────────────────────────► LIVE
        ▲                                     │         │
        │                          unbind ────┘         │ rebind
        │                          rebind               │
        │                          delete (force)       │
        │                                               ▼
        └────────────── claim again ◄──────────── RELEASED
                                (revives the same row)
```

| Transition | Written by | Effect on the row |
|---|---|---|
| activate | `activation.pb.js` → `recordBinding` | insert, or **revive** a released row |
| unbind | `codes.unbind` → `releaseBinding` | `released_at`, `release_reason` |
| rebind | `codes.rebind` → `releaseBinding` + `claimBinding` | release the old, claim the new |
| delete (force) | `codes.delete` → `releaseBinding` | release, so the device is not left blocked |

### The bug this work introduced, and fixed

Adding the index created a failure that did not exist before:

> `codes.delete` can force-delete a **bound** code. Under the old model that was
> harmless — the code was gone, so the binding was gone with it. With an index,
> the row survived pointing at a code that no longer existed, and the
> uniqueness check would then refuse **every** future activation on that device
> — **with no code left on the hub for an operator to unbind.** A deletion would
> have permanently bricked the machine.

`codes.delete` now releases the binding first. The state is also *visible*
rather than silent: `devices.list` and `device.get` both report `code_missing`,
and the Devices page counts it on a card that turns red when non-zero.

This is worth recording because it is the class of bug that only appears in
production: correct in every individual piece, wrong in the composition.

## 3.4 `codes.rebind` — why it is separate from `codes.unbind`

A student replacing a laptop is routine. The old route was unbind → re-activate,
which **works**, but `codes.unbind` clears `activated_at`, and `activated_at`
is the input a term is derived from. Under the term model, an expiry recomputed
from a fresh activation would silently reset time the student had already paid
for.

So `codes.rebind`:

* changes **only** `bound_fingerprint`,
* **never touches `expires_at`**,
* requires a **reason** (into the audit trail),
* accepts an optional `expected_fingerprint` so a rebind cannot be
  fat-fingered onto the wrong code — if it does not match the current binding,
  the action refuses and changes nothing.

The rule an operator should remember: **move a device with Rebind, not Unbind.**

## 3.5 Fingerprint normalisation — must agree everywhere

Fingerprints are stripped to `[a-zA-Z0-9]` before every index read or write,
**and before being written to a code's `bound_fingerprint`**. The code and the
index must hold the *same* value, or the two halves of "one code per device"
disagree: the code names a device the index cannot find, so the check misses it.

| Site | Purpose |
|---|---|
| `activation.pb.js` → `deviceBoundToOtherCode` | index lookup (the check) |
| `activation.pb.js` → `recordBinding` | index lookup **and** write |
| `activation.pb.js` → the bind path | writes the code's `bound_fingerprint` |
| `activation.pb.js` → the re-activation compare | `boundFp !== incomingFp` |
| `code_lookup.pb.js` → the pre-check compare | `bound_this_device` vs `bound_other` |
| `admin_console.pb.js` → `bindingRow` | index lookup (admin) |
| `admin_console.pb.js` → `claimBinding` | index write |
| `admin_console.pb.js` → `codes.rebind` | writes the code's `bound_fingerprint` |
| `codes.list` → the per-row `binding_unindexed` probe | index lookup (console) |

> **This was actually buggy, in three places, and the bug was silent.**
>
> * `recordBinding` looked up the *stripped* value but inserted the *raw* one.
> * The bind path wrote the raw `fp` to the code's `bound_fingerprint` while the
>   index got the stripped one.
> * Both compare sites (re-activation, and `code_lookup`'s pre-check) tested a
>   stored *stripped* value against an incoming *raw* fingerprint.
>
> Real fingerprints are hex, so the two values coincide and none of it fired —
> but a fingerprint carrying any strippable character would have (a) let the
> same device bind a second code, and (b) told a returning student their own
> code was "bound to another device". Both failures are silent: the writes
> succeed, the rows look correct, and only the *check* stops matching.
>
> All three are fixed by normalising once and using that single value for both
> halves. **`check-consistency.sh` group 6(e) now enforces it** — a bare
> `set("fingerprint", fp)` fails the guard, verified by reintroducing the bug.

The rule for any future change: **normalise first, and use the same normalised
value for the lookup, the write, and every comparison.**

## 3.6 The consistency check in `codes.list`

Each row reports `binding_unindexed`: **true** when the code holds a
fingerprint the index cannot see.

This is a real reachable state — a code bound before the index existed, or an
index row an operator released while the code still holds a fingerprint — and in
that state the one-code-per-device rule **cannot enforce**. So it is surfaced
(a `⚠ unindexed` marker on the row, and a line in the detail panel) rather than
discovered later, when it would look like the rule failing rather than being
unable to see.

Repairing it is the ordinary path: re-activating the device calls
`recordBinding`, which brings the two into agreement.

## 3.7 What is deliberately NOT enforced, and what that costs

The client's fingerprint still falls back to a value persisted **only in the
client's own config** (`locus/device.rs`). Lose that file — a reinstall, a
reset, a wiped AppData — and the device presents a new identity.

With the binding rule now enforced, that means: **a reinstalling student is
refused at 409 and needs an operator.** Before this work they would have
silently taken over their own code; now they are correctly refused and
correctly stuck.

That is the honest trade: the piracy hole is closed, and the false-positive
support cost is real. The mitigation is the escape hatch — an audited rebind,
found by the name in `label` — which is why search-by-name is load-bearing
rather than a nicety. The fingerprint fix itself is deferred; see
[`05-not-yet-true.md`](05-not-yet-true.md) §5.2.
