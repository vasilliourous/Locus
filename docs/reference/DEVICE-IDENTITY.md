# Device identity and the activation code's durability

```
audience:    builder
status:      live
authoritative-for: how the client keeps a student's entitlement across an update, an uninstall or a reset, and why device identity was removed
verified-against: client/src-tauri/src/locus/credential.rs, client/src-tauri/src/locus/store.rs, server/pb_hooks/activation.pb.js
```

> **Read this before touching the activation gate, `store::read`/`clear`, or the
> `codes` collection.** It replaces the previous device-identity design, which
> was removed rather than fixed.

---

## 1. What changed, and why

There used to be a **durable device identity**: a `device_id` plus a secret,
kept in a machine-scoped file, that the hub matched on so a returning student
could skip the code prompt. Activation codes were **bound** to that identity so
one code could not be used on two devices.

Both were removed. The reasons:

1. **It never worked.** The identity was never written on the activation path,
   so recognition looked up a row that did not exist and every device looked
   unknown; and a code bound to an identity that a reinstall could change came
   back "bound to another device" for the student's own machine.
2. **It was the wrong fix for the reported problem.** The problem is "a student
   lost the code for a device they still own". Binding an entitlement to a
   device is a heavier mechanism than the problem needs, and it fails whenever
   the device is replaced — the ordinary case.
3. **The code is enough.** A code is single-use and not tied to anything. The
   client keeps it durably on the device, so a reinstall or an update does not
   ask for a card the student may have thrown away.

There is no device identity anywhere now: no `device_identities` collection, no
`bound_fingerprint`, no session token, no `/api/device-recognise`.

---

## 2. How a student keeps their access

The activation code is the whole credential. Two things make it durable.

### The code is stored in two places

| Store | Where | Survives |
|---|---|---|
| `verge.yaml` (primary) | the app's config | an app update and a restart; **not** an uninstall, and not a config reset |
| the machine store (mirror) | `/var/lib/locus` (Linux), `/Library/Application Support/Locus` (macOS), `%PROGRAMDATA%\Locus` (Windows) | an uninstall and a reinstall |

`crate::locus::credential` owns the mirror. It is written owner-only (`0o600`)
on every `store::store`, because the code is a bearer credential. On read,
`store::read` prefers the config and falls back to the mirror; `read_code_rehydrating`
writes a recovered code back into the config so the launch is not a one-off.

### The code is not erased for a local reason

`store::clear_entitlement` and `store::record_lapsed_grace` are deliberately
different:

- **`clear_entitlement`** — a definitive hub refusal (suspended, expired,
  refunded). Wipes the code, the tier, the expiry **and the mirror**.
- **`record_lapsed_grace`** — the heartbeat grace period ran out locally.
  Clears only the session state (tier, expiry, heartbeat bookkeeping) and
  **keeps the code**. A week offline is not a statement that the code is bad,
  and erasing it there is exactly how a student ends up back on the code screen
  with nothing.

The rule: **a local inability to confirm the entitlement must never destroy the
credential.** Only the hub may say a code is void.

### Re-activating a code is allowed

On the hub, `codes.activated_at` is the single-use stamp. It is set on the first
activation. A later activation of the same code is **not** refused — it returns
the tier config and reports "Already activated", restoring the student's access
on a new machine or after a reinstall. The stamp exists to tell a first
redemption from a repeat one, not to lock a code to a machine.

---

## 3. What the fingerprint is still for

`client/src-tauri/src/locus/device.rs` still computes a hardware fingerprint.
It is **not** a binding key, an identity, or a credential. It survives for two
non-authoritative uses:

- a **rate-limit and support key** carried on `/api/activate`,
  `/api/code-lookup` and `/api/heartbeat` — the hub buckets by it and truncates
  it into an attempt log; it is compared against nothing;
- the **truncated "device id"** shown in the status command, for support.

It is re-derived on every launch and may differ between runs. Nothing depends on
it being stable, which is the property that made it unfit to bind to.

---

## 4. Removing the old model

`/api/device-recognise` remains as a **tombstone** that answers a uniform
`unknown`. Deployed 3.2.x clients call it once per launch; without the route they
would get a 404 and log a transport error on a path that no longer matters. The
tombstone does no work — no lookup, no token, no verifier — and
`activation_contract.rs` asserts that.

An existing hub keeps its `device_identities` and `device_bindings` collections
and any rows in them; nothing reads or writes them. Deleting that data is an
operator decision, not something a schema script should do. A fresh hub
(`seed-pb.py`) is not given them.

The `codes.bound_fingerprint` column is likewise kept on an existing hub and
never read or written; `check-consistency.sh` fails if any hook references it.
