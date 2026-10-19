# Locus logic inventory — the module map for `locus/`

```
audience:    builder
status:      design-record
authoritative-for: the module map and the behaviour each Locus module owns
verified-against: client/src-tauri/src/locus/
```

> **Design record — only partly a to-do.** `client/src-tauri/src/locus/` exists and
> carries this behaviour: `contract`, `activation`, `device`, `heartbeat`, `tier`,
> `store`, `apply`, `runtime`, and `update/`. Where this document and the code
> disagree, **the code is right** and this file is a bug — fix it in the same
> change. Companion to `ARCHITECTURE.md` (what to cut) and
> `UPDATE-ARCHITECTURE.md` (updater).
>
> Convention: `locus/…` is new Rust we own. `cmd/`, `core/`, `config/`, `feat/` are
> inherited from Verge and are touched only as noted.

---

## 0. Module map

> **As built.** The tree below is the actual layout of `src-tauri/src/locus/`. The
> original blueprint named `update/contract.rs` and `update/public.rs`, which were
> never created — the contract lives in the top-level `contract.rs`, and the public
> manifest is decoded in `update/signal.rs` (`decode_public_manifest`).

```
src-tauri/src/
├─ locus/
│  ├─ mod.rs
│  ├─ contract.rs        # every wire name (HUB_URL, tier payload, platform keys, artifact names)
│  ├─ hub.rs             # the one HTTP client: platform roots, bundled-roots fallback
│  ├─ activation.rs      # code validation + /api/activate + /api/code-lookup
│  ├─ device.rs          # device fingerprint (per-OS) — a rate-limit/support key
│  ├─ credential.rs      # the activation code mirrored to a machine-scoped store
│  ├─ heartbeat.rs       # loop, backoff, jitter, grace, outcome classification
│  ├─ expiry.rs          # subscription date parsing + connect-gate decision
│  ├─ tier.rs            # tier payload -> mihomo config (incl. the UoT outbound)
│  ├─ store.rs           # product state as additive IVerge fields
│  ├─ apply.rs           # write the tier as a profile, call Verge's pipeline
│  ├─ runtime.rs         # owns the heartbeat loop's lifecycle
│  └─ update/
│     ├─ mod.rs
│     ├─ version.rs      # comparison semantics + rejection reasons
│     ├─ signal.rs       # decode heartbeat update fields + the public manifest
│     ├─ apply.rs        # download, verify SHA-256 (fail closed), stage
│     └─ install.rs      # drive the Tauri plugin's installer
└─ cmd/locus.rs          # thin Tauri commands -> locus/*
```

> `egress.rs` (the public-IP check) existed in an earlier round and was **removed**
> — the Connection Check card it backed is gone. The line above the original
> blueprint had it; it is not in the tree today.

---

## 1. `locus/activation.rs`

**Reference:** `legacy/wails-client/internal/activation/activation.go` (~400 lines) and
`luhn.go`.

> **Two corrections to the blueprint below, from the live code — read these first.**
> This section was written from the retired Wails client, and two of its
> statements no longer describe the shipping client:
>
> 1. **`fingerprint` no longer binds anything.** It is still *sent* (a coarse
>    rate-limit/support key) and `device_fingerprint` is still *echoed* as a frozen
>    wire field, but a code is single-use (`codes.activated_at`) and tied to no
>    device. The `409` / "code used on a different fingerprint" rows in the table
>    below are **historical** — see [`../../docs/reference/DEVICE-IDENTITY.md`](../../docs/reference/DEVICE-IDENTITY.md).
> 2. **`udp_relay` is in the live contract.** `contract.rs` declares it, and
>    `tier.rs` gates the UoT outbound on `udp_relay && uot_port > 0` (below).

### Required

- `validate_code_format(code) -> Result<()>` — **client-side, no network.** Luhn
  checksum, plus a length/charset/prefix check using **hub-supplied** values.
  Never hardcode the alphabet or prefix: the old client exposes `GetCodeCharset` and
  `GetCodePrefix` and the hub owns them.
- `activate(code, fingerprint) -> ActivateResponse`
- `lookup_code(code, fingerprint) -> LookupResponse` — the pre-check that tells a user
  "ready to activate" vs "already used" before they spend a request.

### Response shape (verbatim from the old client)

```rust
struct ActivateResponse {
    code: i32,                          // hub status code, not HTTP
    message: String,
    tier: Option<String>,
    device_fingerprint: Option<String>,
    server_config: Option<ServerConfig>,
    udp_relay: Option<bool>,
}

struct ServerConfig {
    server: String,
    server_port: u16,
    password: String,
    method: String,
    // The hub emits "uot_port" (verbatim tier-config passthrough); the retired
    // client read "server_port_uot". Accept BOTH keys. See tier.rs.
    #[serde(rename = "server_port_uot", alias = "uot_port")]
    server_port_uot: Option<u16>,
}
```

### Behaviours that must be preserved

**Hub status mapping (verified in `activation.go`, `attemptActivate`).** Note the
response body carries its own `code` field, which is what is switched on — the HTTP
status is only checked separately for `429`:

> **The device-binding rows below are HISTORICAL as of 2026-10.** A code is no
> longer tied to a device: there is no "bound to another device" refusal, no
> `409`, and re-activating a redeemed code **succeeds** (it restores the
> entitlement). The rows are kept because the *status codes* are still the wire
> contract the client parses. The live model: [`../../docs/reference/DEVICE-IDENTITY.md`](../../docs/reference/DEVICE-IDENTITY.md);
> the wire facts: [`../../docs/reference/API.md`](../../docs/reference/API.md) §5b.

| Situation | Expected |
|---|---|
| Code fails Luhn | Reject locally, no request, specific message |
| Code valid, unused | `code: 200` → success |
| Code **already redeemed** (any device) | **Succeed** — re-activation restores access; message "Already activated" |
| Code malformed / unknown | `code: 400` or `404` → invalid |
| ~~Code used on a different fingerprint~~ | **HISTORICAL** — no such refusal; a code is not device-tied |
| Account suspended | `code: 403` with `"suspended"` in `message` → suspended |
| ~~This device already holds a different code~~ | **HISTORICAL** — no `409` is emitted; `codes.rebind` is a `410` tombstone |
| Code expired | `code: 410` → **expired** (not 403) |
| Rate limited | `code: 429` **or** HTTP 429 |
| Anything else | generic server error carrying the code |
| Malformed hub URL | `ValidateHubURL` — must reject before any request |
| Fingerprint too short | `ValidateFingerprint` rejects before any request |

The 403 message-substring test is how bound and suspended are distinguished. That is
fragile — it depends on the hub's wording — but it is the **existing contract with
deployed clients**, so the fork must reproduce it, and the hub must not reword the
message without a client release. Add a test that pins this.

**The 409 is deliberately NOT a 403.** 403 is already overloaded for two distinct
situations that are told apart only by wording; adding a third would make an
older client report the wrong account state, because a client that does not know
409 renders the hub's message through its generic error path. Matching on the
**status code** is what makes this one robust — the message can be reworded
freely, provided it never contains the word `suspended`. `DeviceAlreadyActivated`
is also its own outcome arm, distinct from `BoundToAnotherDevice`, because the
student's next step differs: one is "this code belongs to someone else", the
other is "this machine already has a code".

The hub normalises the fingerprint to `[a-zA-Z0-9]` before storing or comparing
it. The client sends it as-is and must **not** normalise on its own — doing it in
two places is how the two halves drift.

**Retry policy:** client-side errors (invalid, bound, suspended, expired, rate-limited)
are **not** retried; transport failures are. Port this distinction — retrying a bound
code hammers the hub and delays the error the user needs to see.

The "used code on same device succeeds" case is not an optimisation; a student who
reinstalls and pastes their code again must not be told their code is dead.

### Options

The old client has `WithTimeout` and `WithMaxRetries` client options, and activation
retries separately from heartbeat. Port both; a school network drops requests.

---

## 1b. `locus/hub.rs` — the one hub HTTP client

Every call to the hub — activation, heartbeat, the update download — goes through
`hub::client`, which returns a client **and**, when the platform store was
rejected, a diagnosis naming what failed and what was used instead.

**Why it is one module.** The three call sites each built their own
`reqwest::Client`. They shared a policy on timeout, User-Agent and connection
pooling, and that policy is harmless when it drifts — except for one setting:
which certificate roots are trusted. A bare builder trusts only the OS store,
which on Windows means a machine that has not taken its root-certificate updates
cannot verify the hub's chain (anchored at `ISRG Root X2`) and reports it as
`Expired`. See `docs/reference/FIXES.md`, 2026-10-18.

| Behaviour | Why |
|---|---|
| Platform store first | Honours operator-installed roots and corporate proxies a compiled-in list cannot know about |
| Bundled `webpki-roots` fallback | Ships with the binary, so it needs no OS update; only a **certificate** failure swaps to it |
| Transport failures are not remembered | A school network dropping one packet must not pin the process to one store for the session |
| `no_proxy()` | The hub is reached directly — routing it through the tunnel would make activation depend on the Core the entitlement is meant to start |
| Per-call `Timeout` | Activation 30 s, heartbeat 15 s, download 600 s; the difference is deliberate and named |

Guarded by `check-consistency.sh` §36, which fails if a hub-facing module builds
its own client, if the bundled fallback disappears, or if the TLS config stops
naming its crypto provider.

---

## 2. `locus/heartbeat.rs`

**Reference:** `legacy/wails-client/internal/heartbeat/heartbeat.go`.

### Constants (verified values — do not re-tune by feel)

| Constant | Value |
|---|---|
| `MIN_INTERVAL` | 5 minutes |
| `MAX_INTERVAL` | 2 hours |
| `GRACE_PERIOD` | 7 days |

### Required

- `start()` / `stop()` / `is_running()` / `stats() -> (total, successes, failures)`
- Loop: interval starts at `MIN_INTERVAL`; on failure `interval = min(MIN * 2^failures,
  MAX)`; on success reset to `MIN`. Jitter is applied (`intervalWithJitter`).
- `do_beat()` for an immediate beat outside the loop (used after activation and by a
  manual "check now").
- `failures() -> u32`
- `remaining_grace(last_heartbeat_ok) -> Duration` — if the device never succeeded,
  the full 7 days from first launch.
- Response carries `status`, `server_time`, `tier`, and the update signal
  (→ `update/signal.rs`).
- A callback/event so the UI can react to state transitions rather than polling.

### Interaction with Verge

Heartbeat is **our** loop, but it must be able to (a) notice a tier change and
re-apply config via `core/manager/config.rs`, and (b) surface the update signal to the
UI. It must **not** start before activation, and must stop cleanly on app exit —
Verge's `singleton!` pattern is a reasonable model to follow.

---

## 3. `locus/store.rs`

**Reference:** `legacy/wails-client/internal/storage/storage.go` (476 lines).

The old client stores product state in `storage.json` under the platform config dir,
with atomic writes and **3-deep rotating backups** (`storage.json.bak.{0,1,2}`) and
`RestoreFromBackup`.

### Decision required

Verge already has a config store (`config/verge.rs`, `IVerge`, plus a draft/transaction
system in `crates/clash-verge-draft`), and it is `Option<T>`-based with serde defaults.

**Recommended:** put product *identity* fields in `IVerge` (additive, no new machinery):

```rust
// added to IVerge, all Option<T> to match the existing style
pub activation_code: Option<String>,
pub tier: Option<String>,
pub device_fingerprint: Option<String>,
pub last_heartbeat_ok: Option<i64>,
pub heartbeat_failures: Option<u32>,
pub update_pending_version: Option<String>,
pub update_pending_sha256: Option<String>,
```

…and keep **only** what does not belong in a user-editable config in a small private
`locus/state.json` (heartbeat counters, crash sentinel). Rationale: one store, one
backup mechanism, one migration path; a second JSON file next to Verge's would be a
second thing to corrupt.

**Security note:** `IVerge` is a plaintext, user-writable config. A stored activation
code is a bearer credential for that tier — the old client accepted this (it must
persist to survive restarts), but be deliberate: do not log it, do not include it in
diagnostics export, and match the old client's file mode discipline (`configDirPerm =
0700`, `filePerm = 0600` — verified in `storage.go`).

---

## 4. `locus/tier.rs`

Converts an activation/heartbeat `ServerConfig` into a mihomo config, then hands it to
**Verge's existing pipeline**:

```
core/manager/config.rs:
  update_config_forced() -> validate_and_apply() -> apply_config()
                                                 | apply_config_by_service()
                                                 -> reload_or_restart()
```

Do not write a second apply path. This brings config validation, the privileged
staging decision, and reload-vs-restart policy for free.

### The `uot_port` naming trap (FIXES #29)

- Wire key the **hub emits** (and must keep emitting): **`uot_port`**
- Key the **retired Wails client** read: **`server_port_uot`**

The hub passes the tier's config JSON through **VERBATIM**
(`heartbeat.pb.js`, `activation.pb.js`), so the endpoint reaches the client under
its stored key `uot_port` — the hub does **not** rename anything on the way out.
The client is the side that tolerates both spellings: `contract.rs` declares
`#[serde(rename = "server_port_uot", alias = "uot_port")]`.

Why `uot_port` and not `server_port_uot`: the retired client declared only
`server_port_uot` while the hub emitted `uot_port`, and an unknown-to-the-client
key is a **silent no-op** rather than an error — UDP-over-TCP was dead fleet-wide
until 2026-09-19. Deployed builds cannot be updated retroactively, so the hub
must keep emitting `uot_port`; the client accepts both. (This is the same hazard
class as `download_* → update_*`.)

### Tier → transport mapping

| Tier | Transport | Notes |
|---|---|---|
| free | Shadowsocks TCP | the free plan (port 8443). The `eco` seed row was **removed 2026-10-06**: the live hub held no code carrying it, and a duplicate row on one port renders in the console as a second plan. The on-box names (`shadowsocks-eco`, `tc-eco-cap`, `ECO_PASS`) are infrastructure, not a tier. See `docs/business/04-tiers.md` §4.1 and §4.5 |
| strike | + UDP over TCP (UoT) | the paid plan (port 8445), customer-facing name **Full**. Gated on `udp_relay` **and** `uot_port > 0` |

> **`stealth` is deliberately absent from this table, not listed as retired.**
> Instructions for a dead tier are how its PSK gets re-committed; nothing in the
> client or the hub resolves `stealth`.

The GC rule for `udp_relay`: it is only meaningful when `uot_port > 0`; an operator
toggling one without the other is editing a half-configured tier (noted in
`admin_console.pb.js`). The client should treat `udp_relay && uot_port > 0` as the
single condition to enable UoT, not `udp_relay` alone.

**Where the flag is read (fixed 2026-09-27).** `locus_connect` passed a literal
`false` to `apply_tier`, so no tier ever built a UoT profile regardless of what the
hub sent. It now reads `store::udp_relay()`, which returns the value activation
stored, defaulting to `false` for devices activated before the flag existed. If you
add a connect path, take the flag from `store` — do not pass a literal.

---

## 5. `locus/device.rs`

**Reference:** `legacy/wails-client/internal/activation/fingerprint_{linux,darwin,windows}.go`.

Per-OS device fingerprint. Behaviours to preserve:

- Stable across app restarts and reinstall.
- Distinct per machine (a code is bound to one device).
- **Not** a value the user can trivially share, and **not** PII-leaking in diagnostics
  (the old client's diagnostics deliberately truncate it).

Windows is the priority platform; Linux is best-effort (~2% of clients).

> **Its role changed twice.** It was once the code-binding key; it is now neither
> a binding key nor an identity. It is a **rate-limit and support key** sent on
> `/api/activate`, `/api/code-lookup` and `/api/heartbeat`, and the truncated
> "device id" shown in the status command. It is derived from hardware and falls
> back to a random value, so it is stable within a run but not across installs —
> which is exactly why nothing may authorise on it. Do not reintroduce a
> dependency on it.

---

## 5a. `locus/credential.rs`

**Rewritten 2026-10.** The activation code mirrored to a machine-scoped store,
so a reinstall, an update or an unexpected shutdown does not lose it. The full
design is in
[`../../docs/reference/DEVICE-IDENTITY.md`](../../docs/reference/DEVICE-IDENTITY.md);
this is the module summary.

- It replaces `identity.rs`, the durable device identity that was removed
  (recognition never worked, and binding an entitlement to a device is heavier
  than the problem needs).
- The code is the whole credential: `verge.yaml` is the primary store,
  `credential.rs` the machine-scoped mirror, and `store::read` prefers the config
  with the mirror as fallback.
- Only `store::clear_entitlement` — a definitive hub refusal — removes the
  mirror. The grace-period path (`store::record_lapsed_grace`) keeps it, because
  a local inability to confirm the entitlement must never destroy the credential.

## 6. `locus/update/*`

Fully specified in `UPDATE-ARCHITECTURE.md`. Summary of files:

| File | Content |
|---|---|
| `contract.rs` | 4 platform ids, 4 artifact filenames (with the `macos_*`/`darwin-*` asymmetry), layer-1→layer-2 field renames |
| `version.rs` | numeric-segment comparison, zero-padding, prerelease ordering, build metadata ignored; **no string compare** (`1.9.0` vs `1.10.0`) |
| `signal.rs` | decode the heartbeat's `update_*` fields for all four platforms, legacy single-hash fallback, drop non-newer |
| `public.rs` | unauthenticated `FetchPublicRelease` fallback so expired/unbound users can still update |
| `mod.rs` | decision + install call; log every branch |

---

## 7. Verge-side changes (minimal, deliberate)

| File | Change | Why |
|---|---|---|
| `config/verge.rs` | add product fields (§3) | activation/tier persistence |
| `lib.rs` | register `cmd/locus.rs` handlers | Tauri command surface |
| `core/updater.rs` | **delete** | replaced |
| `capabilities/*.json` | drop `updater:*`, tighten `fs`/`http`/asset scopes | shipping requirement |
| `core/handle.rs`, `core/tray/*` | tray menu → Connect/Disconnect/Open/Quit | product surface |
| `cmd/profile*.rs`, `cmd/webdav.rs`, `cmd/backup.rs` | remove | profiles cut (keep the *pipeline*, not the UI) |
| `crates/clash-verge-media-unlock` | delete crate | unlock page cut |

---

## 8. Frontend to write

| Component | Purpose | Reference |
|---|---|---|
| `ActivationScreen` | code entry, Luhn feedback, error states | old client `ActivationScreen.vue` (533 lines) |
| `StatusIndicator` | disconnected/connecting/connected/degraded/grace | old client `StatusIndicator.vue` |
| `TierBadge` | tier + expiry/grace countdown | old client `TierBadge.vue` |
| `MainScreen` (rework of `home.tsx`) | connect/disconnect, status, update prompt | old client `MainScreen.vue` (786 lines) |
| `UpdatePrompt` | must not dead-end (§5 of update doc) | — |

The old client's `bridge.ts` (328 lines) is the reference for which operations the UI
actually calls; a debloated fork should need far fewer.

---

## 9. Ported test vectors

From the old client's Go tests — port these **before** the corresponding logic, so the
contract is pinned rather than the behaviour re-derived:

| Source test | Guards |
|---|---|
| `activation/luhn_test.go` | checksum correctness |
| `activation/lookup_test.go` | ready/used/invalid classification |
| `updater/version_test.go` | ordering incl. `1.9.0` vs `1.10.0`, prerelease, build metadata |
| `updater/manifest_test.go` | public manifest parsing + platform selection |
| `uotkey/contract_test.go` | the `uot_port` / `server_port_uot` rename |
| `uotkey/uotkey_test.go` | UoT key derivation |
| `storage/storage_test.go` | atomic write + backup rotation + restore |
| `updatecfg/updatecfg_test.go` | platform ids, artifact names, field renames |
| `buildinfo/buildinfo_test.go` | version/commit reporting |

---

## 10. What we do **not** write

- **Elevation / privileged TUN.** Verge's service (`clash_verge_service_ipc`, tag
  v2.7.3) plus `core/service.rs` owns this. The old client's elevation path was buggy
  three separate times (FIXES #17, #40, #41) and should not be ported in any form.
- **Process supervision.** `core/manager/lifecycle.rs`
  (`start_core`/`stop_core`/`restart_core`) replaces the old `manager` package. But the
  *behaviour* it must satisfy comes from FIXES #49–53: disconnect must not orphan the
  core, "already running" must not be a dead end, and the watchdog must not restart a
  core after a user disconnect. Those are test cases for Verge's code, not new code.
- **Config validation.** `core/validate.rs`, `core/manager/config.rs`.
- **Autostart, hotkeys, tray plumbing.** `core/autostart.rs`, `core/hotkey.rs`,
  `core/tray/*`.
