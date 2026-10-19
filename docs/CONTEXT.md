# Locus — project context: what it is, the names, and the layout

```
audience:    all
status:      live
authoritative-for: what the product is, the glossary, the naming rule, the live-data rule, and the repo layout
verified-against: docs/state.toml, docs/reference/DEVICE-IDENTITY.md, docs/business/04-tiers.md
```

One-page orientation: what is sold, what the words mean, which names must not be
"tidied", and where things live. The index is [`README.md`](README.md); the fact
rules are [`STATE.md`](STATE.md) and [`operate/CLAIMS.md`](operate/CLAIMS.md); the
client's internals are [`../client/docs/`](../client/docs/). Those are owners —
this file links, it does not restate. Retired material is in `archive/` and
`history/`, and every defect has a dated entry in [`reference/FIXES.md`](reference/FIXES.md).

---

## 0. Glossary — one name per thing

| Term | Meaning | Notes |
|---|---|---|
| **hub** | The deployed server: the VPS running the PocketBase hooks, Caddy and the two Shadowsocks services. | **Prefer "hub"** in prose. "server" means the same thing and survives in older text; `server/` is the code directory. |
| **client** | The shipping desktop app at `client/` — a Tauri 2 fork of Clash Verge Rev tunnelling through mihomo. | `legacy/wails-client/` is the retired pre-Tauri client and does not ship. |
| **tier** | A service level, rendered as a *plan*. The row names `eco` and `strike` are frozen wire names. | Canonical table: [`business/04-tiers.md`](business/04-tiers.md) §4.1. Both plans carry UDP; the differentiator is rate. |
| **plan** | What a student calls a tier: **Free** or **Full**. | The hub still sends the row name; one component maps wire name → label, so the two cannot drift. |
| **code** | A student's activation code, `RQ-XXXX-XXXX-XXXX-C`. | The unit of entitlement. **Single-use and tied to no device** — device binding was removed in 2026-10. See [`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md). |
| **term** | How long one purchase lasts, measured from activation. | `term_days` on the code, materialised into `expires_at`. |
| **middleman** | The reseller who sells a card for cash. | The distribution model; see [`business/08-distribution.md`](business/08-distribution.md). |

---

## 1. What this project is

A commercial VPN service for students at N4L-managed NZ schools (Macleans
College). It gets past Palo Alto firewalls using **Shadowsocks TCP** — still the
only transport that consistently survives N4L's detection. The reasoning is
research, not a rule: [`history/school-vpn-blocking-implementation.md`](history/school-vpn-blocking-implementation.md)
and [`history/comprehensive-vpn-blocklist.md`](history/comprehensive-vpn-blocklist.md).

Two plans, **Free** (row `eco`) and **Full** (row `strike`), both with
server-enforced tc caps. Ports, caps, prices and the free tier's design in full
are [`business/04-tiers.md`](business/04-tiers.md) §4.1 — the canonical table,
guarded against the tree. It is not copied here: a second copy is a second thing
that can drift.

**Codes** are `RQ-XXXX-XXXX-XXXX-C`: `RQ` + 3×4 random characters from
`ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no `I/O/0/1`) + one Luhn check character.
The checksum covers the whole body **including** the `RQ` prefix — a generator
that gets that wrong produces codes the device rejects. The pre-2026-08-17
`MYVPN-` prefix is dead; every live code was migrated.

**Distribution.** Middlemen sell physical activation code cards for cash (~20–30%
commission), so a student never needs a card. The commercial plan is
[`business/`](business/) (start at [`business/README.md`](business/README.md)).

**Ground truth about the deployed system** is never a sentence in this file —
derived facts live in [`state.toml`](state.toml), world claims in
[`operate/CLAIMS.md`](operate/CLAIMS.md) §5.

---

## 2. Repository layout

| Directory | What it is | Read next |
|---|---|---|
| `client/` | **The shipping client.** Tauri 2 + React on Clash Verge Rev; Locus logic in `src-tauri/src/locus/`. | [`../client/AGENTS.md`](../client/AGENTS.md), [`../client/docs/ARCHITECTURE.md`](../client/docs/ARCHITECTURE.md) |
| `server/` | **The live hub:** deploy modules (`modules/00-env.sh` … `08-firewall.sh`), `pb_hooks/`, `scripts/`, the operator console at `console/`, and the public landing page at `site/`. | [`operate/DEPLOY.md`](operate/DEPLOY.md), [`operate/OPS.md`](operate/OPS.md) |
| `docs/` | This set. Index: [`README.md`](README.md). | — |
| `docs/archive/`, `docs/history/` | Retired material. Read for *why*, never for *what is*. | [`history/README.md`](history/README.md) |
| `legacy/` | Retired clients (`wails-client/`, `v4/`). **Read it, never build it.** | — |
| `scripts/` (root) | The Luhn-mod-N code generator and the PDF card printer. | — |

Two traps in the layout itself:

- **Editing `server/` changes nothing until the hub copy is updated.** `setup.sh`
  deploys from a staging copy on the VPS, not from this repo. `server/scripts/hooks-sync.sh`
  does the copy and verifies it; how to check drift by hand is in
  [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) ("If you are the next agent here").
- **There is no root version file and no root bump script.** Those versioned the
  retired Wails client and were deleted. The client has its own version authority
  and its own CI — [`operate/RELEASING.md`](operate/RELEASING.md),
  [`operate/CI-CD.md`](operate/CI-CD.md).

---

## 3. Naming rule — read before "tidying up" a `myvpn` string

The product is **Locus**. Two classes of legacy `myvpn` name survive **on
purpose**, because they exist on deployed machines:

1. **Live server paths and unit names** — `/usr/local/bin/myvpn-backup.sh`,
   `myvpn-tc-apply.sh`, `/etc/myvpn/tc`, `/var/log/myvpn-*.log`. Renaming them in
   the repo desynchronises the definition from the running system, so a fresh
   `setup.sh` would install a file the existing units do not call. **Rename only
   as part of a redeploy, never as a drive-by.**
2. **The client backup directory** `.myvpn-backups/` — on every activated machine's
   disk. Renaming it orphans existing rollback copies.

Everything else — user-visible strings, hook headers, log banners, document text,
the Windows executable's product name — is **Locus** and should be fixed on
sight.

No live file carries the old name any more.

### The hub's old hostname is a third class, and for the same reason

`[hub.domain_legacy]` in [`state.toml`](state.toml) is **not** a leftover to tidy.
Every build before 3.3.2 has it compiled in, and the heartbeat — the only channel
that carries a hub move — needs a working hub to reach. It is therefore **served
from the same Caddy site block, not redirected**: a 308 requires the old name to
present its own certificate first, which is exactly what failed.

**Do not delete it while any earlier build is in the field** — the failure is
permanent and silent, leaving the device at the activation gate unable to update
out of it. `check-consistency.sh` §32 enforces that it keeps being served, and
`contract::LEGACY_HUB_URL` records it beside every other wire name.

---

## 4. The two facts that must not be written down here

An **IP address** and a **count of live codes or customers**. Both are build
failures if written (`check-consistency.sh` §10), both decayed in this very file,
and both are owned by [`operate/CLAIMS.md`](operate/CLAIMS.md) §1 and §4. Resolve
the domain (in [`state.toml`](state.toml)) when you need an address; keep the rule
*never bulk-delete `codes` or `code_events`; revoke with Suspend, move with
Unbind.*

---

## 5. Standing decisions

These are product calls that shape the code. They are recorded here because each
one is repeatedly questioned; where a call has an owner, the owner wins.

- **TUN is always on**, not a mode the student chooses. A system-proxy mode was
  considered and rejected.
- **One hub, one server per tier.** No multi-region plans.
- **Windows and macOS first, Linux last.** Linux is supported last because a
  headless box is the easiest place to *verify* it, not because it matters least.
- **No macOS code-signing or notarisation.** The operator ships a "launch unsigned
  apps" tutorial instead. The updater's shape still has to be right — see
  [`operate/RECOVER-MACOS-UPDATE.md`](operate/RECOVER-MACOS-UPDATE.md).
- **The client versions itself**, on its own line rather than Clash Verge's
  inherited number. The list of version sites is data in
  [`state.toml`](state.toml) `[client.version_sites]` — do not restate the count
  in prose.
- **No identity is stored.** A code is single-use and bound to nothing; the client
  keeps it durably so a reinstall usually restores access. See
  [`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md).

---

## 6. Pitfalls worth knowing before you debug

Each has a full write-up in [`reference/FIXES.md`](reference/FIXES.md); this is
the index so the same afternoon is not spent twice. The method (reproduce →
locate → prove → fix → guard) is
[`reference/DEBUGGING-METHOD.md`](reference/DEBUGGING-METHOD.md); the hub-side
hook traps are owned by
[`operate/POCKETBASE-SETUP.md`](operate/POCKETBASE-SETUP.md) §4.

- **Never re-add `ufw limit 22/tcp`,** and never inject a limiter chain into
  `/etc/ufw/before.rules`. Both locked SSH out of a live box.
- **The live hub is production and there is no sandbox.** Stage on the VPS copy,
  never experiment on the live hooks ([`operate/OPS.md`](operate/OPS.md)).
- **A committed fix is inert until the piece that runs it is redeployed** — the
  hub has its own copy of the scripts and hooks.

---

*Everything cut from this page — duplicate tier and version tables, the retired
client's structure, the per-round VPS bug lists — lives where it is owned:
`archive/`, [`reference/FIXES.md`](reference/FIXES.md) and
[`business/04-tiers.md`](business/04-tiers.md).*
