# Recovering macOS self-update

```
audience:    human-operator
status:      live
authoritative-for: restoring macOS self-update after the v3.2.26 bare-Mach-O publish
verified-against: docs/reference/FIXES.md (2026-10-05), docs/operate/CLAIMS.md §5 A9
```

**Situation this document covers.** A macOS client is offered an update and
refuses it:

```text
the update to <version> is not an installer (<N> bytes); refusing to execute it
  — this build cannot install a raw executable, and neither can the platform installer
```

`N` is the size of the **bare Mach-O** binary (`48150583` for the `3.2.26`
`locus-darwin-arm64`). The client-side guard is correct and doing its job: it
stopped a payload its installer can never apply. **The hub is serving the wrong
file** — a bare Mach-O in a macOS *updater* slot, where `tauri_plugin_updater`
extracts a tar of an `.app` bundle. Full narrative in
[`../reference/FIXES.md`](../reference/FIXES.md), 2026-10-05.

> **This is the macOS twin of [`RECOVER-WINDOWS-UPDATE.md`](RECOVER-WINDOWS-UPDATE.md).**
> Same shape — the client is right, the hub serves the wrong artifact — on a
> different platform. Read whichever matches the error you have.

> **Three things must be true, in this order.** Each depends on the one before it.
>
> | # | What | Blocked on |
> |---|---|---|
> | 1 | A release carries `locus-darwin-<arch>.app.tar.gz` **and its `.sig`** | CI + `LOCUS_UPDATE_KEY` |
> | 2 | The **host's** `fetch-release.py` resolves the tarball | a deploy (`hooks-sync.sh --fetch-service`) |
> | 3 | `update_config` is re-published | console → Fetch & publish |
>
> **Publishing before step 2 changes nothing.** The fetcher on the host chooses
> the filename; while it is stale it resolves the bare Mach-O again, for the new
> version, and the symptom repeats unchanged.

---

## Why `v3.2.26` cannot simply be re-published

`v3.2.26` predates the macOS packaging step. Its GitHub Release carries only the
bare binary:

```console
$ curl -s https://api.github.com/repos/vasilliourous/Locus/releases/tags/v3.2.26 \
    | python3 -c "import json,sys; print([a['name'] for a in json.load(sys.stdin)['assets']])"
… 'locus-darwin-arm64' …        # present
…                               # NO locus-darwin-arm64.app.tar.gz
```

Its own `manifest.json` advertised the bare name for both macOS slots:

```console
$ curl -sL .../releases/download/v3.2.26/manifest.json \
    | python3 -c "import json,sys; d=json.load(sys.stdin); print({k:v['file'] for k,v in d['platforms'].items()})"
{… 'macos_intel': 'locus-darwin-amd64', 'macos_arm': 'locus-darwin-arm64'}
```

There is no tarball to serve and no tarball to sign, so **a new tag is
required**. Republishing 3.2.26 can only reproduce the bare-Mach-O row.

> **The hub no longer *silently* produces such a row.** `fetch-release.py` used to
> fall back to the bare Mach-O, log a WARNING, and publish anyway — which is
> exactly how 3.2.26 reached the live hub. It now **refuses** a macOS slot with no
> `.app.tar.gz` (`FetchError`, seen at publish time). So a pre-fix release is
> rejected instead of served.

---

## Step 0 — Confirm the diagnosis is still current

Cheap, and worth doing before touching anything:

```sh
# What would a macOS client on the previous version be offered?
curl -s "https://networkingguides.duckdns.org/api/update?version=<prev>&platform=macos_arm" \
  | python3 -m json.tool
```

- `"url": "…/locus-darwin-arm64.app.tar.gz"` → already fixed, stop.
- `"url": "…/locus-darwin-arm64"` (no `.app.tar.gz`) → proceed.
- HTTP `204` → nothing is being offered at all; check `update_config.active`.

You can also confirm the served bytes are a bare Mach-O (`cf fa ed fe …`) rather
than a gzip (`.app.tar.gz` starts `1f 8b`):

```sh
curl -s "…/updates/<version>/locus-darwin-arm64" | head -c 4 | od -An -tx1
```

---

## Step 1 — Cut a release that carries the macOS tarballs

The tarballs are produced by the CI step **"Package the macOS updater payload"**
and signed by **"Sign the macOS tarball as its own artifact"** (its OWN `.sig` —
minisign signs exact bytes, so the bare binary's signature cannot stand in).

```sh
cd client
pnpm release-version X.Y.Z          # writes ALL FIVE version sites
git add -A && git commit -m "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "Locus client vX.Y.Z"
git push origin main
git push origin vX.Y.Z              # the tag triggers the build
```

> `pnpm release-version` is the only supported bump — see
> [`RELEASING.md`](RELEASING.md).

### Verify the release before doing anything else

**This is the gate.** Do not publish until the tarballs and their signatures are
attached:

```sh
gh release view vX.Y.Z --repo vasilliourous/Locus --json assets \
  -q '.assets[].name' | sort
```

Required, and the tarball lines are the ones easy to be missing:

```text
installer-Locus_X.Y.Z_x64-setup.exe
installer-Locus_X.Y.Z_x64-setup.exe.sig
locus-darwin-amd64
locus-darwin-amd64.sig
locus-darwin-amd64.app.tar.gz        ← REQUIRED
locus-darwin-amd64.app.tar.gz.sig    ← REQUIRED
locus-darwin-arm64
locus-darwin-arm64.sig
locus-darwin-arm64.app.tar.gz        ← REQUIRED
locus-darwin-arm64.app.tar.gz.sig    ← REQUIRED
locus-linux-amd64
locus-linux-amd64.sig
locus-windows-amd64.exe
locus-windows-amd64.exe.sig
manifest.json
```

Also confirm CI's manifest names the tarballs:

```sh
curl -sL https://github.com/vasilliourous/Locus/releases/download/vX.Y.Z/manifest.json \
  | python3 -c "import json,sys; d=json.load(sys.stdin); print({k:v['file'] for k,v in d['platforms'].items()})"
# -> {'macos_intel': 'locus-darwin-amd64.app.tar.gz', 'macos_arm': 'locus-darwin-arm64.app.tar.gz', …}
```

---

## Step 2 — Deploy the fetcher to the host

The release fetcher lives on the host at `/root/server/scripts/fetch-release.py`
and runs as a systemd service. A fix in this repo is **inert** until it is
deployed — the whole 2026-10-01 Windows incident was this step being skipped.

```sh
# Show the repo-vs-host diff, change nothing:
server/scripts/hooks-sync.sh --fetch-service --dry-run

# Deploy it:
server/scripts/hooks-sync.sh --fetch-service
```

If the deployed copy already matches the repo, this is a no-op. Confirm the
service restarted; `hooks-sync.sh` proves the result rather than assuming it.

---

## Step 3 — Publish

**Console (normal):** Releases → enter the version → **Fetch & publish**.

**CLI:**

```sh
DRY_RUN=1 server/scripts/publish-release.sh X.Y.Z --from-github   # validate first
PB_ADMIN_EMAIL=… PB_ADMIN_PASS=… server/scripts/publish-release.sh X.Y.Z --from-github
```

> The CLI defaults to the bare VPS hostname, which has **no SSH key** and hangs
> at `install -d`. Always run it as `VPS=locus-hub server/scripts/publish-release.sh …`.

The fetch now **refuses** rather than degrades: a release with no macOS tarball
fails with a `FetchError` naming the platform and the file, and a stale deployed
fetcher fails with "the hub's platform filenames disagree with CI's manifest …
Update PLATFORMS … or re-deploy with `hooks-sync.sh --fetch-service`".

---

## Step 4 — Verify the hub is serving it

```sh
curl -s "https://networkingguides.duckdns.org/api/update?version=<prev>&platform=macos_arm" | python3 -m json.tool
curl -s "https://networkingguides.duckdns.org/api/update?version=<prev>&platform=macos_intel" | python3 -m json.tool
```

Each must return `"url": "…/locus-darwin-<arch>.app.tar.gz"`.

Then confirm the served bytes are a gzip, not a Mach-O:

```sh
curl -s "https://networkingguides.duckdns.org/updates/X.Y.Z/locus-darwin-arm64.app.tar.gz" \
  | head -c 2 | od -An -tx1      # expect: 1f 8b
```

And, if available, hash the served bytes against `update_config`:

```sh
server/scripts/verify-release.sh X.Y.Z     # read-only; hashes the SERVED bytes
```

---

## What "fixed" looks like

| Check | Passes when |
|---|---|
| `check-consistency.sh` §24 | all four sites name `.app.tar.gz`; a bare Mach-O in a macOS slot is `BAD` |
| `smoke-macos-payload-resolution.sh` | a pre-fix release (bare Mach-O only) is **refused**; a fixed release resolves the tarball |
| `/api/update?platform=macos_*` | `url` ends `.app.tar.gz` |
| the served tarball | first two bytes are `1f 8b` (gzip) |
| a macOS client | offered the update; downloads; installs; restarts on the new version |

---

## The guards this fix added, and why each exists

Each is in `server/scripts/check-consistency.sh` §24 and was **shown to fail**
against the code it catches (the repository's rule: a check that cannot fail
reads exactly like a check that passed).

| Guard | Catches |
|---|---|
| (h) `resolved = legacy` must not exist | the silent fallback that published the v3.2.26 bare-Mach-O row |
| (h) the refusal must `raise FetchError` | a crash with no reason, instead of "why nothing published" |
| (i) staging binds `resolved` from `available=set(assets)` | staging and the cross-check answering different questions |
| (i) the cross-check iterates the staged `resolved` list | comparing the manifest to *preferred* names while a different file is served |
| (j) `smoke-macos-payload-resolution.sh` | **behaviour** — catches a rewrite that keeps the identifiers but restores the fallback, which every grep above would miss |

---

## History

- **2026-10-04, v3.2.27** — the macOS packaging step landed: CI now builds and
  signs `locus-darwin-<arch>.app.tar.gz`, and the hub's `PLATFORMS` was corrected
  to resolve it (`90dbd4f`). Before this, macOS auto-update had **never** worked:
  both sides named a bare Mach-O in the update slot.
- **2026-10-05** — a `3.2.24` client offered `3.2.26` failed with *"not an
  installer (48150583 bytes)"*. Cause: the live `update_config` row was published
  through the **silent fallback**, because v3.2.26 predates the packaging step.
  Fix: the fallback is now a refusal, the cross-check resolves what staging
  serves, and §24 (h)/(i)/(j) guard both. Recovery: publish a release from ≥3.2.27.
