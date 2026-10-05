# Locus — Project Context (V5 Reference)

```
audience:    all
status:      live
authoritative-for: project history, philosophy, repo layout, and agent guidance
verified-against: docs/state.toml
```

> **Purpose:** Everything a future agent needs to understand this project without
> searching across the entire repository. Read this first before touching any code
> or documentation.
>
> **This covers:** project history, V5 philosophy, directory structure, what was
> tested and what broke, known issues, agent guidance.
>
> **Not covered here:** build steps (see `docs/`), API contracts (see `docs/reference/API.md`),
> server deployment (see `docs/operate/DEPLOY.md`).

> **⚠️ READ THIS BEFORE TRUSTING ANY "CLIENT" SECTION BELOW.** This document spans
> two eras and is **not uniformly current**. It began as the reference for the
> **retired Wails client**, and while its server material is live, several
> client-facing sections still describe that retired client — marked with a
> `⚠️ HISTORICAL` banner in place. The rule for the rest:
>
> | If a section is about… | Its status |
> |---|---|
> | `server/`, hooks, the hub, tiers, ops | **current** |
> | `client/` — the Tauri 2 fork | **current**; authoritative detail is in `client/docs/` |
> | `legacy/wails-client/`, `legacy/v4/` | **retired**, kept as the behavioural contract only |
> | anything saying "the client code lives in `legacy/wails-client/`" | **wrong** — that is pre-Tauri and predates the fork |
>
> **The shipping client is `client/`**, first shipped 3.0.0 and now on the 3.x
> line (current version: `[client.version]` in [`state.toml`](state.toml)). Where
> this file and the code disagree, the code is right and this file is a bug
> (`docs/README.md` rule 3).

> **Naming rule (read before "tidying up" a `myvpn` string).** The product is
> **Locus**. Two classes of legacy `myvpn` name survive on purpose:
>
> 1. **Live server paths and unit names** — `/usr/local/bin/myvpn-backup.sh`,
>    `myvpn-tc-apply.sh`, `/etc/myvpn/tc`, `/var/log/myvpn-*.log`,
>    `SKIP_CONSOLE`-adjacent unit descriptions. These exist on the deployed host
>    right now. Renaming them in the repo desynchronises the definition from the
>    running system, so a fresh `setup.sh` would install a file the existing
>    units do not call. **Rename only as part of a redeploy, never as a
>    drive-by.**
> 2. **Client backup directory** `.myvpn-backups/` — on every activated
>    machine's disk. Renaming it orphans existing rollback copies.
>
> Everything else — user-visible strings, hook headers, log banners, document
> text, the Windows executable's product name — is **Locus** and should be fixed
> on sight. The one that actually reached a user was the shadowsocks link tag in
> the (now-deleted) `hiddify.pb.js` ("Eco - MyVPN"), fixed 2026-09-19
> (FIXES.md 34). That hook was later removed outright — it had never worked
> (goja has no `btoa`) and had no remaining consumer — so no live file carries
> this string any more.
>
> **Rebrand (2026-08):** This product was renamed **MyVPN → Locus** and re-themed
> from dark-purple to **dark green**. All client code, identifiers, the module path,
> binary/output names, the `locus0` TUN interface, app/log/code-prefix tokens, the
> GUI, and this documentation now use **Locus / locus**. The **deployed server
> keeps legacy `myvpn-*` artifact/path names** (`/etc/myvpn`, `myvpn-*.sh`,
> `myvpn-*.log`, `networkingguides.duckdns.org/…/myvpn-*`) because it has not been
> redeployed — treat those literal server paths as still-current until a fresh
> `setup.sh`/`restore.sh` run renames them. Note the modules themselves still
> *install* under `/etc/myvpn` and `/usr/local/bin/myvpn-*`, so a re-run does not
> rename anything either — the legacy names are not going away on their own.

> **⚠️ The hub holds paid codes in active use.** Do not assume it is a test
> system. **Never bulk-delete `codes` or `code_events` rows** — earlier in the
> project's life test-data cleanup was safe; it is not any more. To revoke access
> use **Suspend** (reversible); to move a student to a new laptop use **Unbind**.
>
> **The count and the distributor's name are deliberately NOT recorded here.** A
> live figure is wrong the next time a code is sold, and this product's core
> design decision is to store no identity — a distributor name in a pushed
> repository contradicts that in the cheapest possible place. Read the live figure
> from the console if you need it. The reasoning is in
> [`operate/CLAIMS.md`](operate/CLAIMS.md) §4.

---

## 0. Glossary — one name per thing

The docs use these words; this is what each one means, and which word to prefer.

| Term | Meaning | Notes |
|---|---|---|
| **hub** | The deployed server: the VPS running PocketBase hooks, Caddy and the two Shadowsocks services. | **Prefer "hub"** in prose. "server" means the same thing and survives in older text; `server/` is the code directory. |
| **client** | The shipping desktop app at `client/` — a Tauri 2 fork of Clash Verge Rev tunnelling through mihomo. | Old text that says "client" about the Wails build is retired; see `legacy/wails-client/`. |
| **tier** | A service level, rendered as a *plan*: **Free** (TCP 8443, UDP 8447, the `eco` row) or **Full** (TCP 8445, UDP 8446, the `strike` row). A tier fixes its ports and its tc cap. **Both plans carry UDP**; the differentiator is rate. | Canonical table: `business/04-tiers.md` §4.1. The row names are frozen wire names; the labels are not. |
| **code** | A student's activation code, `RQ-XXXX-XXXX-XXXX-C`. | The unit of entitlement. **Single-use and tied to no device** — device binding was removed (2026-10); the client keeps the code in a machine-scoped store, so a reinstall usually restores access with no re-entry. See `reference/DEVICE-IDENTITY.md`. |
| **plan** | What a student calls a tier: **Free** or **Full**. | The customer-facing name. The hub still sends the row name (`eco`, `strike`); one component maps wire name → label, so the two cannot drift. |
| **term** | A length of access bought once and measured from **activation** (not purchase), materialised into `expires_at` when a device binds. | Distinct from a calendar expiry date set at mint, which is what the hub used before the redesign. |
| **renew** | Extending a code's `expires_at` from the **later of now and the current expiry**. | Paying early never loses days. |
| **bind / unbind** | Attaching a code to a device fingerprint / releasing it. | **Suspend** (not delete) revokes; **Unbind** moves a student to a new laptop. |
| **UoT** | UDP-over-TCP — carrying game UDP inside the TCP tunnel. **Both plans have it**, each on its own listener (free 8447 capped 1 Mbps, paid 8446 uncapped). | See `GAMING-UDP.md` and `business/04-tiers.md` §4.3.1. |
| **FIXES / STILL-OPEN** | Two dated docs: what was broken and fixed, and what is still unfinished. | `reference/FIXES.md`, `reference/STILL-OPEN.md`. |

---

## 1. What This Project Is

A commercial VPN service for students at N4L-managed NZ schools (Macleans College).
It bypasses Palo Alto firewalls using **Shadowsocks TCP** — the only protocol that
consistently passes through N4L's detection (no TLS fingerprint, no UDP dependency).

The service has two plans:

| Tier | Price | Port | CC | Cap | Transport | Use case |
|------|-------|:----:|:---:|:---:|:---------:|----------|
| Free (`eco` on the box) | $0/mo | 8443 | BBR | 1 Mbps tc | TCP only | 10 GB/mo of chat and browsing |
| Full (`strike` on the box) | $5/mo | 8445 | BBR | 100 Mbps tc | TCP+UDP (UoT) | Streaming, downloads and gaming |

> **Two plans, and the on-box names differ from the customer-facing ones.**
> Both row names are FROZEN wire names because a code in the field carries its
> tier string and the hub resolves it by that string; the labels live in one
> place (`client/src/components/connection/tier-badge.tsx`). Stealth (8444) was
> retired when the two paid tiers were merged. Canonical table:
> [`business/04-tiers.md`](business/04-tiers.md) §4.1, §4.2.4 and §4.6.

**Activation codes are `RQ-XXXX-XXXX-XXXX-C`** (15 chars: `RQ` prefix + 3×4
random charset chars + 1 Luhn check char; charset
`ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, no `I/O/0/1`). The pre-2026-08-17
`MYVPN-XXXX-XXXX-XXXX-C` form is **dead** — codes were migrated, and no live code
uses the old prefix. The Luhn-mod-N checksum covers the whole body **including**
the `RQ` prefix, so any code generator, hook or validator must agree on that or
generated codes will fail on the device. (This detail is easy to get wrong from a
code read — the client validates the full 15-char string, while the server-side
checksum helper is fed the full body too.)

**Distribution model:** Middlemen hand out physical activation code cards for cash.
Students pay cash (no credit card needed). Middlemen take ~20-30% commission.
The live commercial plan is [`business/`](business/) (start at
[`business/README.md`](business/README.md)); the older business
and sales docs are kept privately — not in this repo.

---

## 2. Repository Structure

```
Locus/
├── client/              ← THE LOCUS CLIENT. Tauri 2, built on Clash Verge Rev
│                        tunnelling via mihomo. Has its own docs in client/docs/.
├── server/              ← LIVE hub. VPS deployment modules + PocketBase hooks
│   ├── console/         ← Admin console SPA (Vue 3 + Vite), served at /admin/
│   ├── modules/         ← Numbered deploy modules (00-env … 08-firewall)
│   ├── pb_hooks/        ← PocketBase JS hooks (activation, heartbeat, release, …)
│   └── scripts/         ← publish-release, fetch-release, hooks-sync, seed-*, smoke-*
├── legacy/
│   ├── wails-client/    ← RETIRED Go + Wails + sing-box client (was v5/client).
│   │                    STALE. Kept as reference for its logic when rebuilding
│   │                    the fork. See its ARCHIVED.md.
│   └── v4/              ← Predecessor Go + Fyne client. Reference only (stale).
├── docs/                ← Architecture, deploy, ops, API, fixes, CONTEXT (this file)
│   └── history/         ← Curated research archive + dated session records
├── scripts/             ← Code generator + PDF card printer (canonical location)
└── README.md            ← (root VERSION, bump.sh, .github/workflows/ removed — see below)
```

> **Restructured 2026-09-23.** Everything under `v5/` moved: `v5/server/` →
> `server/`, `v5/console/` → `server/console/`, `v5/client/` →
> `legacy/wails-client/`, `v5/docs/` + `extra-details/` → `docs/`. Moved with
> `git mv`, so history is intact. Any path in this document still reading `v5/`
> other than a historical note is stale — fix it.
>
> **Version authority — REMOVED.** The old machinery (root `VERSION`,
> `bump.sh`, `server/scripts/bump-version.sh`, `stamp-syso.py`, `smoke-bump.sh`,
> `scripts/release-cut.sh`, the committed `.syso` resources, and
> `.github/workflows/build.yml`) is **deleted**. It versioned only the archived
> Wails client, never the shipping fork. The fork now has **its own version
authority (three sites, asserted equal by a test) and its own CI** at
> `.github/workflows/client.yml`; see `docs/operate/RELEASING.md` and `docs/operate/CI-CD.md`.

### What goes where

| Directory | Purpose | For whom |
|-----------|---------|----------|
| `client/` | The Locus client — Tauri 2 on Clash Verge Rev, with Locus activation, tiers, heartbeat and updater. Functional and verified against the live hub; see `docs/reference/STILL-OPEN.md` for what remains | Builders of the client |
| `legacy/wails-client/` | RETIRED Go + Wails + sing-box client (see its `ARCHIVED.md`) | Contract/spec reference only |
| `server/console/` | Admin console SPA — day-to-day hub operations in a browser | Operators |
| `server/` | VPS deployment modules (bash) + PocketBase hooks | Server deployers |
| `docs/` | Architecture, client guide, deploy, ops, API, fixes | Client developers, operators |
| `docs/history/` | Curated pre-V5 research archive (business model, N4L threat analysis) | Reference only — read CONTEXT/ARCHITECTURE first |
| `legacy/v4/` | Predecessor Go + Fyne client code | Reference only |
| `scripts/` (root) | Luhn-mod-N code generator + PDF card printer | Middleman managers |

> **2026-08 culling (round 2):** `modular-vps/`, root `CONTEXT.md`, and
> `v5/scripts/` were deleted as stale duplicates — `server/` is the one and
> only server deployment directory (it absorbed `hiddify.pb.js`), root
> `scripts/` is the one and only code-generator location, and `docs/CONTEXT.md`
> is the one and only context document. (Their paths were later flattened again
> in the 2026-09-23 restructure above.) All deleted files remain recoverable
> from git history.
>
> **2026-08-14 follow-up:** macOS support was RE-ENABLED (unsigned local build
> — see `docs/archive/CLIENT-GUIDE.md` for the Gatekeeper workaround). The darwin
> code paths (`darwinTUN`, `pfctl` kill-switch, `networksetup` DNS, `ioreg`
> fingerprint), `darwin_link.go`, macOS CI targets, updater URLs, and Makefile
> targets were restored. macOS installs are unsigned: the user must
> right-click → Open (or `xattr -cr`) on first launch.
>
> **2026-08 history archive:** the still-relevant `originals/` docs (business
> model + N4L attacker/defender research) were restored to `docs/history/`
> after review; the rest of `originals/` (superseded action plans) stays in
> git history only.

---

## 3. V5 — What Makes It Different

V5 exists because earlier versions (V1–V4) were spread
across the repo with overlapping docs, untested assumptions, and no central reference.
V5 consolidates everything into one place: **both client and server code**, hardened
from real-world testing, with comprehensive documentation.

### Principles

1. **Only 2 binaries on the client** — locus (GUI + manager) + sing-box (engine).
   No TUN helper service, no tun2socks, no sslocal, **no SOCKS5 proxy layer**.
   BYOD means every user has admin rights, so sing-box creates TUN directly.

   > **Why no SOCKS5?** Earlier versions (V1–V4) used `ss-local` which exposes a
   > SOCKS5 proxy, adding +2 RTT handshake overhead per connection and requiring
   > per-app proxy configuration. V5 eliminated this entirely. sing-box's native
   > TUN inbound creates a virtual network interface and routes all device traffic
   > through it — no SOCKS5, no extra processes, no handshake overhead. The only
   > privilege needed is TUN creation, which sing-box handles directly on BYOD
   > machines. See `archive/ARCHITECTURE-wails.md` for the exact sing-box config.
2. **Server-enforced caps** — tc HTB qdisc on the VPS limits bandwidth per tier.
   The client can't bypass its cap because the throttle happens post-decryption.
3. **Permanent device binding** — one activation code = one device forever.
   SHA256 of MAC + disk serial + motherboard UUID. Admin can suspend, not deactivate.
4. **Crash-safe updates** — two-phase sentinel handshake. No signing keys needed.
   The updater also refuses anything that is **not strictly newer** than the
   running build, so a stale `update_config` row cannot downgrade the fleet.
5. **No TLS in the tunnel** — Shadowsocks AEAD is indistinguishable from random data.
6. **The tunnel is self-healing** — a 10s watchdog probes real traffic and escalates
   (restart → kill foreign engines + drop the stale `locus0` TUN → start fresh),
   then hands control back to the student rather than churning forever.
7. **Hub TLS is pinned by SPKI** — activation, heartbeat and update downloads
   reject a leaf key that is not on the allow-list. Pinning the *key* (not the
   cert) survives Let's Encrypt renewals. Empty pin list = fail-open with a
   warning, so a factory build still works until an operator sets
   `LOCUS_HUB_PINS`.

### Client Hardening (legacy/wails-client/ vs v4/)

> **⚠️ HISTORICAL — describes the RETIRED Wails client, not the shipping one.**
> Moved to [`archive/CONTEXT-client-hardening.md`](archive/CONTEXT-client-hardening.md)
> during the 2026-09 overhaul. Neither `legacy/wails-client/` nor `legacy/v4/`
> ships; the live client is `client/`. Nothing in that list applies to the
> shipping client, which inherits upstream Verge's hardening instead.

---

## 4. VPS Testing Results

All modules were originally proven on a Voyager VPS (Ubuntu 22.04,
kernel 5.15.0-161-generic). They were then **re-proven on a completely blank
box** on 2026-09-19 — which found seven further bugs that only bite on a fresh
host (see `docs/reference/FIXES.md` → "Blank-VPS deploy"). That blank-box run is the more
meaningful result, because a deploy script that only works on an
already-provisioned host is not a deploy script.

**Current hub:** addressed by its **domain** — see [`state.toml`](state.toml) for
that and the derived facts, and [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 for
whether it is actually up right now. Earlier hosts are retired — they went stale
three times, each time recorded here as a fact, which is why no address is written
down any more (`operate/CLAIMS.md` §1). Run `free -m` on any change; RAM is tight
and the memory guard in `00-env.sh` is deliberately tuned for it.

| Module | Status | Notes |
|--------|:------:|-------|
| 00-env | ✅ | OS, arch, root, disk, memory all validated (**memory guard fixed** — it rejected 512MB droplets) |
| 01-bbr | ✅ | BBR active, TCP tuning params set |
| 02-shadowsocks | ✅ | 2 instances installed and enabled |
| 04-tc | ✅ | Free 1Mbit, Full 100Mbit classes active (+ fq_codel leaf qdiscs) |
| 05-caddy | ✅ | Caddy with ratelimit plugin; now also `/admin/` SPA + `/updates/` file server. Console bundle is a **required** deploy input |
| 06-pocketbase | ✅ | 0.22.21 installed; bootstrap failure is now **fatal**, not a warning |
| 07-backups | ✅ | Timer enabled; verification now hashes the **downloaded** artifact |
| 08-firewall | ✅ | UFW active, all ports open, SSH protected by **fail2ban** |

End state of the blank-box run: all 8 modules exit 0, full `setup.sh` re-runs are
idempotent, and `smoke-test.sh` reports **23 passed / 0 failed / 0 warnings**.

**Zero-touch deploy (since the 2026-09 config pass):** a fresh `setup.sh` needs
no follow-up. It deploys the admin console and the release fetch service (both now
**required** — a missing bundle fails the deploy instead of warning), installs
the Strike UDP-over-TCP endpoint on 8446 and opens it in the firewall, seeds
tier configs with `uot_port` advertised, and installs everything enabled at
boot. Optional extras: `FIRST_BATCH=<n>` (+ `FIRST_BATCH_MIDDLEMAN`,
`FIRST_BATCH_EXPIRES`) mints a first batch of codes once, recorded in
`/root/.first_batch_done` so a re-run cannot create duplicate inventory. Opt
outs: `ENABLE_UOT=0`, `SKIP_CONSOLE=1`, `SKIP_DNS_CHECK=1`.

### 9 Issues Found & Fixed (first VPS round)

All documented in `docs/reference/FIXES.md`:

| # | Severity | Issue | Fix |
|:-:|:--------:|-------|-----|
| S1 | 🔴 | Caddy systemd service missing | Auto-create service with cap_net_bind_service |
| S2 | 🔴 | TLS cert provisioning failed | XDG_CONFIG_HOME + /var/lib/caddy |
| S3 | 🔴 | PocketBase NAMESPACE error | Removed systemd security hardening |
| S4 | 🟡 | Admin token extraction failed | Python-based seeding script |
| S5 | 🟡 | Bash heredocs mangled JSON | Moved to seed-pb.py |
| S6 | 🔴 | `!$double` bash bug | Replaced with if/else |
| S7 | 🔴 | PB 0.22 JS hook API incompatibility | All hooks rewritten |
| S8 | 🟡 | Smoke test IP detection off by one | Fixed awk pattern |
| S9 | 🟡 | Secrets decryption via process substitution failed silently | Temp file instead of `source <(cmd)` |

### Blank-VPS deploy — 7 further bugs (2026-09-19)

A fresh host is a different test from a re-run, and it found bugs that had been
invisible for months. Full write-ups in `docs/reference/FIXES.md`; the headlines:

| # | Issue | Consequence |
|:-:|-------|-------------|
| 1 | `00-env.sh` memory guard compared against 512 **MiB** | A "512MB" droplet reports 454MB — impossible to pass on the hardware it targeted |
| 2 | `seed-pb.py` used `/api/collections/_superusers/` (404 on PB 0.22) | **No admin, no collections, no schema** — while printing "✓ complete" |
| 3 | `06-pocketbase.sh` treated that as a warning | A hub serving 500s looked deployed; now fatal + health-poll |
| 4 | `08-firewall.sh` used `ufw limit 22/tcp` (6 conns/30s) | Locked out deployment automation; replaced with fail2ban |
| 5 | `smoke-test.sh`: `tc … | grep -q` SIGPIPEs `tc` | Under `pipefail` → exit 141, false tc failures |
| 6 | `update_config` duplicated a row every deploy | Multiple active rows; now reconciled |
| 7 | `findFirstRecordByData` **throws** on a miss | Unknown codes returned HTTP 500 instead of 404 in all four hooks |

The single most dangerous finding, though, was not a deploy bug at all:
**`$app.dao().findRecordsByFilter()` silently returns an empty array** on this
PocketBase build (0.22.21) for every collection and filter, with no error. It had
quietly disabled the **client update gate**, the **activation rate limit** and the
**code-lookup rate limit** — the latter two turning `/api/activate` and
`/api/code-lookup` into unbounded code-enumeration oracles. Use
`findFirstRecordByFilter` / `findRecordsByExpr` / `findFirstRecordByData` instead.
See `docs/reference/FIXES.md` → "Client update system" for the full list.

⚠️ **Do not** re-add `ufw limit 22/tcp` and **do not** inject a custom limiter
chain into `/etc/ufw/before.rules` — the latter locked SSH out completely (port
22 timed out while 80/443 served) and needed provider-console recovery.

---

## 5. Client Code Structure

### The current client — `client/` (Tauri 2, built on Clash Verge Rev)

> **Read `client/docs/UPSTREAM-CHANGES.md` first.** It is the authoritative list of
> what is ours versus upstream's, and it covers the load-bearing details that look
> like mistakes.

```
client/
├── src/                          # React + TypeScript frontend
│   ├── pages/activation.tsx      #   first-run gate — NOTHING else renders until activated
│   ├── pages/connection.tsx      #   the product: one Connect/Disconnect + traffic
│   ├── pages/account.tsx         #   device, tier, subscription, diagnostics
│   ├── components/connection/    #   the connection state machine (use-connection, phase)
│   ├── services/locus.ts         #   typed shims over the cmd::locus_* commands
│   └── services/hub.ts           #   the hub URL, in exactly one place
└── src-tauri/src/
    ├── locus/                    # ← ALL the product logic we own
    │   ├── contract.rs           #   every wire name: tier payload, frozen uot_port, platform ids
    │   ├── activation.rs         #   Luhn-mod-N, /api/activate, /api/code-lookup
    │   ├── device.rs             #   per-OS fingerprint (the code-binding key)
    │   ├── expiry.rs             #   subscription date parsing + the connect gate
    │   ├── heartbeat.rs          #   loop, backoff, jitter, 7-day grace
    │   ├── tier.rs               #   tier → mihomo YAML, incl. the UoT outbound
    │   ├── store.rs              #   product state as additive IVerge fields
    │   ├── apply.rs              #   writes the tier as a profile, calls Verge's pipeline
    │   ├── runtime.rs            #   owns the heartbeat loop's lifecycle
    │   └── update/               #   version compare, signal decode, installer hand-off
    ├── cmd/locus.rs              # thin Tauri commands over locus/*
    ├── core/ config/ enhance/    # INHERITED from Clash Verge Rev — do not rewrite
    └── tests/version_consistency.rs  # the three version sites must agree
```

**The architectural rule:** upstream's machinery is *used*, not replaced. The Locus
modules sit beside it and call into it — in particular `apply.rs` hands the tier to
`CoreManager::update_config_forced()`, so config validation, the service-vs-sidecar
decision and reload-vs-restart policy stay Verge's. **Never write a second apply
path.** The profiles *engine* (`config/profiles.rs`, `enhance/`) is load-bearing and
must not be deleted; only the profiles *UI* was removed.

### The retired client — `legacy/wails-client/`

> **⚠️ Reference only.** It does not ship. It is kept because it encodes the
> behavioural contract the current client reproduces, in executable form —
> `internal/updatecfg`'s test reads the hub's hook and publish script *as text* and
> asserts the field names agree, spanning three languages.

> **⚠️ Updated for the Wails migration (2026).** The GUI moved from Fyne to
> **Wails v2 + Vue 3** (`frontend/`). The old Fyne GUI, helper binary, and old
> entry point were removed in the Wails migration (the `v5/legacy/` reference
> copy was deleted in the 2026-08 cleanup; pre-migration client in `v4/`,
> git history for the removed files). The `internal/` backend packages are
> unchanged. See `docs/archive/WAILS-MIGRATION.md` and `docs/archive/BACKEND-API.md`.

```
legacy/wails-client/
├── main.go                      # Wails app entry (embedds frontend/dist, binds App)
├── app.go                       # App struct — wraps internal/ for the Vue UI
├── wails.json                   # Wails project configuration
├── internal/
│   ├── storage/storage.go      # Persistent JSON state (thread-safe, atomic writes, backups)
│   ├── activation/
│   │   ├── activation.go       # Activation client with retry + context + ValidateCodeFormat
│   │   ├── fingerprint_linux.go   # Self-contained fingerprint (shared logic + Linux collector)
│   │   ├── fingerprint_windows.go # Self-contained fingerprint (shared logic + Windows collector)
│   │   ├── fingerprint_darwin.go  # Self-contained fingerprint (shared logic + macOS collector)
│   │   └── luhn.go             # Luhn-mod-N checksum validation
│   ├── heartbeat/heartbeat.go  # Periodic server health check with jitter
│   ├── manager/
│   │   ├── process.go          # sing-box lifecycle + config generation
│   │   ├── watchdog.go         # 10s tunnel probes + recovery ladder (restart → reset → degraded)
│   │   ├── selfheal_{unix,windows}.go  # kill foreign engines, drop stale locus0 TUN
│   │   └── process_{unix,windows}.go   # process-group detach / Windows specifics
│   ├── pinned/pinned.go        # Hub TLS SPKI pinning (fail-closed once configured)
│   ├── tray/tray.go            # OPT-IN system tray (LOCUS_TRAY=1); no-op on darwin
│   ├── tunnel/tunnel.go        # TUN interface + kill switch (per-platform)
│   └── updater/
│       ├── updater.go          # Two-phase update with crash safety
│       ├── recover.go          # Crash detection and auto-revert
│       ├── version.go          # Numeric version compare — refuses downgrades
│       ├── update_linux.go     # Linux binary swap + fork
│       ├── update_windows.go   # Windows binary swap + fork
│       └── update_darwin.go    # macOS binary swap + fork
├── frontend/                    # Vue 3 + TypeScript + Vite UI
│   └── src/                    # App.vue, components/, stores/, lib/bridge.ts
├── engines/README.md           # Engine binary placeholder
├── (rsrc_windows_*.syso removed — version-stamped resources, deleted with the
│    archived-client release tooling; see the version-authority note above)
├── go.mod
└── Makefile                    # Wails build targets
```

Legacy (NOT in the Go module — removed in the 2026-08 cleanup):

```
(removed — old Fyne GUI, TUN helper binary, and entry point. Pre-migration
client in `v4/`; removed files recoverable from git history)
```

### Key Metrics

> **⚠️ These are metrics of the ARCHIVED Wails client** (`legacy/wails-client/`),
> not of the shipping `client/`. The line count below was also stale: the tree
> measures **9,504 lines across 48 Go files**, not ~7,700 across 38 — the figure
> predates the Wails migration. For the live client see
> `client/docs/ARCHITECTURE.md`.

| Metric | Value |
|--------|-------|
| Total lines (Go) | ~9,500 across 48 files (measured 2026-09-29) |
| Client version | the Wails client is **frozen** (its `VERSION`/`bump.sh` tooling is deleted). The **shipping fork versions itself** in its own manifests, asserted equal by a test — current value: `[client.version]` in [`state.toml`](state.toml) |
| Engine | sing-box 1.12.1 (client bundle + optional server UoT both pin 1.12.1) |
| Min Go version | 1.22 |
| Platforms | Linux, macOS (Intel+ARM, unsigned), Windows |
| Dependencies | Wails v2.12.0 + Vue 3 (Fyne removed) |

---

## 6. Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│                   CLIENT DEVICE                        │
│                                                        │
│  ┌────────────────────────────────────────────────┐   │
│  │              locus (Go + Wails / Vue 3)            │   │
│  │                                                  │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │   │
│  │  │Activation│  │ Heartbeat │  │   Updater    │   │   │
│  │  │ + retry  │  │ + jitter  │  │ 2-phase SAFE │   │   │
│  │  └────┬─────┘  └────┬─────┘  └──────┬───────┘   │   │
│  │       │              │               │            │   │
│  │  ┌────┴──────────────┴───────────────┴───────┐   │   │
│  │  │              Manager                       │   │   │
│  │  │  • Generates sing-box JSON config          │   │   │
│  │  │  • Spawns sing-box as subprocess           │   │   │
│  │  │  • Health monitoring + auto-restart        │   │   │
│  │  │  • Graceful shutdown with timeout          │   │   │
│  │  └───────────────────┬───────────────────────┘   │   │
│  └──────────────────────┼───────────────────────────┘   │
│                         │                                 │
│              ┌──────────┴──────────┐                      │
│              │  sing-box (engine)   │                      │
│              │  Creates TUN device  │                      │
│              │  Routes all traffic  │                      │
│              │  through Shadowsocks │                      │
│              └──────────────────────┘                      │
└──────────────────────────┼───────────────────────────────┘
                           │ Shadowsocks TCP (AES-256-GCM)
                           │ :8443 (free) / :8445 (paid)
                           ▼
┌──────────────────────────────────────────────────────┐
│                   VPS (Ubuntu 22.04)                    │
│                                                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │  Caddy   │  │PocketBase│  │  ssserver × 2        │  │
│  │  TLS +   │  │ SQLite   │  │  Free / Full          │  │
│  │  rate    │  │ JS hooks │  │  BBR / BBR            │  │
│  │  limit   │  │          │  │  1 / 100 Mbps         │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

---

## 7. Key Technologies

> **⚠️ HISTORICAL — this is the ARCHIVED Wails client's stack.** It is not the
> shipping client, which is Tauri 2 + React on the mihomo engine (see
> `client/docs/ARCHITECTURE.md`). Kept because the server-side rows still hold
> and the table records what the retired client required.

| Component | Technology | Version |
|-----------|-----------|:-------:|
| Client language | Go | 1.22+ |
| GUI toolkit | Wails v2 | 2.12.0 |
| Frontend | Vue 3 + Vite + TypeScript | ^3.4.0 |
| Tunnel engine | sing-box | 1.12.1 |
| Server OS | Ubuntu | 22.04 |
| Proxy protocol | Shadowsocks (ssserver-rust) | v1.23.0 |
| Reverse proxy | Caddy (custom rate_limit) | Latest |
| Database | PocketBase | 0.22.21 |
| TCP CC (all tiers) | BBR | Kernel built-in |
| Backups | Backblaze B2 | — |
| Traffic shaping | tc (HTB qdisc) | — |
| Firewall | UFW + fail2ban | — |

---

## 8. Agent Guidance

### Where to start

1. **Read `docs/README.md`** — the documentation index; it says what each doc is
   and which are historical.
2. **Read `docs/reference/STILL-OPEN.md`** for what is unfinished, including the open fork
   versioning/release-path decision.
3. **Read `client/docs/ARCHITECTURE.md`** and `client/docs/LOGIC-INVENTORY.md`
   for the **shipping** client (`client/`, the Tauri fork). `docs/archive/ARCHITECTURE-wails.md`
   is the *archived* client's design, kept for the contract it encodes.
4. **Refer to `legacy/wails-client/`** only as a **stale logic oracle** when
   rebuilding the fork — it is archived, not the product.
5. **Refer to `server/`** for the live hub and `docs/operate/DEPLOY.md`/`docs/operate/OPS.md` for
   operations.

### Rules of thumb

1. **The shipping client is `client/`.** The archived Wails client
   (`legacy/wails-client/`) and the predecessor in `legacy/v4/` are stale
   reference only; do not read them as current. There is **no root version file
   and no bump script** — the fork versions itself in three manifests guarded by
   a test, and CI lives at `.github/workflows/client.yml` (see §2).
2. **`legacy/v4/` is historical.** Never build from it; consult it only for
   historical context.
3. **The server modules are idempotent.** You can re-run any module safely.
   Each module checks if its work is already done before proceeding.
4. **The JS hooks have been rewritten for PocketBase 0.22+.** If activation
   returns a generic 400 error after a fresh deploy, check `journalctl -u pocketbase`
   for hook load errors.
5. **Both tiers use BBR — no kernel modules to maintain.** Bandwidth caps are tc-based
   (Free 1 / Full 100 Mbps); after a kernel update, re-apply with
   `systemctl restart tc-eco-cap tc-strike-cap`.
6. **The hub's stable identifier is its domain**, recorded in `state.toml`
   (DigitalOcean, Sydney; the OS/spec line there is marked unverified).
   **The address behind it is NOT stable and is deliberately not written down** —
   it changed three times, and the last recorded value stopped existing while
   still labelled authoritative. DNS is managed by the hosting provider, so
   **resolve the name; never cache the address** (`operate/CLAIMS.md` §6).
7. **Stopping PocketBase stops the backup timer.** `pocketbase-backup.timer` has
   `Requires=pocketbase.service` — systemd `Requires=` propagates stops but not
   starts, so after any `systemctl stop pocketbase` run
   `systemctl restart pocketbase-backup.timer`. `restore.sh` does this automatically.
8. **Restored DBs may have a stale admin password.** If the DB predates the current
   secrets file, `pocketbase admin update <email> <pass>` (in `/opt/pocketbase`)
   aligns it. `restore.sh` does this automatically when `PB_ADMIN_*` are set.
9. **Use the current b2 CLI syntax.** Plain bucket names fail ("Invalid B2 URI");
   use `b2://bucket/path` URIs (`b2 ls --recursive`, `b2 file download`, `b2 file info`).
   `restore.sh` uses the current syntax.
10. **Use the admin console for day-to-day work.** `https://…/admin/` replaces SSH +
    Python + sqlite for issuing codes, suspending/unbinding, editing tiers and
    publishing releases. The SSH recipes in `docs/operate/OPS.md` remain the fallback path
    (and are what the console itself calls).
11. **Do not bulk-delete `codes` or `code_events`.** Real customer data lives
    there — see the live-data warning at the top of this file.

### How the operator wants work done

These are working agreements established with the operator across the
2026-09 sessions. They are instructions, not trivia; they changed the work
materially.

- **Automate anything the operator would otherwise do by hand.** They are also the
  end user: *"make the project easy for me to deploy and use with little
  friction. Basically everything should be done with automation."*
  `server/scripts/deploy.sh` and the installed `locus-hub` SSH key exist for this
  reason. Do not hand over a list of steps when a script could do it; if a manual
  step is genuinely unavoidable, say so and say why.
- **Functionality first; polish later.** *"Let us not go overboard before having a
  functional product … then we prune and clean up afterwards."* Deferred cleanup
  is correct while the product is unfinished — but keep the list and do it once
  the thing works.
- **Do not plan instead of doing.** Produce a plan when asked, then **execute**
  and ask at genuine decisions — not at every step.
- **Ask when the decision is theirs** (version numbering, deleting a feature,
  signing-key custody). Do not guess on anything expensive or irreversible, and
  do not present long A/B/C prose menus.
- **Survey the whole system before deciding.** The operator pushed back hard on
  this once: features (the admin console, the updater) were found only after they
  prompted. Research first; this cuts the other way too — docs drift from code, so
  when they disagree **the code wins and the doc is the bug**.
- **Do not fixate on one thing.** The specific failure: finding Windows CI bugs
  one per 20-minute run instead of reading the code and fixing the class at once.
- **Communication:** direct and technical, no filler. **Correct your own earlier
  claims explicitly** when you find they were wrong. Say what is *verified*
  separately from what is *assumed* — "tests pass" is not "proven against the
  live system".

**Standing product decisions** (from the same sessions): TUN always on (not a mode
choice — a system-proxy mode passes traffic the school can see); no node selection
(one server per tier); Windows and macOS first, Linux last; **no macOS
code-signing/notarisation** (the operator supplies a "launch unsigned apps"
tutorial instead); the client version is its own line (not Clash Verge's inherited
`2.5.5`), with the three version sites guarded by a test.

### Key credentials (live VPS)

```
PocketBase admin:   admin@networkingguides.duckdns.org
PocketBase UI:      https://networkingguides.duckdns.org/_/
Admin API token:    <see /root/.admin_api_token on the VPS>
                    (the host is the DOMAIN, deliberately not an IP — CLAIMS.md §6)
B2 bucket:          vpsvpnbackup
```

> **History note (2026-09-29).** Earlier revisions of this file, in the *previous*
> repository, embedded the literal admin token; it was redacted and never entered
> this repository's history. This checkout's history is a fresh 3-commit initial
> import with no dangling or unreachable objects, so **no token is recoverable
> from it** — verified by searching every reachable object, not assumed from the
> redaction. The pre-migration history was not carried over.
>
> That is a property of *this* history, and it stops being true the moment old
> objects are pushed into it. If a legacy history is ever added or the repository
> is moved somewhere less trusted, rotate the token (`ADMIN_API_TOKEN` in
> `/etc/environment` on the VPS, then update `/root/.admin_api_token` and the
> console's bookmark). Rotating it invalidates any `/admin/?token=…` bookmark.

All credentials are stored on the VPS at `/root/` — see `docs/operate/POCKETBASE-SETUP.md` for
the full list of credential files and locations. Secrets never go in the repo in
plaintext; they live age-encrypted in `server/secrets.env.age`
(see `docs/operate/SECRETS-MANAGEMENT.md`).

### Common pitfalls

- **The shipping client is `client/`** — the Tauri 2 fork. Build from there.
- **`legacy/wails-client/` is RETIRED** and `legacy/v4/` is its predecessor.
  Both are reference-only; **never build from either**, and do not read them as
  current. (This bullet previously said "the client code now lives in
  `legacy/wails-client/` — always build from legacy/wails-client/", which was
  true only of the pre-Tauri era and is now exactly backwards.)
- **The JS hooks have been rewritten for PocketBase 0.22+** — if activation still returns a generic
  400 error after a fresh deploy, check `journalctl -u pocketbase` for hook load errors.
- **A new hook file needs a PocketBase restart.** Editing an existing `*.pb.js`
  hot-reloads; *adding* one does not. This is the usual reason a newly deployed
  endpoint 404s.
- **Never trust `findRecordsByFilter` in a hook.** It returns zero rows with no
  error on PB 0.22.21. See the note in §4.
- **Helper functions must be declared INSIDE a `routerAdd` handler.** File-scope
  declarations are not visible to the callback ("helperFn is not defined") — this
  silently 500'd an entire hook for its whole life.
- **DNS is managed by the hosting provider** — the hub's domain resolves to the
  current box without a DuckDNS updater. **This means the address is expected to
  change and must never be written down as a fact.** It has moved three times;
  each old address survived in these docs as a present-tense claim and the last
  one outlived the server. Resolve the name at the moment you need it
  (`operate/CLAIMS.md` §1 and §6).
- **The live hub is production, and there is no sandbox.** Stage changes on the
  VPS copy under `/root/server/`, never experiment on the live hooks.
