# Locus — Secure School VPN

A VPN service for students at N4L-managed NZ schools. It bypasses N4L's Palo Alto
filter using Shadowsocks TCP over a sing-box/mihomo TUN device — no TLS
fingerprinting surface, no client-visible SOCKS layer, and TCP is the only
transport that has to survive the filter.

> **This page is a front door, not a reference.** It says what Locus is and where
> to go next. Every fact it could restate lives in a document that owns it, and
> linking is the whole point — a second copy of a fact is a second thing that can
> disagree. The map of every document is
> [`docs/README.md`](docs/README.md); the rule for where a fact belongs is
> [`docs/STATE.md`](docs/STATE.md).

---

## Start here

| If you want to … | Go to |
|---|---|
| Work on **the client** | `client/` (Tauri 2 + React, mihomo engine) — [`client/AGENTS.md`](client/AGENTS.md), [`docs/README.md`](docs/README.md) |
| Work on **the hub / server** | `server/` — [`docs/operate/DEPLOY.md`](docs/operate/DEPLOY.md), [`docs/operate/OPS.md`](docs/operate/OPS.md) |
| **Cut a release** | [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md) — read [`AGENTS.md`](AGENTS.md) first; the tag/version order is not optional |
| **Diagnose a defect** | [`docs/reference/DEBUGGING-METHOD.md`](docs/reference/DEBUGGING-METHOD.md) |
| Write **a fact** anywhere | [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) — before you write it |
| Know what is **unfinished** | [`docs/reference/STILL-OPEN.md`](docs/reference/STILL-OPEN.md) |

---

## What Locus is

**Two plans.** Free is the funnel; Full is the product. Each has **its own**
UDP-over-TCP listener, because a sing-box inbound is bound to a single password.
The box keeps the internal row name `strike` for the paid plan — a code in the
field carries that string and the hub resolves tiers by it — while the
customer-facing name is **Full**.

The authoritative statement of tiers, ports, caps, `tc` classes and the UoT split
is [`docs/business/04-tiers.md`](docs/business/04-tiers.md). It is not restated
here, deliberately: the table that used to live on this page and the one in that
document were the same six numbers maintained by hand in two places.

**The product surface** is a first-run activation gate and a single
Connect / Disconnect. A student never sees a profile, a node or a proxy mode.
Activation codes are `RQ-XXXX-XXXX-XXXX-C` — 15 characters, Luhn-mod-N checksum,
charset `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`. The older `MYVPN-` form was retired in
the 2026-08-17 migration.

**Architecture.** The client is a Tauri 2 fork of Clash Verge Rev carrying Locus
product logic in `client/src-tauri/src/locus/` (contract, activation, device
fingerprinting, heartbeat, tier→config, store, apply, runtime supervisor,
hub-mediated updater). The hub is PocketBase + `pb_hooks/` behind Caddy, with the
Shadowsocks/tc/BBR stack installed by numbered modules under `server/modules/`.
Client internals are in [`client/docs/ARCHITECTURE.md`](client/docs/ARCHITECTURE.md);
the hub's HTTP contract is [`docs/reference/API.md`](docs/reference/API.md).

`legacy/wails-client/` and `legacy/v4/` are **retired** — read for *why*, never
build.

---

## Repository layout

```
Locus/
├── client/        the desktop app — Tauri 2 + React, Locus logic in src-tauri/src/locus/
├── server/        the hub — deploy modules, pb_hooks, console/, site/
├── docs/          live documentation; docs/README.md is the index
└── legacy/        retired clients. REFERENCE ONLY — read it, never build it.
```

Per-directory detail is in [`docs/CONTEXT.md`](docs/CONTEXT.md); what moved in
the 2026-09 separation, and which old paths are dead, is
[`client/docs/RESTRUCTURE.md`](client/docs/RESTRUCTURE.md).

> **Build output is not committed.** `client/target/`, `node_modules/`, `dist/`,
> the geo databases and the mihomo sidecar are gitignored and re-fetched by
> `client/scripts/prebuild.mjs`.

---

## Deploying and building

The deployable sequence — copy the tree, `setup.sh`, verify PocketBase, stage the
console, mint codes — is [`docs/operate/DEPLOY.md`](docs/operate/DEPLOY.md).
Building the client is [`client/AGENTS.md`](client/AGENTS.md); cutting and
publishing a release is [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md).

Two traps worth naming on the front page because they cost real incidents:

- **The version must be bumped before the tag is pushed.** `pnpm release-version`
  writes every version site; the tag name must match. CI does not bump versions —
  it labels the release from the tag. See [`AGENTS.md`](AGENTS.md).
- **Publishing to the hub is a separate manual step.** A GitHub Release alone does
  not make clients update.

### Version authority

The obsolete machinery (root `VERSION`, `bump.sh`, `stamp-syso.py`,
`release-cut.sh`, the committed `.syso` resources, `.github/workflows/build.yml`)
versioned only the retired Wails client and has been **deleted**. CI lives at
`.github/workflows/client.yml` — at the repo root, because GitHub ignores a
workflow under `client/.github/` — and on a `v*` tag it builds, **signs** and
releases every platform artifact plus `manifest.json`. Signing is mandatory: the
updater verifies a minisign signature over every download and has no bypass
([`client/docs/SIGNING.md`](client/docs/SIGNING.md)).

**The list of version sites is data, not a sentence.** It is
`[client.version_sites]` in [`docs/state.toml`](docs/state.toml), recomputed from
`client/scripts/release-version.mjs` by `check-consistency.sh` §17. Four documents
once drifted against each other on this number; the count is not written here for
that reason.

---

## Operations

Running, health-checking, backing up and restoring the hub is
[`docs/operate/OPS.md`](docs/operate/OPS.md). PocketBase and the console are
[`docs/operate/POCKETBASE-SETUP.md`](docs/operate/POCKETBASE-SETUP.md). Secrets
are age-encrypted and passphrase-based —
[`docs/operate/SECRETS-MANAGEMENT.md`](docs/operate/SECRETS-MANAGEMENT.md).

> ⚠️ **The hub holds live customer data.** Never bulk-delete `codes` or
> `code_events` — suspend or unbind instead. The rule, and why it exists in a
> product that deliberately stores no identity, is
> [`docs/STATE.md`](docs/STATE.md) and [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) §4.

**The only stable identifier for the deployed system is the domain** —
`[hub.domain]` in [`docs/state.toml`](docs/state.toml). There is deliberately no
IP address here. The hub's reachability is a **dated claim** in
[`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) §5, never a present-tense fact.

> **The hub answers on two names, and both are load-bearing.** The canonical one
> is `[hub.domain]` (since 3.3.2, a name under a domain the operator owns —
> `*.duckdns.org` was filtered by N4L, which took activation, heartbeat and the
> update path down together). The previous name, `[hub.domain_legacy]`, is
> **served from the same Caddy site block, not redirected**: every build before
> 3.3.2 has it compiled in and the only channel that could carry a new address
> (the heartbeat) needs a working hub to reach. **Removing it strands the
> deployed fleet at the activation gate** with no path to the update that would
> fix it. `05-caddy.sh` emits both; `check-consistency.sh` §32 fails the build if
> that stops being true. `locusvpn.jadedns.uk` is the landing page only and must
> never serve the API.

---

## Where else to read

| | |
|---|---|
| The whole documentation map | [`docs/README.md`](docs/README.md) |
| Project history, philosophy, layout | [`docs/CONTEXT.md`](docs/CONTEXT.md) |
| What is unfinished or unverified | [`docs/reference/STILL-OPEN.md`](docs/reference/STILL-OPEN.md) |
| The dated defect log | [`docs/reference/FIXES.md`](docs/reference/FIXES.md) |
| How the client differs from upstream | [`client/docs/UPSTREAM-CHANGES.md`](client/docs/UPSTREAM-CHANGES.md) |
| The commercial plan | [`docs/business/README.md`](docs/business/README.md) |
| Retired material (read for *why*) | [`docs/archive/`](docs/archive/), [`docs/history/`](docs/history/) |

---

## History in one paragraph

The repository was restructured on 2026-09-23: `v5/server/` → `server/`,
`v5/console/` → `server/console/`, `v5/client/` → `legacy/wails-client/`,
`v4/` → `legacy/v4/`, and `v5/docs/` + `extra-details/` → `docs/`. All moves used
`git mv`, so history is intact. Earlier removals (`v3/`, `simplified/`,
`modular-vps/`, `originals/`) are recoverable from git history. The full narrative
— including why the Wails client was retired — is
[`docs/CONTEXT.md`](docs/CONTEXT.md) and [`docs/history/LAYOUT.md`](docs/history/LAYOUT.md).
