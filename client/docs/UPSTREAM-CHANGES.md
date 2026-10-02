# What we changed from Clash Verge Rev

```
audience:    builder
status:      live
authoritative-for: how the Locus client differs from upstream Clash Verge Rev
verified-against: docs/STATE.md
```

> **The client is an application built on Clash Verge Rev v2.5.5, not a fork with a
> new logo.** This file is the authoritative list of how it differs, so a reader can
> tell "ours" from "upstream's" without diffing against a tag that does not exist in
> this repository (upstream history was removed when the copy was made).
>
> **Read this before changing anything in `client/`.** Two of the differences are
> load-bearing contracts with clients already in the field; §5 lists them.

---

## 1. The shape of the change

| | Size | Notes |
|---|---|---|
| **Locus logic we wrote** | ~5,800 Rust | `src-tauri/src/locus/` + `cmd/locus.rs` |
| **Locus frontend we wrote** | ~500 TS/TSX | activation gate, connect control, service shims |
| **Locus tests** | 156 unit | inline in the modules they cover, plus 6 integration |
| Upstream code retained | the rest | `core/`, `config/`, `enhance/`, `feat/`, `cmd/` |

Upstream's machinery was **not** rewritten. Everything Verge already did well —
core lifecycle, config generation and validation, privilege elevation, the service
vs sidecar decision, reload-vs-restart policy — is kept and used as-is. The Locus
modules sit *beside* it and call into it.

That is a deliberate architectural rule, and the reason is in `../../docs/reference/FIXES.md`: the
retired client hand-rolled each of those and re-earned a bug for every one.

---

## 2. What was added (ours)

### `src-tauri/src/locus/` — the product logic

| Module | Responsibility | Why it is not upstream's |
|---|---|---|
| `contract.rs` | Every wire name in one place: the tier payload, the frozen `uot_port` key, platform ids and artifact filenames | Three languages (goja JS, Rust, Go) must agree on these and nothing enforced it before |
| `activation.rs` | Luhn-mod-N code validation, `/api/activate`, `/api/code-lookup` | Verge has no concept of a user, a code, or an entitlement |
| `device.rs` | Per-OS hardware fingerprint, cached for the process lifetime | The original code-binding key; now a rate-limit/support key only |
| `credential.rs` | The activation code mirrored to a machine-scoped store | A reinstall or an update must not lose a student's code |
| `heartbeat.rs` | The loop, backoff, jitter, 7-day grace, config refresh | Verge has no server relationship at all |
| `tier.rs` | Tier payload → mihomo YAML, including the UoT outbound | Verge generates config from *profiles*; Locus generates it from an entitlement |
| `store.rs` | Product state as additive `IVerge` fields | Deliberately reuses Verge's store rather than adding a second JSON file |
| `apply.rs` | Writes the tier into a profile slot, then calls Verge's existing pipeline | The bridge; see §4 |
| `runtime.rs` | Owns the heartbeat loop's lifecycle | One owner, because the loop must never run twice nor outlive the app |
| `update/` | Version compare, signal decode, public-manifest fallback, installer hand-off | Verge's updater is replaced, not configured — see §3 |

### `cmd/locus.rs` — the command surface

`locus_status`, `locus_validate_code`, `locus_check_code`, `locus_activate`,
`locus_connect`, `locus_disconnect`, `locus_hub_url`, `locus_update_staging_dir`.

Every one returns a **typed** result. The retired client returned strings, so the UI
could not distinguish "this code is bound to your old laptop" from "we could not
reach the hub" — and every distinct situation collapsed into one unhelpful message.

### The frontend surface

| File | What it is |
|---|---|
| `pages/activation.tsx` | First-run gate. Nothing else renders until a code is accepted |
| `pages/connection.tsx` | The product: one Connect/Disconnect, status, traffic, subscription |
| `pages/account.tsx` | Device, tier, subscription detail, diagnostics |
| `components/connection/` | The connection state machine (`use-connection.ts`, `phase.ts`) and cards |
| `pages/connect-notice.ts` | Which message the connection screen shows, and in what priority |
| `pages/_theme.tsx` | The Locus palette (`LOCUS_COLORS` / `LOCUS_LIGHT`) |
| `services/locus.ts` | Typed shims over the commands |
| `services/hub.ts` | The hub URL, in exactly one place |

> The 3.1.0 rework replaced this whole surface; see `client/docs/FRONTEND.md`.
> In particular `components/home/proxy-tun-card.tsx` no longer exists — its state
> machine moved to `components/connection/use-connection.ts`.

---

## 3. What was **replaced** (upstream behaviour deliberately removed)

### The updater

**Verge's decision layer is deleted**, not reconfigured:

- `core/updater.rs` (503 lines, `SilentUpdater`) — gone
- the `tauri-plugin-updater` **plugin builder** in `lib.rs` — gone (it is never
  initialised with Tauri)
- the **WebView-facing** updater permissions — gone: `updater:default`,
  `updater:allow-check`, `updater:allow-download-and-install` (and a duplicate
  `updater:default`). Nothing in the frontend ever imported
  `@tauri-apps/plugin-updater`, and capabilities gate the webview, not Rust — so
  those granted the UI the ability to drive an updater it does not call
- the `@tauri-apps/plugin-updater` **JS package** — removed from `package.json`
- `services/update.ts`, `hooks/use-update.ts`, `update-viewer.tsx`, the settings
  button, the sidebar update button, the home "last checked" row — gone

**What remains of the plugin, deliberately:**

- the `tauri-plugin-updater` **Rust dependency** — `locus/update/install.rs` calls
  `Update::install()`, which is the installer we keep (see below)
- the `plugins.updater.pubkey` in `tauri.conf.json` — required for the plugin's
  mandatory minisign verification

**Why:** Verge's updater points at a static signed manifest on GitHub Releases and
has no concept of entitlement, rollout, or a per-device release. Locus updates are
hub-mediated: the heartbeat carries the signal, `/api/update` serves the manifest,
and the hub decides what this device is offered.

**What is kept from the plugin:** its *installer*. `Update::install()` still runs —
we hand it our own URL, hash and signature via `UpdaterBuilder::endpoints(..)`, so
NSIS/macOS/Linux installation and the mandatory minisign verification are unchanged.
Only the decision layer is ours.

> `tauri.conf.json` deliberately leaves `plugins.updater.endpoints` **empty**. Only
> `pubkey` is set. `EmptyEndpoints` is raised solely by `check()`, which we never
> call, so a stale static manifest can never be consulted.

### The debloat

Removed because Locus has no such product surface:

| Removed | Why |
|---|---|
| `pages/profiles.tsx` (999 lines) + `components/profile/` | Subscription management. Students activate a code; they do not manage subscriptions |
| `pages/unlock.tsx` + `crates/clash-verge-media-unlock/` | Streaming-service unlock checking. Nothing to do with a VPN |
| `pages/connections.tsx` + `components/connection/` | Live connection table — a tool for *choosing* between nodes, which we do not have |
| `pages/rules.tsx` + `components/rule/` | Rule-set browser. The rules still run; there is no reason to display them |
| 7 profile commands (`create_profile`, `import_profile`, `reorder_profile`, …) | Their only callers were the pages above |
| Deep links entirely (plugin, dependency, capability, scheme registration, HTTP route) | They existed solely to import subscription URLs |

### **What was deliberately KEPT, and must stay**

This is the part most likely to be got wrong by someone "finishing the job":

| Kept | Why it is load-bearing |
|---|---|
| `config/profiles.rs`, `prfitem.rs`, `enhance/` | **The profile pipeline is how a tier config becomes a running mihomo config** — `enhance()` → `collect_profile_items()` → `current_mapping()`. Deleting it leaves no path from activation to a tunnel |
| `core/manager/*` | Config validation, service-vs-sidecar staging, reload-vs-restart, `start_core`/`stop_core` |
| `cmd/profile.rs` (`get_profiles`, `patch_profile`, `patch_profiles_config`, …) | The commands that write the tier config |
| `core/backup.rs`, DNS handling, `validate.rs` | Live, unrelated to the removed pages |
| `pages/logs.tsx` | Kept and re-branded: it is the in-app support surface |
| `use-connection-data.ts` (summary only) | The home page's "active connections" metric |
| `PROFILE_SELECTIONS_PENDING_COMMIT`, `restore_selected_nodes`, `AutoBackupTrigger::Scheduled` | Used by the core lifecycle, even though their siblings were dead |

**The rule:** the debloat removed *pages and their exclusive consumers*. Where
shared machinery sat underneath — the app-data provider, the profiles engine, the
connection-summary hook — the page went and the machinery stayed.

---

## 4. How a tier becomes a running tunnel

This is the piece worth understanding, because it is where the two codebases meet:

```
locus::apply::apply_tier(config, udp_relay)
   │
   ├─ tier::build_profile()          → a mihomo document (ours)
   │
   ├─ write app_profiles_dir()/locus.yaml
   │     └─ a LOCAL profile, named "Locus"
   │        Re-applying REPLACES it. A refresh happens every heartbeat, and a new
   │        file each time would grow the config directory indefinitely.
   │
   └─ CoreManager::update_config_forced()      ← VERGE'S pipeline, unmodified
         └─ validate → apply → reload_or_restart
```

The tier config becomes a *profile* because that is the slot Verge's pipeline reads.
There is no second apply path: validation, the service-vs-sidecar decision and
reload-vs-restart policy all come from Verge, and the retired client's bugs in each
of those are not re-earned.

**Do not write a parallel apply path.** If something about applying is wrong, it is
wrong in `core/manager/config.rs` and should be fixed there.

---

## 5. Load-bearing details that look like mistakes

An agent tidying these up will break something in the field.

### The proxy group name must differ from the proxy name

```rust
pub const PROXY_NAME:     &str = "Locus";
pub const GROUP_NAME:     &str = "Locus Auto";   // NOT "Locus"
```

mihomo reads a group whose name matches a member as a **reference loop** and refuses
the **entire configuration**:

```
loop is detected in ProxyGroup, please check following ProxyGroups: [Locus]
```

The document is valid YAML and every unit test passed; only running the real engine
(`verge-mihomo -t`) caught it. It surfaced on a device as a tunnel that never starts.

### The frozen wire names

| Frozen | Why |
|---|---|
| `uot_port` (hub) ↔ `server_port_uot` (client) | The hub passes the tier config through verbatim; deployed clients read `uot_port`. Tolerance lives on the client side |
| `download_<platform>` (record) → `update_<platform>` (heartbeat) | Historical, and now load-bearing: deployed clients read `update_linux` |
| platform key `macos_intel` ↔ filename `locus-darwin-amd64` | CI produces `darwin-*`; the hub resolves by that exact name |

### The `204` on `/api/update`

`204 No Content` means "nothing to install". The Tauri plugin treats it as a clean
no-update, whereas a `200` whose body it cannot parse is raised as an **error it
logs**. Returning `200 {}` would make every up-to-date client log a failure on every
check.

### The fingerprint is not an identity, and codes are not bound

Rewritten 2026-10. The activation code used to be bound to a hardware
fingerprint in an attempt to stop one code working on two machines, and a
durable "device identity" was added so a reinstalling student could skip the
code prompt. Neither worked: the identity was never written on the activation
path, and a fingerprint re-derived on every launch told a student their own code
belonged to another device.

**Both are gone.** A code is single-use and not tied to anything —
`codes.activated_at` is the whole record of redemption, and re-activating a code
restores access on a new machine or after a reinstall. The code is kept durably
by `locus/credential.rs`, in a machine-scoped store, so a reinstall does not lose
it.

**Do not put authorisation back on the fingerprint.** It is still computed and
still sent, but only as a rate-limit and support key. Full design:
[`../../docs/reference/DEVICE-IDENTITY.md`](../../docs/reference/DEVICE-IDENTITY.md).

---

## 6. Identity changes

| | Upstream | Locus |
|---|---|---|
| App id | `io.github.clash-verge-rev.clash-verge-rev` | `com.locus.client` |
| Data dir | upstream's | migrated on first launch (see below) |
| Product name | Clash Verge | Locus |
| Version | 2.5.5 (inherited) | its own 3.x line, not Verge's — current version in `../../docs/STATE.md` |

**The data-dir migration is real user state**, not a rename: `APP_ID` is the app data
root, and on Windows it is also how the privileged service authenticates (the root's
owner SID plus a token file inside it). `utils::dirs::migrate_legacy_app_data_dir()`
renames the old directory on first launch, and the rule is deliberately conservative:
only when the new root does not exist, rename (never copy), and report failure
non-fatally. Full detail in `IDENTITY-MIGRATION.md`.

**Version drift is guarded by a test.** `tests/version_consistency.rs` reads
`Cargo.toml`, `package.json` and `tauri.conf.json` from disk and asserts they agree.
A build whose reported version disagrees with its bundled one either refuses the
update that would fix it or silently declines every release — and neither says so.

---

## 7. What is still upstream, on purpose

Not everything un-branded is an oversight. These are **not** load-bearing, but they
are also not worth the churn:

- Rust crate names (`clash_verge_i18n`, `clash_verge_logging`, …) and the `crates/`
  workspace members.
- Sidecar filenames (`verge-mihomo`, `verge-mihomo-alpha`) and the mihomo IPC socket.
- The tray id `clash-verge-rev-tray`.
- The **Windows service name** — it comes from the pinned `clash-verge-service-ipc`
  crate, which also ships the service binary. Renaming it means forking that crate
  and owning a privileged Windows service end to end. It appears in `services.msc`,
  not in the product surface.
- Runtime/check config filenames (`clash-verge.yaml`, `clash-verge-check.yaml`).

---

## 8. How to tell ours from upstream's

```sh
# Everything we own lives here:
ls client/src-tauri/src/locus/
ls client/src/services/locus.ts client/src/services/hub.ts
ls client/src/pages/{activation,connection,account}.tsx
ls client/src/components/connection/
ls client/src/pages/connect-notice.ts client/src/pages/_theme.tsx

# Anything in these is upstream, modified only where a comment says so:
client/src-tauri/src/{core,config,enhance,feat,module,process}/
```

Within a modified upstream file, the change is marked by a comment explaining the
constraint — not by narration. `client/AGENTS.md` rule 2: *comments state constraints,
not what the code does*.

---

## 9. Verification beyond unit tests

A configuration can pass every test and still be refused by the engine that runs it.

```sh
cd client
cargo test --manifest-path src-tauri/Cargo.toml     # lib + integration tests
cargo clippy --manifest-path src-tauri/Cargo.toml   # must be 0 warnings

# The check that catches what tests cannot:
src-tauri/sidecar/verge-mihomo-x86_64-unknown-linux-gnu -t -f <generated>.yaml
```

The mihomo check is how the proxy-group loop was found. Run it against
`tier::build_profile` output whenever that module changes.

### The readiness probe has its own manual

`core/manager/probe.rs` and the `core/manager/mod.rs` around it are **upstream
files Locus modified**, and the modification is load-bearing: the app's
connected/connecting distinction is decided there, and a defect there reads on
screen as "stuck on connecting" with no other symptom. Two shipped defects came
from this one chain.

Before changing any probe constant or the delay classifier, read
[`../../docs/reference/EGRESS-READINESS.md`](../../docs/reference/EGRESS-READINESS.md).
It carries the full chain, a four-command localisation procedure, and §4 — the
trap that caused the recurrence: **do not derive the classifier's acceptance
window from the timeout budget, and do not make it agree with the frontend's
`classifyDelay`.** The probe asks "did a packet move?", not "how fast was it?".

---

## 10. Related

- `ARCHITECTURE.md` — the original rework plan (what to cut/keep), written pre-port
- `LOGIC-INVENTORY.md` — module-by-module spec; now largely implemented
- `FRONTEND.md` — the shipping two-tab UI, routing, and the connection/notice rules
- `UPDATE-ARCHITECTURE.md` — why the updater is hub-mediated
- `SIGNING.md` — key custody and the signing pipeline
- `IDENTITY-MIGRATION.md` — the app-id migration and what must not be renamed
- `RESTRUCTURE.md` — repo layout history
- `../../docs/reference/EGRESS-READINESS.md` — the readiness chain, and why the
  button says "connecting"
- `../../docs/operate/UPDATE-SYSTEM.md` — the hub side, end to end
