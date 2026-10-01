# Device identity and code-free re-activation

```
audience:    builder
status:      live
authoritative-for: how a device is identified, how a reinstall keeps its entitlement, and the wire contract for device recognition
verified-against: client/src-tauri/src/locus/identity.rs, server/pb_hooks/device_recognise.pb.js, server/pb_hooks/heartbeat.pb.js
```

> **Read this before touching device binding, the fingerprint, or the activation
> gate.** It replaces the fingerprint's role as the primary device identity. The
> fingerprint still exists and is still sent; it is no longer what an entitlement
> hangs on.

---

## 1. The problem this solves

Two symptoms, reported as separate issues, that turned out to be **one defect**:

1. A code bound to a device stops being recognised as bound to that device —
   often noticed around an update, but not caused by one.
2. An uninstall/reinstall loses the code, and a student who threw the paper card
   away has no self-service recovery.

### The actual cause: the identifier was not durable

`bound_fingerprint` was never wrong. The device presented a **different** value,
so the hub's `boundFp !== incomingFp` guard fired — the guard working exactly as
designed against an input that should never have changed.

`client/src-tauri/src/locus/device.rs::compute()` derived the fingerprint from
hardware on every launch, degrading through weaker combinations, and **finally to
a random value**:

```rust
fn compute() -> String {
    match platform_sources() {
        Some(fingerprint) => fingerprint,
        None => random_fingerprint(),
    }
}
```

That random value was persisted **only in the app's own config** — the same file
an uninstall deletes. So:

| Situation | Before |
|---|---|
| Hardware source changes (machine-id, NIC, hostname) | Derived value changes → the hub sees a new device |
| All sources empty (bare VM/container) | Random value |
| App config wiped, or a reinstall | Random value is **not re-derivable** → a new random one |

The last row is the killer, and it is the same root cause for **both** reported
symptoms.

**Note the cache scope.** `fingerprint()` is a `OnceLock` — cached for the
*process* lifetime only. Its own doc comment already describes the across-install
version of this bug without naming it.

---

## 2. The design

Identity is now **three distinct things**, and conflating them is the mistake the
design exists to prevent:

| Part | What it is | Who may see it |
|---|---|---|
| `device_id` | A stable, **non-secret** name | Logs, support, diagnostics (truncated to 12 chars) |
| `secret` | 32 random bytes — the actual credential | **Nobody.** Never logged, never exported |
| `sha256(secret)` | The verifier the hub stores and compares | The hub only |

**Why the split is a security boundary, not tidiness.** A `device_id` is a
*name*: safe to display, useless to an attacker. A `secret` is a *claim*: it must
be **proven**, not merely presented. If the id alone could re-obtain an
entitlement, then anyone who read a support screenshot could take over an
account. That is the IDOR class this split prevents.

### Resolution order — the actual fix

`client/src-tauri/src/locus/identity.rs::resolve()` is a **pure function** over
an injected `IdentitySources`:

1. **A stored identity wins, verbatim** — whatever the hardware now reports.
   This is the core fix. A reinstall finds the machine store and reuses the
   identity instead of re-deriving something the hub has never seen.
   *Consequence, intended:* a device whose hardware changed but whose stored
   identity survived keeps its entitlement. The student did not change devices;
   their MAC did.
2. **Otherwise derive from the machine id** — a stable *name*, but still a
   **random secret**, because a hardware identifier is guessable and therefore
   unfit to be a credential. Deterministic in the machine, so an upgraded build
   can recompute the id and recognise a device activated *before* this module
   existed.
3. **Otherwise generate** — fresh random secret, id derived from it.

### Durability is reported, never assumed

| `Store` | Survives a reinstall? |
|---|---|
| `Machine` — `/var/lib/locus` (Linux), `/Library/Application Support/Locus` (macOS), `%PROGRAMDATA%\Locus` (Windows) | **Yes** |
| `AppFallback` — the app's own config | **No** — and `describe()` says so |
| `Fresh` — generated this run, not yet written | No |

`AppFallback` is used only when the machine store is unwritable without
elevation. **The UI must not promise durability the install does not have** —
over-claiming here is exactly the support call the feature exists to remove.

### Testability without hardware

`resolve()` takes no filesystem, no `/etc/machine-id`, no keychain. That is
deliberate: the property that matters most ("the identity survives a reinstall")
is otherwise checkable only on real Windows and macOS hardware — which is
precisely why the fix sat **deferred** in
[`../business/redesign/03-device-binding.md`](../business/redesign/03-device-binding.md)
§3.5 and
[`../business/redesign/implementation/05-not-yet-true.md`](../business/redesign/implementation/05-not-yet-true.md)
§5.2 for months.

The I/O layer (read/write/`resolve_with_stores`) is separate and tested against
`std::env::temp_dir()`, following the existing pattern in `update/apply.rs`.

---

## 3. Recognition: skipping the code prompt

On first launch with nothing stored, the client asks the hub whether it already
knows this device. If it does, the code prompt is skipped.

### `POST /api/device-recognise`

**Request:**

```json
{
  "verifier": "<sha256(secret), 64 hex>",
  "device_id": "<non-secret device name>",
  "store": "machine" | "app"
}
```

**Response (`recognised`):**

```json
{
  "status": "recognised",
  "tier": "strike",
  "expires_at": "2027-03-14 00:00:00.000Z",
  "store": "machine",
  "server_config": { "server": "…", "server_port": 8445, "uot_port": 8446, … },
  "udp_relay": true,
  "token": "<session token>",
  "token_expires_at": "2026-10-31T…Z"
}
```

**Every other outcome is the same body:**

```json
{"status": "unknown", "message": "This device is not recognised"}
```

### The three rules that make it safe

**1. It never returns the activation code.** The code is a bearer credential for
the entitlement. If this response carried it, anyone able to produce a matching
identity could read it out and walk away. The client does not need it — the tier
config is what builds a tunnel. This is enforced by a test
(`recognition_never_returns_the_activation_code`) that scans every
`response.<field> =` assignment against an allow-list.

**2. Authentication is proof, not assertion.** The verifier is `sha256(secret)`,
which only a device actually holding the secret can produce. The `device_id` is
**not** used for authorisation — it is a diagnostic label that appears in support
logs.

**3. Every negative is identical.** Unknown, revoked, malformed, and
nothing-entitled all return `{"status":"unknown"}`. Distinct shapes would let a
caller learn which ids or verifiers are real. Rate-limited per **IP**, not per
verifier — keying on the verifier would let an attacker spread guesses across
many and never trip the limit. The verifier is **never** written to the
`activation_attempts` log; only a redacted device-id prefix is.

Expiry and suspension are re-checked on **every** recognition by re-reading the
code row, so a reinstall cannot revive a lapsed or suspended entitlement. The
identity's `code` column is a string copy; the code row is the authority.

---

## 4. The session token — and why it is not optional

A recognised device has **no activation code**. But `/api/heartbeat` is
code-keyed, and **every enforcement rule runs there**: suspension, expiry, config
refresh, the update signal.

> A recognised device without a credential could connect and then **never check
> in again**. A suspended device would keep working forever — worse than not
> recognising it at all.

So recognition mints a **device-scoped, revocable session token**:

- `/api/heartbeat` accepts either `code` **or** `token`.
- The hub resolves a token to its code **first**, so every check below runs on
  the same `code` regardless of which credential was used.
- Only `sha256(token)` is stored, in `device_identities.token_hash` (UNIQUE).
- 30-day life, re-minted on every recognition.
- Failure to mint/store a token makes recognition report **`unknown`** — a device
  that cannot heartbeat is not a restored device.

### The client side of a lapsed token

A 401 from the heartbeat means the **credential** is stale, not the entitlement.
The client maps it to its own outcome, `BeatOutcome::StaleCredential`, which is
neither a refusal (nothing is torn down — the student's access was never in
question) nor an `Unreachable` (which would retry the same dead token on a
doubling schedule forever). The supervisor responds by re-running recognition and
restarting the loop with a fresh token.

**Do not fold 401 into the 403/404/410 refusal set, and do not let it fall
through to the catch-all.** Either choice is a real bug: the first kills a
working tunnel for a student who merely had not opened the app in a month; the
second never recovers.

---

## 5. Wiring map

### Hub

| File | Role |
|---|---|
| `server/scripts/seed-pb.py` | The `device_identities` collection (schema authority) |
| `server/pb_hooks/device_recognise.pb.js` | `/api/device-recognise` — mints the token |
| `server/pb_hooks/heartbeat.pb.js` | Resolves a token to its code before enforcing |

`device_identities` fields: `verifier` (UNIQUE), `device_id`, `code`, `store`,
`first_seen_at`, `last_seen_at`, `token_hash` (UNIQUE), `token_expires_at`,
`revoked_at`, `revoke_reason`.

**Why a separate collection, not a column on `device_bindings`:** a binding is
about a *code* and is created at activation; an identity is about a *device* and
exists **before any code is entered** — which is the entire point, since that is
what lets a returning device skip the prompt. A device with no entitlement yet
has an identity and no binding.

### Client

| File | Role |
|---|---|
| `src-tauri/src/locus/identity.rs` | The durable identity (pure core + I/O) |
| `src-tauri/src/locus/contract.rs` | Wire types: `RecogniseRequest/Response`, `Recognised`, `HeartbeatRequest` |
| `src-tauri/src/locus/activation.rs` | `recognise()`, `Recognition`, `store_label()` |
| `src-tauri/src/locus/heartbeat.rs` | `Credential`, `StaleCredential`, `beat()` |
| `src-tauri/src/locus/runtime.rs` | `start` / `start_with` / `renew_credential` |
| `src-tauri/src/locus/store.rs` | `read_identity`, `store_identity`, `device_token`, `store_device_token` |
| `src-tauri/src/config/verge.rs` | `locus_device_id`, `locus_device_secret`, `locus_identity_store`, `locus_device_token`, `locus_token_expires_at` |
| `src-tauri/src/cmd/locus.rs` | `locus_recognise`, `resolve_identity()` |
| `src/main.tsx` | The first-launch recognition attempt in the activation gate |

---

## 6. Debugging

### "The student is asked for a code again after a reinstall"

Check, in order:

1. **The store.** `locus_identity_store` in the config says `machine` or `app`.
   `app` means the machine store was not writable — the reinstall was never going
   to keep the identity, and recognition will correctly miss.
2. **The hub row.** `device_identities` for that device's `verifier`. A row with
   `code` empty means the device registered but never activated.
3. **The code row.** Recognition re-reads it; an expired or suspended code
   answers `unknown` on purpose.
4. **The token.** `token_expires_at` in the past → the device must re-recognise.

### "The tunnel works but the account never updates"

The device is connected but not heartbeating. Look for
`[locus] refusing to start a heartbeat with no credential` in the log — that means
neither a code nor a token was stored, and the loop refused to start rather than
hammering the hub with empty credentials.

### Reading the logs

- `[locus] device recognised; restored the <tier> entitlement` — the happy path.
- `[locus] heartbeat credential is stale, renewing` — the token lapsed; renewal
  is running.
- `[locus] could not renew the credential` — renewal failed. The tunnel is
  deliberately **not** torn down; the grace period still bounds it.
- `[locus] recognised device has no tier config for <tier>` — operator error: the
  `tier_configs` row is missing. The device is entitled but cannot connect.

**Never log the secret, the verifier, or the token.** Device ids are logged
truncated (`store::redact`, 12 chars).

### Re-verifying the guards

```sh
cd client && cargo test --manifest-path src-tauri/Cargo.toml --test activation_contract
bash server/scripts/check-consistency.sh
```

---

## 7. What is NOT verified

Stated plainly, because this is the part most likely to be assumed working.

- **Nothing here has run against a real PocketBase.** There is no PB runtime in
  this checkout. The hooks are syntax-checked (`node --check`) and covered by the
  `check-consistency.sh` trap wave — which is the same standard every other hook
  in this project meets, and it is not a substitute for running them.
- **The schema changes are unapplied** to the live database. `seed-pb.py` is
  additive-only and idempotent, but this collection has never been created on the
  hub.
- **The token renewal path** (`StaleCredential` → `renew_credential`) needs a real
  hub to return a 401. It compiles and is reasoned; it has never executed.
- **"The identity survives a real reinstall"** still needs a real Windows and a
  real macOS machine, an install → uninstall → reinstall cycle, and confirmation
  that the machine-scoped path is writable **without elevation** in each
  deployment shape. The design moves as much of this as possible out of the
  "cannot verify here" bucket, but it does not close it.

### Deliberately left undone

- `locus_status` does not yet report `durable`, so the account screen cannot warn
  "remembered only until reinstall". The value exists on `RecognitionResult`.
- `codes.rebind` (operator-facing, a different problem from this one) is specified
  in [`../business/redesign/03-device-binding.md`](../business/redesign/03-device-binding.md)
  §3.2 Part C and is **not** part of this work.

---

## 8. Related

- [`../business/redesign/03-device-binding.md`](../business/redesign/03-device-binding.md)
  — the original analysis and the three-part fix this implements §3.2 A/B of.
- [`../business/redesign/implementation/05-not-yet-true.md`](../business/redesign/implementation/05-not-yet-true.md)
  §5.2 — where the fingerprint fix was explicitly deferred, and why.
- [`API.md`](API.md) — the endpoint reference.
- [`FIXES.md`](FIXES.md) — the dated defect log entry for the missing contract
  test this work also closed.
