# `server/scripts/` — index

Every script here runs against the **live hub** unless it says otherwise. There is
no sandbox: the single VPS is production, and it holds real activation codes. Read
`docs/operate/OPS.md` before running anything for the first time.

Two conventions hold across this directory:

- **Hooks deploy from `/root/server/pb_hooks/`, not from the repo** — `setup.sh`
  reads the staging copy. Use `hooks-sync.sh` to push a hook change, or a repo edit
  is inert.
- **Prefer the console.** Most of these are the fallback path for when the console
  is unavailable, or the automation the console itself calls.

## Deploy and propagation

| Script | What it does | Safe to re-run? |
|---|---|---|
| `deploy.sh` | One command to put the repo's server state onto the hub (was: know which of six scripts to run, in what order, with which flags) | Yes — idempotent |
| `deploy-console.sh` | Build the admin-console SPA locally (needs `npm` on **your** machine, not the VPS) and upload it to `/root/server/console-dist.tar.gz` for `05-caddy.sh` to install. Requires `VPS` and `DOMAIN` explicitly | Yes |
| `hooks-sync.sh` | Copy the repo's `pb_hooks/*.pb.js` to the live hub and verify they landed (sha256 drift check, atomic move, restart, then a real request to `/api/release`). **`--fetch-service`** does the same for `fetch-release.py` at `/root/server/scripts/` (compiles the host's copy, probes `/health`) | Yes |
| `write-admin-credentials.sh` | Assemble every effective credential on the VPS into `/root/locus-credentials.txt` (mode 600): tier passwords, PB admin creds, admin API token, fetch-link HMAC secret, B2 creds. Ends with a fleet-divergence warning — read it | Yes |

## Seeding and one-off repairs

| Script | What it does |
|---|---|
| `seed-pb.py` | Create/reconcile the PocketBase collections and seed tier configs, `update_config` and the first batch of codes. **Reconciles — never drops or retypes.** Safe on a live hub *except* it re-seeds the `update_config` row |
| `fix-tier-configs.py` | Repair `tier_configs` on a live hub (empty collection, stale passwords, wrong port) without a full re-seed |
| `add-signature-columns.py` | Add the `signature_<platform>` columns to `update_config` on a live hub. **Deliberately not `seed-pb.py`**, which would overwrite the live release row with the empty `1.0.0` placeholder |
| `stamp-batch.py` | Stamp middleman/expiry onto a freshly generated batch of codes. Called by `setup.sh`; exists as a file (not an inline heredoc) because nesting Python inside `setup.sh`'s own heredocs is a quoting trap |
| `codes.json` | The `RQ-` activation-code set used to seed a fresh database — a fresh DB has an empty `codes` collection |
| `seed-live.py` | **DEPRECATED.** Near-duplicate of `seed-pb.py`; kept only for reference. Use `seed-pb.py` |

## Publishing a release

| Script | What it does |
|---|---|
| `fetch-release.py` | **The normal path.** Runs as `locus-fetch.service`; downloads a version's artifacts straight from its GitHub Release, verifies each one's format, SHA-256 **and** signature, then writes `update_config`. Also answers `POST /api/admin/fetch-release` (admin token) and `GET /api/admin/fetch-release?…&sig=` (one-shot signed link) |
| `publish-release.sh` | The CLI path for a hand-built or hotfixed binary not on a GitHub Release. Verifies served bytes by re-downloading. Refuses unsigned artifacts |

## Verification (no network / no live changes)

| Script | What it proves |
|---|---|
| `check-consistency.sh` | The cross-language guards: platform keys, frozen wire names, hook-written fields vs the schema, the single cipher definition, Windows-illegal tracked paths, the derived facts in `docs/state.toml`, and the `client/` licence carve-out. **Runs in CI and in `deploy.sh`** — run it before any change touching a wire name |
| `smoke-test.sh` | Post-deploy health: services, ports, tc classes, hooks, DB rows (expects *N* passed / 0 failed) |
| `smoke-publish.sh` | That `publish-release.sh` **refuses** bad input (truncated artifact, tampered bytes, wrong version) — the failure that matters is a wrong upload, not a failed one |
| `smoke-update-endpoint.sh` | That the hub's version rules agree with the client's, by extracting the logic from the shipped hook |
| `verify-release.sh` | "Is this release actually shippable?" in one command — the checks that a release can look finished and fail |
| `verify-ss2022.sh` | **Run on the host** after `setup.sh`: proves the Shadowsocks 2022 migration took effect (cipher and key length per listener). Separate from `smoke-test.sh`, which answers "is the deployment healthy?" |

## Operations

| Script | What it does |
|---|---|
| `enable-uot.sh` | Idempotently install and start the sing-box UDP-over-TCP endpoint on an already-deployed host, then advertise `uot_port` + `udp_relay=true` for Strike. A fresh deploy enables UoT by default — this is for an older host |
