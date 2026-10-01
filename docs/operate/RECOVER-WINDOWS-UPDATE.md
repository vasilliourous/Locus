# Recovering the Windows update path

```
audience:    human-operator
status:      live
authoritative-for: restoring Windows self-update after the 2026-10-01 hub drift
verified-against: docs/reference/FIXES.md (2026-10-01), docs/operate/CLAIMS.md §5 A7
```

**Situation this document covers.** No Windows client can update itself. The hub
serves `locus-windows-amd64.exe` — the raw PE — for the `windows` slot, and every
client correctly refuses it with:

```text
the update to <version> is not an installer (<N> bytes); refusing to execute it
```

`N` is the size of the raw binary (`51741696` for 3.2.14). The client-side guard
is doing its job; the hub is serving the wrong file. Full narrative in
[`../reference/FIXES.md`](../reference/FIXES.md), 2026-10-01.

> **Read this first: three things must all be true, in this order.** Each depends
> on the one before it, and doing them out of order produces a release that looks
> healthy and still cannot update anybody.
>
> | # | What | Blocked on |
> |---|---|---|
> | 1 | A release carries `installer-Locus_<v>_x64-setup.exe` **and its `.sig`** | CI + `LOCUS_UPDATE_KEY` |
> | 2 | The **host's** `fetch-release.py` resolves the installer | a deploy (`hooks-sync.sh --fetch-service`) |
> | 3 | `update_config` is re-published | console → Fetch & publish |
>
> **Publishing before step 2 changes nothing.** The fetcher on the host is the
> thing that chooses the filename; while it is stale it will stage the raw binary
> again, for the new version, and the symptom repeats unchanged.

---

## Why the current release cannot simply be re-published

`v3.2.14` has the installer but no signature over it:

```console
$ curl -s https://api.github.com/repos/vasilliourous/Locus/releases/tags/v3.2.14 \
    | python3 -c "import json,sys; print([a['name'] for a in json.load(sys.stdin)['assets']])"
… 'installer-Locus_3.2.14_x64-setup.exe' …   # present
…                                             # NO installer-*.sig
```

`tauri_plugin_updater` verifies a minisign signature **mandatorily and with no
bypass** (`install_inner` → `verify_signature()`), and the hub's fetcher refuses
a Windows slot without one. So an unsigned installer is not installable, and
re-publishing 3.2.14 is a dead end. **A new tag is required.**

---

## Step 0 — Confirm the diagnosis is still current

Cheap, and worth doing before touching anything: if the hub has already been
corrected, the rest of this is unnecessary.

```sh
# What would a Windows client on the previous version be offered?
curl -s "https://<domain>/api/update?version=<prev>&platform=windows" | python3 -m json.tool
```

- `"url": "…/installer-Locus_…-setup.exe"` → already fixed, stop.
- `"url": "…/locus-windows-amd64.exe"` → proceed.
- HTTP `204` → nothing is being offered at all; check `update_config.active`.

---

## Step 1 — Cut a release that carries the installer signature

The installer's signature is produced by the CI step **"Sign the Windows
installer"**. It requires `LOCUS_UPDATE_KEY`; without it that step exits 1
deliberately, because an unsigned installer reaches nobody.

```sh
cd client
pnpm release-version X.Y.Z          # writes ALL FIVE version sites
git add -A && git commit -m "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "Locus client vX.Y.Z"
git push origin main
git push origin vX.Y.Z              # the tag triggers the build
```

> `pnpm release-version` is the only supported bump. It writes `package.json`,
> `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `Cargo.lock` and
> `../docs/state.toml`. See [`RELEASING.md`](RELEASING.md).

### Verify the release before doing anything else

**This is the gate.** Do not publish until the signature is attached:

```sh
gh release view vX.Y.Z --repo vasilliourous/Locus --json assets \
  -q '.assets[].name' | sort
```

Required, and the last line is the one that is easy to be missing:

```text
installer-Locus_X.Y.Z_x64-setup.exe
installer-Locus_X.Y.Z_x64-setup.exe.sig      ← REQUIRED
locus-darwin-amd64
locus-darwin-amd64.sig
locus-darwin-arm64
locus-darwin-arm64.sig
locus-linux-amd64
locus-linux-amd64.sig
locus-windows-amd64.exe
locus-windows-amd64.exe.sig
manifest.json
```

Also confirm CI's manifest names the installer:

```sh
curl -sL https://github.com/vasilliourous/Locus/releases/download/vX.Y.Z/manifest.json \
  | python3 -c "import json,sys; print(json.load(sys.stdin)['platforms']['windows'])"
# -> {'file': 'installer-Locus_X.Y.Z_x64-setup.exe', 'sha256': '…', 'signature': '…'}
```

No `.sig` on the release → CI is not signing the installer. Fix CI and re-tag;
do not try to work around it.

---

## Step 2 — Deploy the corrected fetcher to the host

The host runs its **own** copy at `/root/server/scripts/fetch-release.py`. The
repo being correct does nothing until this copy matches it — that gap is the
entire defect, and it is why this step exists as its own step.

```sh
# Show the diff first. Changes nothing, needs no restart.
server/scripts/hooks-sync.sh --fetch-service --dry-run

# Then deploy: sha256-compared, uploaded to a temp name, moved atomically,
# service restarted, and VERIFIED (service active, file compiles, /health 200).
server/scripts/hooks-sync.sh --fetch-service

# Confirm at any later point:
server/scripts/hooks-sync.sh --fetch-service --check
```

On a host with password-only access, supply a transport rather than editing the
script — it does not invent credentials:

```sh
VPS=root@<host> SSH="sshpass -e ssh" server/scripts/hooks-sync.sh --fetch-service
```

**Verify the deployed fetcher is the fixed one.** This is the check that catches
a deploy that "succeeded" without changing anything:

```sh
ssh root@<host> 'grep -c "installer-Locus" /root/server/scripts/fetch-release.py'
# -> at least 1. 0 means the old file is still there.
```

> **Order matters here too.** The corrected fetcher *requires* the installer
> signature. Deploying it while pointing at a release that lacks one means the
> next publish is refused rather than silently wrong — which is the intended
> behaviour, but it will look like a new failure if you did not expect it.

---

## Step 3 — Publish

Console → **Releases** → enter the version → **Fetch & publish**.

The fetcher downloads from the GitHub Release, checks each file's format, hashes
it, cross-checks every filename against `manifest.json`, and then the
`releases.publish` hook writes `update_config`.

Publishing **is** offering. There is no rollout percentage; `active` is the only
off switch.

---

## Step 4 — Verify from outside

Do not trust the console's success message. Ask the hub what it would actually
hand a client:

```sh
curl -s "https://<domain>/api/update?version=<prev>&platform=windows" | python3 -m json.tool
```

The URL must end in `installer-Locus_<v>_x64-setup.exe`, and `sha256` must match
the installer's, not the raw binary's. A `signature` whose decoded text ends
`trusted comment: Locus_windows-installer` is the installer's own signature;
`trusted comment: Locus_windows` is the **raw binary's** and means step 2 did not
take effect.

```sh
# The whole thing, including hashing the served bytes:
server/scripts/verify-release.sh X.Y.Z

# The account-free manifest:
curl -s "https://<domain>/api/release" | python3 -m json.tool
```

Then, and only then, install on a real Windows machine, take the offered update,
and relaunch from the Start Menu.

---

## What this does NOT verify

**`CLAIMS.md` §5 A7 remains unverified after this entire procedure.** These steps
make the hub serve the correct bytes with a signature that verifies; they do
**not** demonstrate that a Windows machine installs them and keeps launching from
its install directory. That needs Windows hardware, and nothing in CI launches
either artifact — which is the reason three green releases shipped broken.

Record the result on `A7` when it is done. Do not let a green CI run stand in
for it.

---

## Rolling back

If a published release turns out to be wrong, `active = 0` **stops offering it
and nothing else**. Clients that already installed it stay on it, and there is no
server-driven downgrade — the only real fix is publishing a higher version.

```sh
# Console: Releases -> Stop offering.  Or on the host:
sqlite3 /opt/pocketbase/pb_data/data.db "update update_config set active=0;"
```

Set it back once a corrected release is published.

---

## Preventing the next one

The checks that now exist, so a future change does not have to rediscover them:

| Check | What it pins |
|---|---|
| `check-consistency.sh` §1a | the filename in **both** hub scripts, and that the stale one is gone |
| `fetch-release.py` | refuses to publish when a platform's filename disagrees with `manifest.json` |
| `fetch-release.py` | refuses a Windows PE with no NSIS overlay, mirroring the client |
| `smoke-publish.sh` case 5b | a manifest naming a different file is refused (proven to fail pre-fix) |
| `client` tests | `is_installer_payload` rejects the raw PE, accepts an NSIS bundle |
| `hooks-sync.sh --fetch-service` | the **host's** fetcher matches the repo |

The last row is the one this incident added and the others could not have. Every
check above it tests one side of a two-sided contract; none compared the repo
against what the hub was actually running.
