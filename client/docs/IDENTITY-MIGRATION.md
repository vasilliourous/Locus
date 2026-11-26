# Client identity migration — Chrysalis

```
audience:    builder
status:      live
authoritative-for: the app-id / service-name migration and what must NOT be renamed
verified-against: client/src-tauri/tauri.conf.json, client/src-tauri/src/chrysalis/
```

> **Implemented for the app-data root; the privileged service name is deliberately NOT renamed.**
> Created 2026-09-24 while re-identifying the fork from upstream Clash Verge Rev.
> Read this before "tidying up" any `clash-verge` string in `client/`.

---

## 1. Why this file exists

The fork was copied from Clash Verge Rev v2.5.5 with upstream git history removed.
Only the *branding* was changed at copy time (product name, icon, app id in
`tauri.conf.json`). The **code** still carried upstream's identity in places that
are load-bearing rather than cosmetic, and one of them — `APP_ID` — is the
application data root.

This file records what was changed, what was deliberately left, and why, so the
next person does not re-open the same questions or "fix" an asymmetry that is
holding something up.

---

## 2. `APP_ID` — the app-data root (CHANGED)

`client/src-tauri/src/utils/dirs.rs`:

| | Upstream | 2026-11-25 (current) | Adopted from |
|---|---|---|---|
| release | `io.github.clash-verge-rev.clash-verge-rev` | `com.chrysalis.client` | ← `com.locus.client` ← upstream |
| dev (`verge-dev`) | `io.github.clash-verge-rev.clash-verge-rev.dev` | `com.chrysalis.client.dev` | ← `com.locus.client.dev` ← `com.locus.client` ← upstream |
| `BACKUP_DIR` | `clash-verge-rev-backup[-dev]` | `locus-backup[-dev]` — **NOT moved, see below** | — |

`APP_ID` moved on 2026-11-25, and `LEGACY_APP_IDS` now lists **both** previous
roots, newest first, so one launch adopts either. `BACKUP_DIR` did **not** move
and is not the same kind of name: it is the collection name in **WebDAV paths**,
so it names a directory on a server this product does not control. Renaming it
would not migrate anything — it would point at an empty collection and make every
already-uploaded backup invisible. See
[`../../docs/reference/NAME-MIGRATION.md`](../../docs/reference/NAME-MIGRATION.md) §3.2.

**Why this is not a cosmetic rename.** `app_home_dir()` joins this onto the
platform data directory, and **43 call sites** resolve through it:

- `verge.yaml` (all app settings), `config.yaml`, `profiles.yaml`, `profiles/`,
  `icons/`, `logs/`, `locus-backup/`
- the product state added by this remodelling — activation code, tier, device
  fingerprint, heartbeat counters, pending-update records
- `.encryption_key` (encrypts the WebDAV credentials in `verge.yaml`)
- on Windows: **the owner token file the privileged Service authenticates
  against**, plus the instance lock/record files

So renaming it is a **user-visible state migration**, not a string change.

### 2.1 The one-time adoption (`migrate_legacy_app_data_dir`)

`utils::dirs::migrate_legacy_app_data_dir()` runs from `run()` in `lib.rs`,
**before** the Windows owner repair, the singleton check, and the logger — all of
which resolve paths through the data root, so any of them running first would
operate on the new root before the old contents arrived.

Rules, deliberately conservative because the failure mode is a half-copied config
directory:

- **Only runs when the current root does not exist.** An existing Chrysalis root is
  never touched, so this is safe to call on every launch and is a no-op after the
  first successful run.
- **Renames, never copies.** `fs::rename` is atomic within a filesystem, so a
  crash cannot leave two partially-populated roots. A cross-filesystem rename
  *fails*, and that failure is reported — it is not degraded into a recursive
  copy, because a truncated profile set that the app then rewrites is worse than
  no migration at all.
- **A failure is non-fatal for the LAUNCH, and fatal for the USER.** The app
  starts with an empty data root; the legacy directory is left intact for a
  manual move, and the adoption's `Err` is surfaced as its own install stage
  (`InstallStage::AdoptionFailed`) rather than only logged, because on this
  deployment "re-activate" means "the student's paid code is gone" — they
  bought it somewhere else and the client cannot recover it. Refusing to launch
  is still worse (a device that will not start cannot report its state), so the
  launch proceeds and the state is made visible on the Account page.
- **The orphan case has its own stage** (`InstallStage::AdoptedAside`). When the
  update migration (see `UPDATE-INSTALL-MIGRATION.md`) has to move a legacy root
  aside because the new root already exists, the data is safe but the *code* the
  device is running on may be in the moved-aside root — so the app says so
  instead of silently starting on a root the student did not expect.
- **Candidates are listed newest-first** in `LEGACY_APP_IDS`, and the first that
  exists wins. Only one legacy root is expected per build flavour; the list
  exists so a future identity change does not require restructuring the call.

`LEGACY_APP_IDS` holds **historical literals** and must not be replaced with the
current `APP_ID` — the migration would then have nothing to look for.

### 2.2 Windows: what the adoption does and does not fix

On Windows the Service authenticates by comparing the **owner SID of the app data
root** against the caller's, and by an owner token file *inside* the root
(`core/owner_identity.rs`, `repair_app_data_root_owner`). Consequences:

- The adopted directory carries the token with it, and the directory owner is
  unchanged, so the migration keeps the Service working.
- If ownership is wrong, `repair_app_data_root_owner` already repairs it, and it
  now runs **after** the adoption rather than before.
- If the Service was installed against the OLD root, the transport health check
  reports `ReinstallRequired` / `ForceReinstallRequired` and the app asks for a
  reinstall. That is the existing self-heal path (`core/service.rs`), not
  something this migration has to handle specially.

### 2.3 Migration checklist for a real install

1. First launch under `com.locus.client` adopts the legacy directory; confirm the
   log/stderr line `[chrysalis] adopted pre-Chrysalis application data from …`.
2. Confirm the activation code / tier survived (the client should not ask to
   activate again).
3. On Windows with the Service installed, confirm core start works; if it asks to
   reinstall the Service, that is expected and is a one-click fix.
4. Confirm `.encryption_key` moved with the directory — otherwise stored WebDAV
   credentials will fail to decrypt.

---

## 3. What was deliberately NOT renamed

### 3.1 The privileged service name (upstream, and load-bearing)

`core/service.rs` opens the service via
`clash_verge_service_ipc::WINDOWS_SERVICE_NAME`. That constant belongs to the
**pinned upstream crate** `clash-verge-service-ipc` (tag `v2.7.3`), which also
ships the service binary itself (`clash-verge-service`, `-install`, `-uninstall`,
fetched by `scripts/prebuild.mjs`).

Renaming it means forking that crate and its binaries, i.e. owning a privileged
Windows service end to end. That is a **separate decision with its own risk
profile**, not part of a branding pass. Until it is taken, the service keeps its
upstream name and the app keeps its upstream identity to the service.

User-visible effect: **none**. The service name appears in `services.msc` and in
the app's own diagnostic strings, not in the product surface.

### 3.2 Rust crate names, identifiers, and paths

`clash_verge_i18n`, `clash_verge_logging`, `clash_verge_draft`,
`clash_verge_signal`, `clash_verge_limiter`, `clash_verge_service_ipc`,
`tauri-plugin-clash-verge-sysinfo`, the `crates/` workspace members, sidecar
filenames (`verge-mihomo`, `verge-mihomo-alpha`), the mihomo IPC socket
(`verge-mihomo.sock`), the tray id (`clash-verge-rev-tray`), temp-file prefixes,
and the runtime/check config filenames (`clash-verge.yaml`, `clash-verge-check.yaml`).

These are internal identifiers. Renaming them is churn with no product benefit and
several failure modes (see §3.1 for the worst one). The rule from
`client/AGENTS.md` applies: **minimal diffs; do not rename inherited identifiers
unless the change requires it.**

---

## 4. Still upstream, and should be fixed on sight

These are user-visible and are *not* load-bearing:

| Where | What | Note |
|---|---|---|
| `src-tauri/tauri.conf.json` | deep-link scheme `locus` | already correct |
| `src-tauri/src/utils/init.rs` | `DEEP_LINK_SCHEMES = ["clash", "clash-verge"]`, `clash-verge.desktop` | Linux deep-link registration |
| `src-tauri/src/utils/resolve/scheme.rs` | accepts `clash` / `clash-verge` schemes | must match §above |
| `src-tauri/src/utils/network.rs` | `clash-verge/v{version}` User-Agent | |
| `src-tauri/src/utils/schtasks.rs` | `clash-verge-task-{user,admin}.xml` | Windows scheduled tasks |
| `src-tauri/src/utils/macos_launch_guard.rs` | `Contents/MacOS/clash-verge` in install-location checks | **must match the real bundle binary name** |
| `src/pages/settings.tsx` | `t.me/clash_verge_re` Telegram link | |
| `src/components/setting/mods/update-viewer.tsx` | `https://chrysalis.app/releases/...` | placeholder domain — needs a real decision |

The deep-link scheme and the bundle binary name are a **matched pair**: changing
one without the other breaks deep links and the macOS launch guard respectively.
Check both sides before editing.

---

## 5. The product rename, in two steps

**2026-11-23 — the name moved; the identity did not.**

**What changed:** `productName` (later; see the 11-25 step), `publisher`, the crate
and npm package names, the `src-tauri/src/locus/` module tree (→ `chrysalis/`), the
15 IPC commands, the `locus_*` config fields, `CHRYSALIS_COLORS`, and every
user-facing string.

**2026-11-25 — the identity and the artefact filenames moved too.**

This is the step that makes the earlier "no data migration was needed" claim out
of date, and it is recorded here because this file owns the identity question:

| Moved | How it stayed safe |
|---|---|
| `APP_ID` → `com.chrysalis.client` | `LEGACY_APP_IDS` carries the old value and `migrate_legacy_app_data_dir` renames the root on first launch, **before** the Windows owner repair, the singleton lock and the logger — all three resolve paths through it. The ordering is pinned by a test. |
| artefact filenames → `chrysalis-*` | CI publishes **both** name sets and the hub resolves both, so a release either side of the move installs. See `NAME-MIGRATION.md` §3.1. |

**What still has not moved, and why** — each is a contract with something this
repository cannot migrate:

| Retained | Because |
|---|---|
| the minisign **key**, and `LOCUS_UPDATE_KEY` as an Actions secret name | installed clients only accept updates signed by the key compiled into them, and Actions secrets are write-only: a renamed *reference* to a not-yet-created secret yields an empty key. The workflow reads both names, so the operator can create the new one in any order. |
| `locus-backup` (`BACKUP_DIR`) | a **remote** WebDAV collection name — see §2. Renaming it orphans every uploaded backup. |
| `LOCUS_UPDATE_KEY_B64` inside the bundle | the key name lives inside committed ciphertext; changing it needs a re-encrypt, which both scripts now support as an either-of. |
| `vasilliourous/Locus` (the remote) | a hosting decision; every reader takes `GITHUB_REPO`. |
| `locusvpn.jadedns.uk` (the landing host) | still the serving name; see `CLAIMS.md` A29/A30. |

**Consequence, stated so nobody re-opens it:** an install from before 2026-11-25
**keeps its data** — the old root is renamed into the new one on first launch. It
does not keep anything keyed to the identifier *string*, because nothing was: the
updater is handed a URL by the hub and verifies against the compiled-in pubkey.

## 6. Related

- `client/docs/ARCHITECTURE.md` — what to cut from upstream and where Chrysalis logic goes
- `client/docs/LOGIC-INVENTORY.md` — the modules to write
- `docs/reference/STILL-OPEN.md` — what is genuinely still open
- `docs/reference/FIXES.md` — the historical defect log
