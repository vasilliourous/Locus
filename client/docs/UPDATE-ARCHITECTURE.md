# Updater architecture: replacing the Tauri updater with the hub-mediated path

```
audience:    builder
status:      design-record
authoritative-for: why the updater is hub-mediated, and how it differs from Tauri's
verified-against: docs/STATE.md
```

> **Design record — implemented, with one correction.** The hub-mediated path is
> built (`locus/update/`). See `UPSTREAM-CHANGES.md` §3 and
> `../../docs/operate/UPDATE-SYSTEM.md` for what is live.
>
> **Correction: the Tauri updater plugin is not removed — it is used as the
> INSTALLER.** What is removed is its *decision layer*: `core/updater.rs`, the
> plugin's Tauri registration, the WebView-facing `updater:*` capabilities, and the
> whole `@tauri-apps/plugin-updater` frontend surface.
>
> `locus/update/install.rs` still calls `Update::install()`, which is what gives us
> NSIS/macOS/Linux installation and the mandatory minisign verification. We feed it
> our own URL, hash and signature via `UpdaterBuilder::endpoints(..)`.
>
> A second correction: `Update` **cannot be hand-constructed** — `extract_path` and
> `context` are private. An earlier version of this plan assumed a struct literal
> would work; it does not compile. The route that does work is `check()` against a
> dynamic `{version, url, signature}` manifest served by the hub.
>
> This document is the design for the single biggest divergence between the fork and
> its Verge base. Read it before touching anything update-related.

---

## 1. How Clash Verge Rev updates (what we are removing)

Verified in `src-tauri/src/core/updater.rs` (503 lines) and `tauri.conf.json`.

**Mechanism:** the Tauri updater plugin (`tauri-plugin-updater` 2.11.0) against a
static JSON manifest on GitHub Releases, fetched through one of three proxy endpoints:

```
https://update.hwdns.net/https://github.com/clash-verge-rev/clash-verge-rev/releases/download/updater/update-proxy.json
https://gh-proxy.org/https://github.com/clash-verge-rev/clash-verge-rev/releases/download/updater/update-proxy.json
https://github.com/clash-verge-rev/clash-verge-rev/releases/download/updater/update.json
```

signed with a minisign pubkey embedded in `tauri.conf.json`.

**Flow** (`SilentUpdater`, a `singleton!`):

1. `start_background_check` — periodic check, downloads in the background.
2. Bytes are written to a **cache dir**: `app_home_dir()/update_cache/pending_update.bin`
   plus `pending_update.json` (`{version, downloaded_at}`).
3. `try_install_on_startup` — on next launch, if cached version > current, ask the
   user via a native dialog, then `update.install(&bytes)` in a `spawn_blocking` with
   a **30-second timeout** (comment: "`install()` may hang (#2558)").
4. Windows NSIS gets `/LANG=<id>` to suppress the language dialog; `nsis_language_id`
   maps only `zh`/`zhtw`/`ru`, defaulting to English.
5. Version comparison is home-grown: `version_lte` (numeric components, strips `-`/`+`
   suffixes) and `is_build_to_stable` (detects a `+build` prerelease promoting to stable).

**Characteristics that matter:**

- **No user concept.** Anyone can update anything; there is no entitlement.
- **No rollout control.** The manifest is the same for every client on earth.
- **No per-platform artifact selection in the app** — the manifest carries it.
- **Trust = the signature**, verified by the plugin against the embedded pubkey.
- **`createUpdaterArtifacts: true`** made the Tauri build emit `.sig` files.

### What we already changed

`client/src-tauri/tauri.conf.json` no longer contains the `plugins.updater` block
(no `pubkey`, no `endpoints`) and now sets `createUpdaterArtifacts: false`.

**This is the right direction but it is not sufficient.** With the plugin present but
unconfigured, any residual call path fails at runtime rather than at compile time. The
plan below removes the code, not just the config.

---

## 2. How Locus updates (what we are porting)

The old client's path, `legacy/wails-client/internal/updatecfg` + `internal/updater`.
It is not a "simpler" updater — it is a **hub-mediated, entitlement-aware, per-platform,
rollout-controlled** system, and it is already the contract deployed clients speak.

### 2.1 The three-layer contract

`updatecfg`'s package doc is explicit that this distinction is load-bearing. Reproduced:

| Layer | Written by | Field names |
|---|---|---|
| 1. `update_config` record | publish scripts | `download_<platform>`, `sha256_<platform>` |
| 2. heartbeat response | `heartbeat.pb.js` hook | `update_<platform>`, `update_sha256_<platform>` |
| 3. client struct | the client | reads layer 2 verbatim |

**The rename between layers 1 and 2 is historical and now load-bearing.** Deployed
clients read `update_<platform>`. The hook must keep translating. The doc says, in as
many words: *"Do not 'simplify' it by making the hook emit `download_<platform>`
directly."*

**Why this exists at all:** two independent publish scripts (`publish-release.sh` and
`publish-update.sh`) disagreed on the field names, and *neither was internally wrong*.
There was simply no shared definition. A record written by `publish-update.sh` produced
a heartbeat with every per-platform URL missing; `PlatformDownloadURL()` then fell
through to the legacy single `update_url`, which both scripts point at the **Linux**
binary — so Windows and macOS clients would download a Linux executable and fail the
checksum. That is FIXES #31.

**Fork action:** port `updatecfg` to Rust as a single source of truth for platform
identifiers, artifact filenames, and field names. Do not re-derive them.

```rust
// locus/update/contract.rs
pub const PLATFORMS: [&str; 4] = ["linux", "windows", "macos_intel", "macos_arm"];
// layer 1 (record)  -> layer 2 (heartbeat) names
// download_linux    -> update_linux   ...
// sha256_linux      -> update_sha256_linux ...
```

### 2.2 Platforms and the artifact-name contract

`updatecfg.Artifacts` pins **a second contract** — between CI's staging step,
`publish-release.sh`, and the Caddy `/updates/<version>/` layout:

| Platform identifier | Artifact filename |
|---|---|
| `linux` | `locus-linux-amd64` |
| `windows` | `installer-Locus_<version>_x64-setup.exe` |
| `macos_intel` | `locus-darwin-amd64` |
| `macos_arm` | `locus-darwin-arm64` |

**Note the deliberate asymmetry:** the *platform key* says `macos_*`, the *filename*
says `darwin-*`. A fork that "tidies" this to `macos-*` breaks the publish pipeline,
because CI produces `darwin-*` and the release job asserts against these names.
(FIXES #47: a 1 MB floor also silently rejected a valid release; FIXES #25:
`download-artifact` does not flatten, and the release job assumed it did.)

**Windows is an installer, not a raw executable.** A Windows client does not run
the payload it downloads — it hands the bytes to `tauri_plugin_updater`, which
ShellExecutes them. `locus-windows-amd64.exe` is a PE the *retired portable
client* consumed; serving it for self-install made an installed client relaunch a
copy of itself outside its install directory. The Windows slot is therefore the
NSIS setup executable, and it carries its **own** minisign signature (the raw
binary's signature cannot stand in — minisign signs exact bytes). The other three
platforms still use raw payloads: they reach a package manager or a `.app` rather
than a ShellExecuted PE.

The name is version-bearing because Tauri names its NSIS bundle with the version.
It is produced in three places that must agree — CI's `manifest.json`,
`fetch-release.py`'s `installer_name()`, and `publish-release.sh`'s
`WINDOWS_INSTALLER` — and `check-consistency.sh` §1a pins it.

> **This drifted once and broke every Windows update.** CI corrected the manifest
> for v3.2.12; the hub's fetcher did not follow, staged the raw binary into the
> Windows slot, and served it with the raw binary's signature. Every Windows
> client refused it — correctly — so no Windows client could update at all, and
the failure only surfaced when a student tried. See `docs/reference/FIXES.md`.

### 2.3 Where the update signal comes from

Updates are advertised **through the heartbeat**, not by a separate poll:

```
heartbeat → { update_available, update_url, update_sha256,
              update_linux, update_windows, update_macos_intel, update_macos_arm,
              update_sha256_linux, ..., update_sha256_macos_arm }
```

The hub gates this on `update_config.active` alone — **there is no rollout
percentage** (removed 2026-09; the `rollout_percent` column remains in the schema
but nothing reads it). The client drops any signal that is not **strictly newer**,
and the installer path refuses a downgrade regardless — because a stale or
rolled-back `update_config` row would otherwise downgrade every client, and there
is **no server-driven downgrade path** (recovery would mean shipping a new
version). FIXES #12.

### 2.4 The code deadlock (FIXES #42) — and the public-manifest fallback

The old client had a genuine deadlock: **updates were only reachable through an
activation code**, so a user whose code had expired or whose device was unbound could
not update to the version that might fix their problem.

The resolution, ported into `locus/update/signal.rs` as `decode_public_manifest`, is a
**public fallback**: an unauthenticated release manifest fetched from the hub
(`GET /api/release`, implemented by `server/pb_hooks/release.pb.js`), used when the
heartbeat path cannot serve an update:

```rust
// shape of the public manifest
{ ok: bool, published: bool, version: String,
  platforms: { <platform>: { url: String, sha256: String } } }
```

**This must survive the port.** It is the difference between "expired user is stuck
forever" and "expired user can still get fixes".

### 2.5 The apply path

From `internal/updater/updater.go`:

1. `PerformUpdate(ctx, info)` — resolve the hash for the platform *actually being
   fetched* (`PlatformSHA256()` falls back to the legacy single `update_sha256`, which
   is exactly the bug in §2.1 — keep the fallback for old hub rows, but the fork's own
   publishes always write per-platform).
2. Download to a **staging dir** (`SetStagingDir`, `ensureStagingDir`) — not CWD
   (FIXES #54: the updater staged into whatever directory the app was launched from).
3. `verifyChecksum` — SHA-256, reject on mismatch.
4. `createBackup` → `swapBinary` → `forkNewProcess`, with `restoreBackup` on failure
   (FIXES #22: swap failures could destroy the installation).
5. `CheckOnStartup(revertFlag)` + `performRevert` — crash-on-update auto-revert via a
   sentinel.
6. `HandoffFlag` (`--handoff`) is a **shared constant** between parent and child so the
   two cannot drift on the spelling; the successor waits 1200 ms, inside the parent's
   1500 ms quit delay, to avoid a double window.

---

## 3. Why the apply path does *not* port verbatim

Steps 4–6 above are **self-replacing a portable binary**. The fork is an **installed
application**: NSIS on Windows, `.app`/`.dmg` on macOS, deb/rpm/AppImage on Linux.

Verge's `tauri-plugin-updater` already solves the "install the platform's package"
problem — that is precisely what it is for. So the fork should:

- **Keep** the contract, the entitlement model, the per-platform selection, the
  rollout gating, the no-downgrade rule, and the public-manifest fallback.
- **Keep** Verge's *install mechanics* (the plugin's `update.install()`), driven by
  our own metadata rather than by a signed static manifest.
- **Delete** the hand-rolled backup/swap/revert/fork machinery. Those exist because a
  portable `locus.exe` had to replace itself; an NSIS install does not, and every one
  of them was a source of field bugs.

**The hybrid (as built):** use `tauri-plugin-updater` as a downloader/installer, but
feed it the URL and hash that *our* heartbeat or public manifest chose. The plugin
verifies a **minisign signature mandatorily** and has no bypass, so the hub signs every
artifact with `LOCUS_UPDATE_KEY`; our own SHA-256 check runs on top. Both checks are
kept — SHA-256 catches corruption, the signature catches substitution, and neither is
redundant. See `client/docs/SIGNING.md`.

### 3.1 Open question: how to drive the plugin from our metadata

Three options, in preference order:

1. **Build-time updater config injection** — generate a per-release static manifest on
   the hub, signed, with our endpoints. Keeps the plugin's model intact (signature
   verification, installer args) but reintroduces a static file and loses the rollout
   gate unless the manifest URL itself is rollout-gated. **Problem:** a static manifest
   is the same for every client, which is the thing Locus explicitly does not want.
2. **Runtime metadata override** — keep the entitlement/rollout logic in `locus/update/`,
   and hand the chosen `(version, url, sha256)` to a lower-level install call. Loses
   the plugin's signature check but keeps our SHA-256 check plus the hub's TLS.
   **This is the recommended path**, and it is what the old client effectively did.
3. **Keep the plugin fully configured against a hub-hosted signed manifest** and drop
   the heartbeat-carried signal for updates. Simplest code, but throws away rollout
   control and the code deadlock fix. Not recommended.

---

## 4. Consequences for other subsystems

| Subsystem | Consequence |
|---|---|
| `core/updater.rs` | **Delete.** All 503 lines. Do not leave it reachable. |
| Silent/cached update + startup dialog | Reconsider: the fork's install is an installer, and the old client's value was in *notification*, not silent pre-download. Simpler: notify, then install on user action. |
| `pending_update.bin` cache | Not needed if we do not pre-download. If we do, reuse the pattern but put it under the app dir, never CWD. |
| Crash-on-update sentinel | Replaced by the installer's own atomicity, **but** keep the "did we come back?" check for the *hosted* case: if the new version fails to start, the user needs a way out. |
| `createUpdaterArtifacts` | Stays `false` unless option 1 is chosen. |
| Version authority | **Guarded.** The fork's three sites (`package.json`, `Cargo.toml`, `tauri.conf.json`) must agree, asserted by `tests/version_consistency.rs`; current value in `../../docs/STATE.md`. Version drift is a **correctness dependency of the updater**, not hygiene (FIXES #20/#45, #12). See `docs/reference/STILL-OPEN.md` ("Fork versioning — RESOLVED"). |
| nsproxy/language | `nsis_language_id` logic is small and worth keeping for silent installs; it is Windows-only and currently maps 3 languages. |

---

## 5. Implementation checklist

- [ ] Port `updatecfg` constants + artifact filename contract to
      `src-tauri/src/locus/update/contract.rs`, with the `macos_*` / `darwin-*`
      asymmetry preserved and commented.
- [ ] Port version comparison (`CompareVersions`/`IsNewer` semantics — numeric
      segments, zero-padding, prerelease ordering, build metadata ignored) and unit-test
      it against the old client's Go test vectors, including `1.9.0` vs `1.10.0`.
- [ ] Port the heartbeat update-signal decoder, accepting all four platforms and the
      legacy single-hash fallback.
- [ ] Port the public-manifest fallback (`FetchPublicRelease`) so unactivated/expired
      users are not stranded.
- [ ] Delete `core/updater.rs`; remove the `tauri-plugin-updater` dependency and its
      capability entries (`updater:default`, `updater:allow-check`,
      `updater:allow-download-and-install`) from `capabilities/desktop.json`.
- [ ] Implement the chosen install call (§3.1 option 2) with our own SHA-256
      verification, and **fail closed** on a missing hash (the old client refuses an
      empty hash — keep that).
- [ ] Wire a UI prompt that cannot dead-end: if the hub is unreachable *and* the public
      manifest is unreachable, tell the user what to do rather than showing a spinner.
- [ ] Port the parity tests: artifact names, field renames, hash resolution per
      platform, no-downgrade, grace arithmetic.
- [ ] Add an update-decision **log line** at every branch (signal dropped as not-newer,
      rollout excluded, hash missing, manifest fallback used). FIXES #10's real damage
      was a silent no-op; the client-side defence is observability.

---

## 6. What is deliberately *not* ported

| Old mechanism | Why not |
|---|---|
| `swapBinary` / `copyFile` / `restoreBackup` | The installer owns installation; hand-rolled swaps destroyed installs once (FIXES #22) |
| `forkNewProcess` + `--handoff` + 1200 ms sleep | Same reason: only needed when a running binary replaces itself |
| `performRevert` sentinel | Installer atomicity replaces it, except for the "did it start" check |
| `SetStagingDir` under CWD | Replaced by the installer's own staging |
| `publish-update.sh` (root) | Documented broken: wrote no `sha256_<platform>` columns, and its macOS URLs (`locus-macos-*`) do not match CI's filenames (`locus-darwin-*`). `publish-release.sh` is the correct path |
