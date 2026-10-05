# Locus — Secure School VPN

A commercial VPN service for students at N4L-managed NZ schools (Macleans College).
Bypasses N4L's Palo Alto firewall using Shadowsocks TCP (no TLS fingerprinting, no UDP blocks).


## Start here

| If you want to … | Go to |
|---|---|
| Work on **the client** | `client/` — Tauri 2 client built on Clash Verge Rev, mihomo engine. See "Client status" below. Docs in `client/docs/` |
| Work on **the hub / server** | `server/` — deploy modules + PocketBase hooks. Docs in `docs/operate/DEPLOY.md`, `docs/operate/OPS.md` |
| Operate the **admin console** | `server/console/`, served at `/admin/` (see `docs/operate/POCKETBASE-SETUP.md`) |
| Find **any document** | `docs/README.md` — the documentation map (one line per doc, grouped by what you are trying to do) |
| Understand **the whole project** | `docs/CONTEXT.md` — read this first |
| See **what is still open** | `docs/reference/STILL-OPEN.md` |

### Client status

The client is **tailored and functional**. It is no longer a branding-only copy
of Clash Verge Rev: it has Locus logic — activation, tiers, a heartbeat, a
hub-mediated updater. Its product behaviour is pinned by the tree's tests and by
`check-consistency.sh`; the **live** verification of the connect path was a set
of dated observations against the deployed hub, not a standing property:

- activation validated against the deployed `/api/activate` and `/api/code-lookup`
- a generated tier config accepted by the real `mihomo` binary
- traffic egressing from the VPS rather than the local address
- a real heartbeat returning the strike tier's config

*(Those observations are historical — see [`docs/reference/STILL-OPEN.md`](docs/reference/STILL-OPEN.md)
for what is still unverified and [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) §5
for the dated register. Do not read this list as a claim about the hub today;
`/api/health` is the one live claim that is currently dated, and it is dated in
that register.)*

The product surface is a first-run **activation gate** and a single
**Connect / Disconnect**; a student never sees a profile, a node or a proxy mode.

**What is still open** is listed in `docs/reference/STILL-OPEN.md` — chiefly whether a
release has been published yet, and Windows/macOS platform verification. Read
that before assuming a given platform is proven.

### Version authority and CI

The old machinery (`bump.sh`, root `VERSION`, `stamp-syso.py`,
`release-cut.sh`, the committed `.syso` resources, `.github/workflows/build.yml`)
versioned **only** the archived Wails client and is **deleted**.

`client/src-tauri/Cargo.toml`, `client/package.json` and
`client/src-tauri/tauri.conf.json` carry the client's version and **must agree**
(current value: `[client.version]` in [`docs/state.toml`](docs/state.toml));
`client/src-tauri/tests/version_consistency.rs` asserts it.
CI lives at **`.github/workflows/client.yml`** — it must be at the repo root,
because GitHub ignores a workflow under `client/.github/` — and on a `v*` tag it
builds, **signs** and releases the four platform artifacts plus `manifest.json`.
Signing is mandatory: the updater verifies a minisign signature over every
download and has no bypass. See `client/docs/SIGNING.md`.

The fork is on its own version line (`3.x`), not the inherited Clash Verge
`2.5.5`; the reasoning is settled in `docs/reference/STILL-OPEN.md` ("Fork versioning —
RESOLVED").

### History

The repository was restructured on 2026-09-23: `v5/server/` → `server/`,
`v5/console/` → `server/console/`, `v5/client/` → `legacy/wails-client/`,
`v4/` → `legacy/v4/`, and `v5/docs/` + `extra-details/` → `docs/`. Moved with
`git mv`, so history is intact. Older removals (`v3/`, `simplified/`,
`modular-vps/`, `originals/`) are recoverable from git history; the still-relevant
research (business model, N4L threat analysis) is preserved in `docs/history/`.

---

## Architecture Overview

> The **client** diagram below describes the **retired Wails client**
> (`legacy/wails-client/`) and is kept for reference. The shipping client is the
> Tauri fork in `client/`; its architecture is documented in
> `client/docs/ARCHITECTURE.md`. The **server** half of the diagram is current.

```
┌─────────────────────────────────────────────────┐
│              STUDENT'S LAPTOP                     │
│                                                   │
│  ┌───────────────────────────────────────────┐   │
│  │    Locus Desktop App — RETIRED (Go + Wails)  │   │
│  │         (Vue 3 UI embedded in binary)       │   │
│  │                                             │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │   │
│  │  │Activation│  │ Heartbeat │  │ Updater   │  │   │
│  │  │ Client   │  │ 5min→2h   │  │ 2-phase   │  │   │
│  │  └────┬─────┘  └────┬─────┘  └─────┬────┘  │   │
│  │       │              │              │        │   │
│  │  ┌────┴──────────────┴──────────────┴────┐   │   │
│  │  │         Manager (sing-box)            │   │   │
│  │  │   Generates config, spawns process    │   │   │
│  │  └─────────────────┬────────────────────┘   │   │
│  └────────────────────┼────────────────────────┘   │
│                       │                            │
│              ┌────────┴────────┐                   │
│              │  sing-box TUN   │  (no SOCKS5 —     │
│              │  device (locus0)│   TUN routes all  │
│              │                 │   device traffic) │
│              └─────────────────┘                   │
└───────────────────────┼────────────────────────────┘
                        │ Shadowsocks TCP
                        ▼
┌─────────────────────────────────────────────────┐
│              VPS (Ubuntu 22.04)                   │
│                                                   │
│  Caddy (TLS + rate_limit) ←──→ PocketBase        │
│       │                              │            │
│       │                         ┌────┴────┐       │
│       │                         │ JS Hooks│       │
│       │                         │  • Act. │       │
│       │                         │  • HB   │       │
│       │                         └─────────┘       │
│  ┌────────────────┐  ┌────────────────┐          │
│  │ Free (eco)     │  │ Full (strike)  │          │
│  │ :8443          │  │ :8445          │          │
│  │ BBR            │  │ BBR+UDP        │          │
│  │ 1M tc          │  │ 100M tc        │          │
│  └────────────────┘  └────────────────┘          │
│                                                   │
│  Backups → Backblaze B2 (hourly, 7-day retention)  │
└─────────────────────────────────────────────────┘
```

The **retired** client shipped two binaries: `locus` (desktop app, Wails + Vue 3)
and `sing-box` (tunnel engine), with no SOCKS5 layer. The **shipping fork** ships
the `locus` binary plus a bundled **`verge-mihomo`** sidecar as the tunnel engine
(see `client/docs/ARCHITECTURE.md`). The server side is unchanged.

---

## The Two Plans

| Plan | Price | TCP | UDP (UoT) | Server CC | Bandwidth | Experience |
|------|-------|:---:|:---------:|:---------:|:---------:|------------|
| **Free** | $0/mo | 8443 | ✅ 8447 | BBR | 1 Mbps (tc capped, both) | 10 GB/mo of chat, browsing, and light gaming (Roblox/Minecraft). The funnel. |
| **Full** | $5/mo | 8445 | ✅ 8446 | BBR | 100 Mbps TCP; UDP uncapped | Everything: streaming, downloads, competitive FPS. |

> **UDP is on both plans; the rate is the difference.** Each plan has its **own**
> UDP-over-TCP listener, because a sing-box inbound is bound to one password —
> 8447 serves the free plan's credentials and is capped at 1 Mbps at the hub,
> 8446 serves the paid plan's and is not capped. See
> [`docs/business/04-tiers.md`](docs/business/04-tiers.md) §4.1, §4.3.1 and §4.10.

> **Two plans, and `Full` is `strike` on the box.** The paid tier's row, password
> and systemd units keep the name `strike` because a code in the field already
> carries that string and the hub resolves it by that string; the customer-facing
> name is **Full**. Stealth (8444) was retired when the ladder was merged into one
> paid plan — Stealth and Strike were the same tier at two rates, differing only
> by the UDP endpoint. See [`docs/business/04-tiers.md`](docs/business/04-tiers.md)
> §4.2.4 and §4.6, and [`docs/business/17-not-built.md`](docs/business/17-not-built.md)
> for what is and is not built.
>
> All tiers are **BBR + tc**: no kernel modules to maintain, caps enforced with
> HTB classes plus `fq_codel` leaf qdiscs to keep latency flat under load.
> The paid tier's UDP-over-TCP endpoint (8446) is live on the hub — see
> [`docs/GAMING-UDP.md`](docs/GAMING-UDP.md).

Activation codes are **`RQ-XXXX-XXXX-XXXX-C`** (15 chars, Luhn-mod-N checksum,
charset `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`). The `MYVPN-` form was retired in the
2026-08-17 migration and no live code uses it.

---

## Project Layout

```
Locus/
├── client/                   ← THE LOCUS CLIENT (Tauri 2, built on Clash Verge Rev; Locus logic in src-tauri/src/locus/)
│   ├── src/                  ── React + TypeScript frontend
│   ├── src-tauri/            ── Rust backend (Tauri), capabilities, bundle config
│   ├── crates/               ── Inherited workspace crates (upstream, untouched)
│   ├── docs/                 ── ARCHITECTURE, LOGIC-INVENTORY, UPDATE-ARCHITECTURE,
│   │                            SIGNING, IDENTITY-MIGRATION, UPSTREAM-CHANGES
│   ├── scripts/prebuild.mjs  ── Fetches the mihomo sidecar + geo databases (MANDATORY before a Rust build)
│   ├── AGENTS.md             ── agent rules for the client (CLAUDE.md / GEMINI.md load it)
│   └── CONTRIBUTING.md       ── human build/submission guide
├── server/                   ← LIVE hub
│   ├── modules/              ── Numbered VPS deploy modules (00-env … 08-firewall)
│   ├── pb_hooks/             ── PocketBase JS hooks (activation, heartbeat, release, console)
│   ├── console/              ── Admin SPA, served at /admin/
│   ├── scripts/              ── publish-release, fetch-release, hooks-sync, seed-*, smoke-*, deploy-console
│   └── setup.sh              ── Orchestrator (deploys from /root/server, not this repo — see docs/operate/DEPLOY.md)
├── legacy/
│   ├── wails-client/         ← RETIRED Wails + sing-box client (STALE. Reference-only for its logic; see ARCHIVED.md)
│   └── v4/                   ← Historical Go + Fyne client (stale)
├── docs/                     ← LIVE project documentation
│   ├── ...                   ── Architecture, deploy, ops, API, FIXES, CONTEXT, STILL-OPEN
│   ├── archive/              ── Retired-client docs (build guide, backend API, migration, aesthetics)
│   └── history/              ── Curated pre-V5 research + dated session records
└── scripts/                  ← Code generation + local probes
    ├── generate_codes.sh     ── Luhn-mod-N activation codes
    ├── print_codes.sh        ── Printable PDF code cards
    └── vps-test/             ── Read-only network probes against the live VPS
```

> **Build output is not committed.** `client/target/`, `node_modules/`, `dist/`,
> geo databases and the mihomo sidecar are all gitignored and re-fetchable/rebuildable
> (`client/scripts/prebuild.mjs` fetches the binaries/databases; they are not in git).
>
> **No version file and no bump script.** Root `VERSION`, `bump.sh`,
> `server/scripts/bump-version.sh`/`stamp-syso.py`/`smoke-bump.sh`,
> `scripts/release-cut.sh`, the committed `rsrc_windows_*.syso` resources, and
> `.github/workflows/build.yml` were **removed** — they versioned and released the
> retired client, not the shipping fork. The fork versions itself (three sites,
> asserted equal by a test) and is released by CI at
> `.github/workflows/client.yml`; see `docs/operate/RELEASING.md`. The client's inherited
> upstream READMEs, changelog pipeline, devcontainer, and release/CI helpers were
> removed with it.

---

## How It Works

### Network Protocol

The VPN uses **Shadowsocks TCP** — a simple, fast tunnel protocol that encrypts traffic with AES-256-GCM. Unlike TLS-based proxies (Trojan, Xray VLESS), Shadowsocks has no TLS handshake or certificate exchange, so it bypasses N4L's JA3 fingerprinting.

All traffic goes through a single TCP connection per tier. The **retired** client
ran **sing-box**, creating a **TUN interface** (`locus0`, `10.0.0.1/30`) and routing
all device traffic through it. The **shipping fork** uses **mihomo** for the same
role. Either way the tunnel encrypts with Shadowsocks and sends to the VPS — no
SOCKS5 layer, no per-app configuration.

### Tiers & Congestion Control

Both tiers use **BBR** (Bottleneck Bandwidth and Round-trip propagation time), Linux's default CC — fair, stable, and bufferbloat-friendly. (An earlier design used the `tcp-brutal` kernel module for the retired Stealth tier, but its aggressive rate-filling caused bufferbloat and jitter on the school network, so it was removed — see `docs/reference/FIXES.md`.) Bandwidth is capped purely with `tc` HTB classes on the VPS:

- **Free** (`eco` on the box): 1 Mbps (class 1:10, port 8443)
- **Full** (`strike` on the box): 100 Mbps (class 1:30, port 8445)

Class `1:20` (the retired Stealth tier) no longer exists in `04-tc.sh`.

### Activation Flow

1. User enters code in the app: `RQ-ABCD-EFGH-JKMN-T`
2. **Client-side Luhn-mod-N validation** catches typos instantly
3. App generates a **device fingerprint** (SHA256 of MAC address + disk serial + motherboard UUID)
4. POST to `/api/activate` with `{ code, fingerprint }`
5. Server validates Luhn checksum, looks up code, binds fingerprint
6. Returns tier config with server address, port, password, method
7. App stores config locally and starts the tunnel

### Heartbeat & Grace Period

The app sends a heartbeat every 5 minutes (POST `/api/heartbeat` with
`{ code, fingerprint }`). If the hub is unreachable:
- Interval doubles: 5min → 10min → 20min → ... → 2h max
- VPN keeps working (config is stored locally, no token dependency)
- **7-day grace period** before requiring re-activation

On server response:
- `200 OK`: Normal operation. Resets interval to 5min.
- `403 suspended`: Server-side suspension handling.
- `410 expired`: The code has lapsed; the client takes the tunnel down.
- `404 not found`: The code no longer exists in the hub's database.
- `update_available` field: an update is being offered (see below).

### Updates

Updates are advertised through the heartbeat, not a separate poll:
1. An operator publishes a release; the hub writes one **active** `update_config`
   row. Publishing *is* offering — there is no rollout percentage and no
   server-driven downgrade.
2. The next heartbeat carries `update_available` plus the per-platform URL,
   SHA-256 and minisign signature for this device's platform.
3. The client records the offer and prompts. On accept it downloads, verifies the
   SHA-256 and the signature (fail-closed — a missing hash or signature means no
   install), and hands the verified bytes to the platform installer
   (NSIS / `.app` / `.deb`).

A **credential-free** `GET /api/release` exists so a client that cannot reach the
heartbeat (unactivated, suspended, or too broken to activate) can still learn a
fix exists. See `docs/operate/UPDATE-SYSTEM.md` for the full pipeline.

### Device Fingerprinting

The fingerprint is a deterministic SHA256 hash of hardware identifiers:

1. **Strong**: MAC address + disk serial + motherboard UUID
2. **Medium**: MAC address + motherboard UUID
3. **Weak**: MAC address + hostname + machine_id
4. **Fallback**: Random UUID v4 (persistent for install lifetime)

This binds an activation code to a specific device. If the device is lost or broken, an admin can suspend the code (binding is preserved and can be un-suspended).

---

## Build Order

### Step 1: Deploy Server

```bash
# Copy the server tree AND the age key to the VPS
scp -r server age-key.txt root@your-vps:/root/server/

# Run setup (~10-15 min). Secrets decrypt automatically from secrets.env.age.
ssh root@your-vps "/root/server/setup.sh"
```

This provisions: BBR, 2× Shadowsocks, tc shaping (Free 1 / Full 100 Mbps, both
with fq_codel), the paid tier's UDP-over-TCP endpoint on :8446, Caddy + TLS
(serving `/admin/` and `/updates/`), PocketBase (+ collections/admin/hooks/tier
configs), the admin console, the release fetch service (`locus-fetch`), B2
backups, UFW + fail2ban.

**No follow-up steps.** Verify with `smoke-test.sh` (expect 23 passed / 0 failed).
Note `setup.sh` deploys **from the copy on the VPS**, not from your working tree —
keep `/root/server/` in sync with the repo. A fresh host also needs the console
bundle staged (`deploy-console.sh`) or `SKIP_CONSOLE=1`; see `docs/operate/DEPLOY.md`
→ "Staging the server tree".

Optional extras:

```bash
# Mint a first batch of codes as part of the deploy (once only)
FIRST_BATCH=50 FIRST_BATCH_MIDDLEMAN=Sarah ssh root@your-vps "/root/server/setup.sh"

# Opt outs
ENABLE_UOT=0      # skip the sing-box UDP-over-TCP endpoint (+ its firewall rule)
SKIP_CONSOLE=1    # skip the admin console (not recommended)
```

The admin console must already be built into a bundle for the deploy to succeed
(`scripts/deploy-console.sh` builds and uploads it); a missing bundle now fails
the deploy loudly rather than leaving `/admin/` 404ing.

### Step 2: Verify PocketBase

`06-pocketbase.sh` and `seed-pb.py` now create the admin, the collections, the
schema and the tier configs automatically — a failure here is **fatal**, not a
warning. To verify or customise by hand, see `docs/operate/POCKETBASE-SETUP.md`
(collections: `codes`, `tier_configs`, `activation_attempts`, `update_config`,
`code_events`).

### Step 3: Deploy the Admin Console

```bash
server/scripts/deploy-console.sh   # builds, uploads, verifies /admin/
```

Day-to-day operations (issuing codes, suspend/unbind, tiers, releases) then
happen at `https://<domain>/admin/` — no SSH required.

### Step 4: Generate & Print Codes

```bash
# Console: Codes & Clients -> Generate. Or from the CLI:
./scripts/generate_codes.sh https://networkingguides.duckdns.org YOUR_PB_ADMIN_JWT free 50
./scripts/print_codes.sh free-codes.txt free-cards.pdf

# The paid plan is minted against the `strike` row and prints as "Full".
./scripts/generate_codes.sh https://networkingguides.duckdns.org YOUR_PB_ADMIN_JWT strike 20
```

The generator needs the **PocketBase admin JWT** (PB 0.22 rejects
`ADMIN_API_TOKEN` for record access).

### Step 5: Build Client App

```bash
cd legacy/wails-client
make build          # current platform only
make build-all      # Linux + Windows + macOS
```

Or push a `v*` tag (e.g. `v3.1.1`) — GitHub Actions builds, **signs** and publishes,
for four platforms:

- the **installers** — a Windows `installer-*.exe` (NSIS), macOS `installer-*.dmg`,
  Linux `.deb` / `.rpm`. These are the downloads a person should run.
- the **updater payloads** — raw `locus-*` executables plus a `.sig` for each, and
  `manifest.json`. These are consumed by the in-app updater and by the hub's
  `fetch-release.py`; they are not an installation method (see above).

An untagged push to `main` builds the same installers and attaches them to the
rolling **`build-preview`** prerelease, so a fix can be installed and tested
without cutting a release. The preview deliberately carries **no** updater
payloads, signatures or manifest: those are what the hub resolves, and publishing
them from a branch would let an unreviewed build reach every client at once —
there is no rollout percentage and no server-driven downgrade.

### Step 6: Install & Test

**Windows:** run `locus-setup-<version>.exe` (installs to `%ProgramFiles%\Locus`).
**macOS:** open the `.dmg`, drag Locus into Applications, then right-click → Open
the first time (the build is unsigned).
**Linux:** install the `.deb` (or `.rpm`).

> **Always install from the installer. Do not run the raw `locus-*` executable.**
>
> `locus-linux-amd64`, `locus-windows-amd64.exe`, `locus-darwin-*` are the
> **updater payloads** — the files the in-app updater downloads and swaps itself
> with. They are published on every release because the hub requires them, and
> they are **not** a way to install Locus. (On Windows the updater is instead
> handed the NSIS installer, for the reason below.) Running one fails, and it
> fails confusingly:
>
> ```
> failed to run validation core "verge-mihomo" ...
> The system cannot find the file specified. (os error 2)
> ```
>
> because the raw payload ships alone — Tauri resolves the `verge-mihomo` sidecar
> relative to the executable's own directory, and there is nothing beside it. On
> Windows it is worse: the NSIS installer is what registers the system **service**
> (`StartVergeService`), and the service is what makes TUN reachable. Bypassing
> the installer means the service never exists, so connecting cannot work even
> when nothing else is wrong.
>
> Only releases carry installers. For an untagged build, use the **`build-preview`
> prerelease** — automated builds attach installers there so a fix can be tested
> without cutting a release.

Windows runs elevated, since TUN creation requires it. Test activation,
connection, heartbeat, and update.

---

## Operations

### Quick Health Check

```bash
ssh root@your-vps "
  systemctl is-active caddy pocketbase shadowsocks-eco shadowsocks-strike
  tc class show dev eth0 | grep -E '1:10|1:30'
  sysctl net.ipv4.tcp_congestion_control
  tc -s class show dev eth0 | head -10
"
```

### Backup & Restore

```bash
# Manual backup
/usr/local/bin/locus-backup.sh

# Full VPS restore
DOMAIN=networkingguides.duckdns.org \
  B2_APPLICATION_KEY_ID=xxx \
  B2_APPLICATION_KEY=xxx \
  B2_BUCKET=my-vpn-backup-bucket \
  /root/server/restore.sh
```

### Monitoring

Set up uptime monitoring on `https://networkingguides.duckdns.org/api/health` (UptimeRobot, PingPing, etc.).
This endpoint returns `{"message":"API is healthy.","code":200}` when PocketBase is running.

Day-to-day operations live at `https://networkingguides.duckdns.org/admin/`
(admin token required) — dashboard, codes, releases and tiers. See
`docs/operate/OPS.md`.

---

## Real-World Testing

The server modules were first proven on a Voyager VPS, then **re-proven on a
completely blank box** (2026-09-19) — which found seven further bugs that only
appear on a fresh host. A re-run is not a deploy test.

| Module | Status | Notes |
|--------|:------:|-------|
| 00-env | ✅ | OS, arch, root, disk, memory all validated (memory guard fixed for 512MB droplets) |
| 01-bbr | ✅ | BBR active, TCP tuning params set |
| 02-shadowsocks | ✅ | 2 instances installed and enabled |
| 04-tc | ✅ | Free 1Mbit, Full 100Mbit classes active (+ fq_codel) |
| 05-caddy | ✅ | Caddy with ratelimit plugin; also serves `/admin/` and `/updates/` |
| 06-pocketbase | ✅ | 0.22.21 installed; bootstrap failure is now fatal, not a warning |
| 07-backups | ✅ | Timer enabled; verification hashes the **downloaded** artifact, restore proven |
| 08-firewall | ✅ | UFW active, all ports open, SSH protected by **fail2ban** |

All 2 Shadowsocks services active (8443/8445). All tiers on BBR with tc caps (1/100 Mbps).
Caddy + PocketBase serving the API, the admin console, and release binaries.

**Live host:** the only stable identifier for the deployed system is the
**domain** — `[hub.domain]` in [`docs/state.toml`](docs/state.toml). There is
deliberately no address or host-spec fact here or in `STATE.md` (an address moves
and goes stale in silence); the hub's reachability is a dated claim in
[`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) §5, and earlier hosts are
retired.
End state of the blank-box run: all 8 modules exit 0, `setup.sh` re-runs are
idempotent, and `smoke-test.sh` reports **23 passed / 0 failed / 0 warnings**
*(this run is dated history, not a claim about today's hub)*.

> ⚠️ **Live customer data.** The hub holds real activation codes in daily use.
> Never bulk-delete `codes` or `code_events` — suspend or unbind instead.

See `docs/CONTEXT.md` for the full project context, `docs/reference/STILL-OPEN.md` for what is
unresolved, and `client/docs/UPSTREAM-CHANGES.md` for exactly how the client differs
from the Clash Verge Rev version it was built on.
