# Repo restructure: separating client, server, and legacy

```
audience:    builder
status:      design-record
authoritative-for: the 2026-09 layout change and what it repointed
verified-against: docs/STATE.md
```

> Records the layout change and everything that must be repointed. The old layout
> tangle (`v4/`, `v5/client/`, `v5/server/`, `v5/console/`, root `scripts/`, root
> `bump.sh`) is the reason this is written down rather than done ad hoc.
>
> **Update (2026-09): the version/CI machinery was DELETED, not repointed.**
> Root `VERSION`, `bump.sh`, `server/scripts/bump-version.sh`, `stamp-syso.py`,
> `smoke-bump.sh`, `scripts/release-cut.sh`, the committed `.syso` resources, and
> `.github/workflows/build.yml` are gone. They versioned and released the retired
> Wails client, never the fork. Sections marked REMOVED below reflect that; the
> fork's release path now exists — three version sites guarded by a test, and CI
> at `.github/workflows/client.yml` (see `docs/operate/RELEASING.md`).

---

## 1. Target layout

```
VPN-Service/
├─ client/                     # the fork (Clash Verge Rev base)
│  ├─ src/  src-tauri/  crates/  scripts/
│  ├─ Cargo.toml  Cargo.lock  package.json  pnpm-lock.yaml
│  └─ docs/                    # ARCHITECTURE, UPDATE-ARCHITECTURE, LOGIC-INVENTORY
├─ server/                     # was v5/server/
│  ├─ pb_hooks/  modules/  scripts/  templates/
│  └─ console/                 # was v5/console/
├─ legacy/
│  ├─ wails-client/            # was v5/client/ — STALE, reference-only (logic oracle)
│  └─ v4/                      # was v4/ — STALE
├─ docs/                       # was v5/docs/ + extra-details/ merged
└─ scripts/                    # code generation + PDF cards + vps-test probes
```

> **As-built (2026-09).** The block above is the *actual* current layout. Two
> entries from the original target were **never created** and must not be looked
> for: `shared/contracts/` (see §6) and `vendor/`. The `packages/` directory
> inside `client/` also does not exist. CI lives at `.github/workflows/client.yml`
> at the **repo root** (the old `build.yml` built the archived client and was
> deleted), and there is **no root `VERSION` file**.

**Why `legacy/wails-client` rather than deleting it:** its Go tests encode contract
behaviour (Luhn, version comparison, the `uot_port` rename, artifact names, grace
arithmetic) that the fork must reproduce. `LOGIC-INVENTORY.md` §9 lists the specific
test files to port. Deleting it before those are ported loses the specification.

---

## 2. Steps, in order

Uses `git mv` so history follows.

1. `v5/server/` → `server/`
2. `v5/console/` → `server/console/`
3. `v5/client/` → `legacy/wails-client/`
4. `v4/` → `legacy/v4/`
5. `v5/docs/` + `extra-details/` → `docs/` (resolve name collisions explicitly;
   `extra-details/` holds `WHAT-WE-DID.md`, `STILL-OPEN.md`, `GOTCHAS-FROM-THIS-WORK.md`,
   `SESSION-UPDATE-ARC.md`, `README.md`)
6. `v5/CONTEXT.md`, `v5/README.md` → `docs/` (review first; they describe the old
   layout and will be stale)
7. `v5/VERSION` → **REMOVED** (deleted with the archived-client version tooling —
   see §3)
8. Delete the now-empty `v5/`
9. ~~Create `shared/contracts/`~~ — **NOT DONE.** No such directory exists; the
   contract is enforced by the archived `internal/updatecfg` test and the hub
   hooks, not by a shared schema dir. Do not go looking for `shared/contracts/`.
10. ~~Handle the stray tracked file at repo root literally named
    `whale resume 01a06fdc-….md`~~ — **RESOLVED.** The `.gitignore` now ignores
    `whale.txt`; no such file is tracked.

---

## 3. Version authority — REMOVED, unresolved

**There is no version authority.** The plan in this section originally said the
repo `VERSION` file would remain the single authority for the client. That plan
was **not** carried out: root `VERSION`, `bump.sh`, `server/scripts/bump-version.sh`,
`stamp-syso.py`, `smoke-bump.sh`, `scripts/release-cut.sh`, the committed
`rsrc_windows_*.syso` resources, and `.github/workflows/build.yml` were all
**deleted**, because they versioned and released the retired Wails client and
never the shipping fork.

The fork's three version sites must now agree, and CI-fails when they do not:

| Site | Value |
|---|---|
| `client/package.json` | the three version sites must agree; current value in `../../docs/STATE.md` |
| `client/src-tauri/tauri.conf.json` | (same) |
| `client/src-tauri/Cargo.toml` | (same) |
| `legacy/wails-client` `main.go` + `wails.json` + `frontend/package.json` | frozen (stale, reference only) |

They are asserted equal by `client/src-tauri/tests/version_consistency.rs`, which
reads them from disk, so the updater's correctness dependency is **guarded**: the
update comparison runs against build-embedded version metadata, and a drifted
version would make the updater mis-decide (see `UPDATE-ARCHITECTURE.md` §4).

The fork is on its own line (`3.x`), not Clash Verge's inherited `2.5.5` —
publishing the inherited number would have pushed the fleet above the hub's
numbering permanently. The reasoning is settled in `docs/reference/STILL-OPEN.md`
("Fork versioning — RESOLVED").

---

## 4. CI layout constraint (important)

**GitHub only runs workflows from `.github/workflows/` at the repository root.**

The fork's CI now exists at `.github/workflows/client.yml` (**repo root** — a
workflow under `client/.github/workflows/` would never trigger). It builds,
signs and releases the shipping fork for four platforms. See `docs/operate/CI-CD.md`.

What a workflow here must know, all now encoded in `client.yml`:

- `defaults.run.working-directory: client` — **but `uses:` steps ignore it**, so
  every `pnpm/action-setup` needs `package_json_file: client/package.json`
  explicitly, or it cannot find the pinned version and fails.
- `cache-dependency-path: client/pnpm-lock.yaml`, and `--frozen-lockfile` so a
  `package.json` edit that skipped the lockfile fails in CI rather than on a user.
- A `verify` job gates the expensive builds: cross-language consistency, typecheck
  + `web:build`, and lint. Keep it separate so a lint failure costs seconds.
- `paths:` filters limit triggers to `client/**` and the workflow file itself.

| Need | Value |
|---|---|
| Rust | `rust-toolchain.toml` → **1.98.1** (`edition = "2024"` requires ≥1.85) |
| Node + pnpm | Node 22, `pnpm install --frozen-lockfile` |
| Linux deps | `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` |
| Windows | NSIS + WebView2 (`src-tauri/webview2.*.json`) |
| macOS | **both** slices build on `macos-14`; `macos-13` never started (retired image) |
| Caching | `Swatinem/rust-cache`, `workspaces: client` (the workspace root is `client/`) |
| Signing | `LOCUS_UPDATE_KEY` must be present or the build fails loudly |

---

## 5. Files that must be repointed

This is the step that breaks most quietly. Every known path consumer:

| File | Depends on | Action |
|---|---|---|
| `client/Makefile` | `$(realpath ../VERSION)` | REMOVED/void — the root `VERSION` file is deleted; the fork versions itself in three manifests guarded by a test |
| `bump.sh` (root) | `v5/VERSION` + Wails sites | **DELETED** |
| `scripts/release-cut.sh` | Wails/tag flow | **DELETED** |
| `scripts/publish-release.sh` | artifact paths, `manifest.json` | **KEPT, live** — takes the version as an argument, does not read root `VERSION` |
| `scripts/publish-update.sh` | **already broken** (see below) | retired to `legacy/publish-update.sh.broken`; `publish-release.sh` is correct |
| `scripts/generate_codes.sh`, `print_codes.sh` | hub only | unaffected |
| `server/modules/*.sh` | internal paths | verified — no dependency on the deleted files |
| `setup.sh` / `deploy-console.sh` | **deploys from `/root/server/`, not the repo** | documented drift trap — re-verify staging sync |
| `docs/reference/STILL-OPEN.md` | the `md5sum` drift-check snippet | repoint (`/root/server/pb_hooks` vs repo) |
| `.github/workflows/build.yml` | `v5/client/wails.json`, `v5/VERSION`, version grep | **DELETED** |

**`publish-update.sh` is already known-broken** and is listed in `STILL-OPEN.md` as such:
it writes no `sha256_<platform>` columns (clients refuse an empty hash) and its macOS
URLs (`locus-macos-amd64`) do not match CI's filenames (`locus-darwin-amd64`) → 404.
Do not "fix and keep" both publishers; the two-publisher divergence is the direct cause
of FIXES #31. Retire it.

---

## 6. `shared/contracts/` — NEVER CREATED

> **This section is a design note, not a description of the repo.** There is no
> `shared/contracts/` directory. The contract is currently enforced by the
> archived `legacy/wails-client/internal/updatecfg` test (reads the hook and the
> publish script as text) and by the hub hooks themselves — not by a shared
> schema directory. If you go looking for `shared/contracts/`, it does not exist;
> the text below is what such a directory *would* hold if it were built.

Both sides need the same definitions, and divergence between them is the root cause of
several recorded bugs. Candidates to move here (source of truth, with copies generated
or asserted rather than hand-maintained):

- Platform identifiers and artifact filenames
  (`LOGIC-INVENTORY.md`, `UPDATE-ARCHITECTURE.md` §2.2)
- The `update_config` record ↔ heartbeat response field rename table
  (`download_*` → `update_*`)
- The tier config shape, including the `uot_port` / `server_port_uot` alias
- Heartbeat constants (interval bounds, grace period, backoff)
- Which of these are read by the **Go legacy client**, **the Rust fork**, and
  **the PB hooks** — a `pb_hooks` file cannot import Rust, so the contract must be
  machine-checkable rather than merely documented

Note the constraint: PocketBase hooks run in goja (JS), the fork is Rust, the legacy
client is Go. A shared *file* is fine for documentation and for the Rust/Go sides;
the hook side needs the values inlined **plus** a test that asserts the inlined copy
matches. That mismatch is exactly FIXES #29 and #31.

---

## 7. Verified build baseline (2026-09-23)

The fork compiles clean on Linux after installing the Tauri dev deps. Recorded so a
successor can tell "the tree is broken" from "my environment is incomplete".

**System packages required (Ubuntu/Debian):**

```
pkg-config build-essential libssl-dev
libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev
libjavascriptcoregtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

Symptom when they are missing: `error: failed to run custom build command for
glib-sys v0.18.1`. Note a runtime `libglib-2.0.so.0` is **not** sufficient — the
`.pc` file and headers come from `libglib2.0-dev`.

**Critical: `node scripts/prebuild.mjs` must run before any Rust build.** It downloads
the sidecars and geo data into `src-tauri/sidecar/` and `src-tauri/resources/`, both of
which are **gitignored** (`src-tauri/.gitignore`). Without them:

```
error: failed to run custom build command for `locus v2.5.5`
resource path `sidecar/clash-verge-service-x86_64-unknown-linux-gnu` doesn't exist
```

What prebuild fetches: `verge-mihomo` + `verge-mihomo-alpha` (from MetaCubeX/mihomo
releases), the three `clash-verge-service*` binaries (from `clash-verge-service-ipc`
v2.7.3), and `country.mmdb`, `GeoLite2-ASN.mmdb`, `geosite.dat`, `geoip.dat`
(from MetaCubeX/meta-rules-dat).

**Verified commands and results:**

| Command | Result |
|---|---|
| `pnpm install --frozen-lockfile` | pass |
| `pnpm run web:build` (tsc + vite) | pass, 3–5 s |
| `node scripts/prebuild.mjs` | pass, fetches sidecars |
| `cargo check --manifest-path src-tauri/Cargo.toml` | **pass**, `Finished dev profile` in ~34 s |
| `cargo clippy --manifest-path src-tauri/Cargo.toml` | **pass**, zero warnings |
| `cargo metadata --locked` | pass |

**Gotcha — stale macro artifacts:** a Rust build interrupted *before* the system deps
were installed leaves a truncated `target/debug/deps/libgtk3_macros-*.so`. The next
build then fails with `error[E0786]: found invalid metadata files for crate
gtk3_macros`. Fix: delete the stale artifacts (`find target -name 'libgtk3_macros*'
-delete`, likewise `libgdk3_macros*`) — `cargo clean` also works but costs a full
rebuild. This is an artifact of the environment, not of the tree.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| A path consumer is missed and fails silently at release time | The table in §5 is exhaustive as of writing; grep for `v5/` and `v4/` repo-wide after the move and confirm zero hits |
| `git mv` on a directory with an untracked `client/` sibling confuses staging | Commit or explicitly stage `client/` before moving; verify `git status` is clean of surprises first |
| Two client workflows both build "the client" | Retire the Wails one when the fork ships; never both |
| `legacy/wails-client` still referenced by docs as the current client | Update `docs/` in the same change; stale docs are how the next agent gets misled |
| The move breaks the `md5sum` drift check against `/root/server/` | Re-run the check immediately after the move |
