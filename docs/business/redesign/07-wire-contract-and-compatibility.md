# 7. Wire contract and compatibility

> **The rule:** a deployed client cannot be forced to update — the updater has
> never successfully installed anything — so every wire change must be
> **additive**, and every frozen name must stay frozen.
>
> Read this before renaming a field, changing a status code, or altering the
> shape of any response.

---

## 7.1 Why this is a hard constraint

Two facts, both verified:

1. **The updater has never installed a build.** `docs/reference/STILL-OPEN.md` states it
   twice, and [`../17-not-built.md`](../17-not-built.md) lists it. Whatever is
   deployed in the field stays deployed until someone installs an update by
   hand.
2. **The consistency guard exists precisely for this class of bug.**
   `server/scripts/check-consistency.sh`'s header names the four worst incidents
   in the project's history, all of them "a value that must agree across
   languages with nothing enforcing agreement":

   * `uot_port` vs `server_port_uot` — **"a SILENT no-op. UDP-over-TCP was dead
     fleet-wide while both sides were individually correct."**
   * `download_<platform>` vs `update_<platform>` — **"a Windows client was
     handed a Linux binary."**
   * `unbound_at` / `unbind_reason` — **"PocketBase silently discarded them, so
     an audit trail looked implemented and recorded nothing."**
   * `aes-256-gcm` duplicated in ten places with nothing binding them.

The redesign adds fields. Every one of them must be checked against this file
and against `check-consistency.sh`.

## 7.2 The frozen names — do not touch

These are contracts with deployed builds. "Tidying" any of them is a silent
outage.

| Name | Where | Why frozen |
|---|---|---|
| **`uot_port`** | `tier_configs`, emitted verbatim through `/api/activate` and `/api/heartbeat` | Written by the seed and hooks, read by the client's `contract.rs`. Documented in `contract.rs`, `heartbeat.pb.js`, and `check-consistency.sh` group 2. Directly caused a fleet-wide UDP outage once. |
| **`download_<platform>`** | `update_config` | Emitted by `publish-release.sh`, read by `/api/update`. The publish-side name; the client's rename to `update_*` is deliberate. |
| **`update_<platform>`** | the heartbeat's update signal | The client-side name. The asymmetry with `download_*` is intentional and guarded. |
| **`macos_intel` / `macos_arm`** | platform keys everywhere | The `macos_*` keys map to `darwin-*` **artifacts**. Both `check-consistency.sh` group 1 and the CI manifest builder (`client.yml`, "The keys say `macos_*`, the files say `darwin-*`: that asymmetry is frozen and must not be 'tidied'") assert it. |
| **`expires_at`** | all four hooks, the client's whole `expiry.rs` | See §7.3. |
| **`days_remaining` → `daysRemaining`** | `SubscriptionStatus::Active` | Pinned by `the_active_wire_shape_is_camel_case` (`the_active_wire_shape_is_camel_case`), which fails if `rename_all_fields` is removed. |

## 7.3 `expires_at` must stay an instant — from three independent directions

[`02-term-and-renewal.md`](02-term-and-renewal.md) §2.2 argues this from the
enforcement side. Two more reinforce it:

**From the wire.** It is currently `string | null`, and
the `expiryForWire` note in `activation.pb.js` documents the contract explicitly:

> *"ADDITIVE ONLY, and the same shape /api/heartbeat already sends... Never an
> empty string: an unset expiry must be `null`, because the client
> distinguishes 'no date recorded' from 'the date is blank' to avoid showing a
> paying student 'expired' on a code that simply has no expiry set."*

**From the client's enforcement.** `Expiry::is_lapsed` in `locus/expiry.rs`:

> *"**Only a known past date lapses.** `Unknown` deliberately does not: a hub
> that has not told us a date, or a code with none recorded, must never be read
> as expired — that would end a valid subscription on a guess."*

So the `null` case is **load-bearing for correctness**, not a convenience. A
design that replaced `null` with a sentinel date would end real subscriptions.
The term model must therefore express "never expires" as `expires_at = null`
and a `term_days` of `0`/absent — never as a far-future date, which would be a
lie the UI would render as a real date.

## 7.4 The `403` dual meaning — and the new refusal

`activation.pb.js` returns **403** for both *suspended* and *bound to another
device*, and the client disambiguates **by substring**
(the 403 classifier in `locus/activation.rs`):

```rust
403 => {
    if response.message.to_ascii_lowercase().contains("suspended") {
        ActivationOutcome::Suspended
    } else {
        ActivationOutcome::BoundToAnotherDevice
    }
}
```

The code names this as fragile and as an existing contract with deployed
clients, pinned by `activation_contract_test`
(`client/src-tauri/tests/activation_contract.rs`).

> **Correction (2026-10-01).** That test did not exist when this file was
> written — see the note in [`03-device-binding.md`](03-device-binding.md) §3.3.
> It was added on 2026-10-01 by the durable-device-identity work.

**Rules for the new `DeviceAlreadyActivated` refusal** ([`03-device-binding.md`](03-device-binding.md) §3.1)
— **all four followed** in `9a91da1`:

1. **Use a status code that is not 403.** `409 Conflict` is the natural choice
   and cannot be misinterpreted by any deployed build.
2. **Never put the word "suspended" in the message**, or an old client would
   show it as a suspension — a wrong account state with the wrong instructions.
3. **Write the message for a student**, because the oldest clients render it
   verbatim through the `ServerError` arm.
4. **Update `activation_contract_test` in the same commit.** That test exists
   to catch exactly this — and as of 2026-10-01 it actually does.

## 7.5 What the redesign adds to the wire — the complete list

Deliberately short. Anything not on this list should not be sent.

| Addition | Where | Client impact |
|---|---|---|
| `term_days`, `term_kind` | `codes` collection | **Never sent to a client.** Internal to the hub and console. |
| A new `409` on activation | `/api/activate` | Only reachable in a situation that is currently mis-handled anyway. New clients get a proper outcome arm. |
| `codes.renew` / `codes.rebind` | the admin endpoint | **Not a client-facing API.** Operator only. |
| `device_bindings` | new collection | Internal. |

**Nothing is added to `/api/heartbeat` or `/api/update` responses.** The
heartbeat already carries `expires_at`, which is all a renewal needs to
propagate. This is a deliberate restraint: the fewer new wire fields, the fewer
cross-language agreements to enforce.

## 7.6 The checks that must stay green

Every one of these is a mechanical guard already in the tree. The redesign does
not weaken any of them.

| Guard | Command / test | What it protects |
|---|---|---|
| Cross-language consistency | `server/scripts/check-consistency.sh` | Platform keys, frozen wire names, hook-written fields vs. schema, the single cipher definition, Windows-illegal paths |
| Hook-written fields exist | `check-consistency.sh` group 3 | The `unbound_at` class — a hook writing a field the schema lacks, silently discarded |
| PocketBase hook traps | `check-consistency.sh` group 6 | File-scope helpers, `findRecordsByFilter`, non-constant-time token compare, headers read from the wrong place |
| Version agreement | `client/src-tauri/tests/version_consistency.rs` | The three manifests agreeing |
| The wire shape | `the_active_wire_shape_is_camel_case` | `rename_all_fields` on `SubscriptionStatus` |
| Activation messages | `activation_contract_test` (`tests/activation_contract.rs`) | The 403/409 substring contract against the live hook |

**New requirement this redesign introduces:** `term_days` will be written by
hooks, so it must be in `seed-pb.py` **in the same commit** or group 3 fails.
That is the guard working as intended, not an obstacle.

## 7.7 Related reading

* The term model that keeps the instant → [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.2
* The migration this constrains → [`05-migration-and-live-data.md`](05-migration-and-live-data.md) §5.5
* The refusal being specified → [`03-device-binding.md`](03-device-binding.md) §3.1
