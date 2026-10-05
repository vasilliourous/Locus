# Retired — the 3.2.7 deployment packet

> **ARCHIVED (2026-10-05).** This was a *prepared deployment packet* for the
> 3.2.7 update and connection fixes, written 2026-09-29 while the hub was
> offline. **It has done its job** — 3.2.7 shipped, and the project is now on the
> version in [`../state.toml`](../state.toml) `[client.version]` — so what follows
> is a record of how those two fixes were sequenced, not an instruction to follow
> today.
>
> **Do not run this checklist.** Its commands target files, a schema and an
> `update_config` row as they were at 3.2.7. For the current procedure read
> [`../operate/RELEASING.md`](../operate/RELEASING.md) (cutting and publishing a
> release), [`../operate/UPDATE-SYSTEM.md`](../operate/UPDATE-SYSTEM.md) (the
> pipeline end to end) and [`../reference/FIXES.md`](../reference/FIXES.md) (what
> actually broke).
>
> It is kept because the *ordering* argument — code fix plus data fix, neither
> sufficient alone — is still the shape of an update-row repair.

---

## What ships, and why

Two severe fixes, both in `docs/reference/FIXES.md` (2026-09-29):

| # | Defect | Fix lives in | Blast radius |
|---|---|---|---|
| 1 | **Updates uninstallable on all four platforms** — `signature_<platform>` was stored as raw `.sig` text, failing the client's base64 decode with an offset error | `server/scripts/fetch-release.py`, `server/pb_hooks/admin_console.pb.js` | every client that sees an update offer |
| 2 | **"Connected" with no usable internet** — readiness proved only that the local Core answered | `client/src-tauri/src/core/manager/probe.rs`, `manager/mod.rs` | every client on a flaky network |

**Fix 1 is two things, and both are required:**

- **Code** — so a bad row cannot be created again (`fetch-release.py` now
  base64-encodes; `releases.publish` now shape-validates).
- **Data** — the *existing* live `update_config` row is corrupt and must be
  repaired. Fixing the code does not touch it. That is step 4 below.

---

## Preconditions

- The hub is reachable (`curl -fsS https://<hub>/api/health` or any known route).
- `PB_ADMIN_EMAIL` / `PB_ADMIN_PASS` for the hub, if using the CLI.
- The GitHub Release for the **previous** version (3.2.6) still exists with its
  `.sig` assets — the repair re-fetches them. (Verified present on 2026-09-29:
  all four `.sig` files served from `/updates/3.2.6/`.)
- A machine that can build the client and push a tag, for step 3.

---

## Checklist

### 1. Sanity: confirm the current production state is broken

Before changing anything, see the fault the way a client sees it:

```sh
server/scripts/smoke-signature-encoding.sh --live https://<hub>
```

Expected **now**: the four offline checks pass (the code is fixed) and the four
`live … NOT the correct wire format` checks **fail**. That failure is the shipped
bug, reproduced against production. After step 4 all eight must pass.

### 2. Deploy the server-side code (hooks + scripts)

The hooks and scripts only reach the host via `setup.sh` (see `DEPLOY.md`); a
change in this repo is inert until then.

```sh
# From the VPS, with the repo synced to /root/server as deploy expects:
server/setup.sh            # or: server/scripts/deploy.sh  — follow DEPLOY.md
```

After the hooks reload, confirm the guard is live by re-running step 1's *offline*
half on the host — it reads the deployed `fetch-release.py` and
`admin_console.pb.js`.

### 3. Build and release the client 3.2.7

The version is already bumped in all three sites (`package.json`, `Cargo.toml`,
`tauri.conf.json`, plus `Cargo.lock`) — `version_consistency.rs` asserts they
agree. CI does the build/sign/release on a tag:

```sh
git tag -a v3.2.7 -m "3.2.7 — repair update signatures; egress-proof readiness"
git push origin v3.2.7
```

CI (`.github/workflows/client.yml`, **repo root**) builds, **signs** and releases
all four platforms + `manifest.json`. Do not proceed until the GitHub Release
exists with binaries **and** `.sig` files — a release without signatures cannot be
published (the updater verifies mandatorily).

### 4. Repair the live update row (fix 1, the data half)

Either route writes the correct `base64(.sig)` encoding into `update_config`.

**Repair the *current* live version** (recommended first — it makes the shipped
3.2.6 installable again, so clients can update at all):

```sh
DRY_RUN=1 server/scripts/publish-release.sh 3.2.6 --from-github   # inspect
PB_ADMIN_EMAIL=… PB_ADMIN_PASS=… \
  server/scripts/publish-release.sh 3.2.6 --from-github           # write
```

**Then publish the new one**, by the normal route (console Releases → *Fetch &
publish*, or `publish-release.sh 3.2.7 --from-github`). Publishing 3.2.7 both
offers it and overwrites the row, so if you publish 3.2.7 directly, that alone
repairs the data — the 3.2.6 repair is only needed if you want the *existing*
clients to have a working install target before 3.2.7 is out.

### 5. Verify the live hub serves the right format

```sh
server/scripts/smoke-signature-encoding.sh --live https://<hub>
```

**All eight checks must pass.** If any `live … NOT the correct wire format`
remains, the row was not rewritten — re-check step 4 (the PATCH may have targeted
a duplicate row; `publish-release.sh` deletes duplicates, but confirm).

Manual confirmation of the same thing:

```sh
curl -s "https://<hub>/api/update?version=3.2.6&platform=linux" \
  | python3 -c 'import sys,json,base64; s=json.load(sys.stdin)["signature"]; \
      print(base64.b64decode(s, validate=True).decode())'
```

Must print four lines beginning `untrusted comment: signature from minisign
secret key`. If it errors with `Only base64 data is allowed`, the row is still
raw text.

### 6. Confirm fix 2 on a real machine (cannot be done here)

Per `docs/reference/STILL-OPEN.md`, neither readiness probe has met a live Core.
Before declaring 3.2.7 good, on a real machine with a real tunnel:

1. **Connect normally** → reaches **connected** (egress probe succeeds).
2. **With the Core running, pull the network** (disable wifi/ethernet) → the
   button must leave **connected** and settle on **connecting**, not hang and not
   stay green. This is the school-wifi case.
3. **Restore the network** → returns to **connected** without a reconnect.
4. **Slow link:** watch that the 3 s egress budget does not false-negative a
   working tunnel (the main risk of fix 2 — see `STILL-OPEN.md`).

### 7. Regression guards (already wired)

These run in CI / locally; nothing to deploy, listed so they are not skipped:

- `server/scripts/check-consistency.sh` §1b — the three-way signature-format guard.
- `server/scripts/smoke-signature-encoding.sh` — extracts and exercises the encode
  step and both hook guards; `--live` checks a hub.
- `client/src-tauri/tests/update_signature_contract.rs` — the client decode contract.
- `client/src-tauri/src/core/manager/probe.rs` tests — the two-proof readiness rule.

---

## Rollback

- **Server code:** re-run `setup.sh` from the previous commit. The guards are
  additive; no schema change.
- **Live row:** `releases.set {"active": false}` in the console switches the offer
  off. There is no server-driven downgrade, so a client that already installed
  stays on what it has.
- **Client:** 3.2.7 is not installed until the user accepts the prompt; nothing is
  forced.
