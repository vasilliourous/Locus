# Locus documentation index

```
audience:    all
status:      live
authoritative-for: the map of the doc set (what each document is)
verified-against: docs/STATE.md
```

`docs/` holds the **live** documentation, organised by what you are trying to do.
Retired material is separated into [`archive/`](archive/) (the Wails client) and
[`history/`](history/) (pre-V5 research), so what remains is current.

**How to read anything here:**

1. **Every live document opens with a front-matter block** naming its `audience`
   and `status` (`live`, `reference`, or `design-record`). That replaces the old
   prose status banners. Retired documents live in `archive/`/`history/`, where
   the folder carries the status.
2. **Nothing current lives in `archive/` or `history/`.** Those are reference
   material.
3. **The code wins.** Where a document and the code disagree, the code is right
   and the document is a bug — fix it in the same change.
4. **Know which kind of fact you are writing.** Read
   [`operate/CLAIMS.md`](operate/CLAIMS.md) before adding a fact to any document.
   In one line: a fact that can be recomputed from this checkout is **data** and
   belongs in [`state.toml`](state.toml) with a guard behind it; a fact about the
   deployed system is a **world claim** and belongs in
   [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 with a date and a re-verify command.
   **Neither belongs in a sentence.**
5. **Never write an IP address.** The hub's only stable identifier is its
   **domain** (`state.toml`). Addresses have gone stale three times here, and the
   last recorded one outlived its server. `server/scripts/check-consistency.sh`
   §10 fails the build if one appears in a live document.
6. **Never write a count of live codes or customers.** It decays before it is read,
   and it identifies a customer — see [`operate/CLAIMS.md`](operate/CLAIMS.md) §4.
   The rule that survives (*never bulk-delete; Suspend/Unbind*) is what you keep.

---

## Start here

| Document | What it is |
|---|---|
| [`../AGENTS.md`](../AGENTS.md) | **Agents read this first.** Repo map, the fact rules, and the release procedure (bump → commit → tag). |
| [`STATE.md`](STATE.md) | Where a fact goes, and the ownership map. **Not** a fact table — derived facts are in [`state.toml`](state.toml), world claims in [`operate/CLAIMS.md`](operate/CLAIMS.md). |
| [`state.toml`](state.toml) | The derived facts (version, domain, console path, platform list) as **data**, with a `source` and `provenance` each. Guard-enforced by `check-consistency.sh` §9. |
| [`operate/CLAIMS.md`](operate/CLAIMS.md) | **Read before writing any fact.** The two claim classes, the decision procedure, and the dated register of world claims (§5). |
| [`CONTEXT.md`](CONTEXT.md) | The project in full: what it is, its history, the repository layout, the naming rules, the live-data warning. |
| [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) | What is unfinished or unvalidated, and what was deliberately left alone. |

## Operate the hub — `docs/operate/`

| Document | What it is |
|---|---|
| [`operate/DEPLOY.md`](operate/DEPLOY.md) | Deploying the hub to a VPS, from scratch or to a new host. |
| [`operate/OPS.md`](operate/OPS.md) | Running it: health checks, backups, restore, the things you do at 2am. |
| [`operate/POCKETBASE-SETUP.md`](operate/POCKETBASE-SETUP.md) | PocketBase specifics, including the admin console at `/admin/`. |
| [`operate/SECRETS-MANAGEMENT.md`](operate/SECRETS-MANAGEMENT.md) | age-encrypted secrets: what is plaintext, what must never be committed. |
| [`operate/UPDATE-SYSTEM.md`](operate/UPDATE-SYSTEM.md) | How an update reaches a client, end to end, and what is verified vs not. |
| [`operate/RELEASING.md`](operate/RELEASING.md) | **Cutting a client release** (bump → commit → tag → CI) and publishing it to the hub. |
| [`operate/RECOVER-WINDOWS-UPDATE.md`](operate/RECOVER-WINDOWS-UPDATE.md) | **When Windows clients cannot update** — the hub serving the raw PE instead of the installer, and the ordered recovery (sign → deploy → publish). |
| [`operate/CI-CD.md`](operate/CI-CD.md) | The client CI pipeline (`.github/workflows/client.yml`). |
| [`../server/scripts/README.md`](../server/scripts/README.md) | Index of every script in `server/scripts/` — what it does, and whether it is safe to re-run. |

## Reference — `docs/reference/`

| Document | What it is |
|---|---|
| [`reference/API.md`](reference/API.md) | HTTP API reference for the hub (activation, heartbeat, code lookup, releases). |
| [`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md) | **How a student keeps their entitlement** across an update, an uninstall or a reset, and why device identity was removed. Read before touching the activation gate or `store::read`/`clear`. |
| [`reference/DEBUGGING-METHOD.md`](reference/DEBUGGING-METHOD.md) | **How to diagnose a defect here** — the reproduce→locate→prove→fix→guard order, two-sided contracts, checks that cannot fail, and what to verify on the hub. Read before fixing anything that "should work". |
| [`reference/EGRESS-READINESS.md`](reference/EGRESS-READINESS.md) | **Why the button says "connecting"** — the readiness chain from probe to rendered phase, the classifier trap, and a four-command procedure to localise a stall. Read before touching `probe.rs`. |
| [`reference/THEMES.md`](reference/THEMES.md) | **The theme registry** — what a theme may and may not change, the six themes, how a selection resolves against the legacy colour fields, and the checklist for adding one. Read before touching `_themes.ts`. |
| [`reference/FIXES.md`](reference/FIXES.md) | Append-only dated log of every real defect and its fix. **Historical** — later entries sometimes correct earlier ones. |
| [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) | What is unfinished or unvalidated. |
| [`GAMING-UDP.md`](GAMING-UDP.md) | The UoT (UDP-over-TCP) work for the gaming tier. Server side is live. |

## Business — `docs/business/`

| Document | What it is |
|---|---|
| [`business/README.md`](business/README.md) | The commercial plan (operator playbook) — start here. |
| [`business/redesign/00-README.md`](business/redesign/00-README.md) | The design rationale for terms, renewal and device binding. |

## The client's own documents — `client/docs/`

| Document | What it is |
|---|---|
| [`client/docs/UPSTREAM-CHANGES.md`](../client/docs/UPSTREAM-CHANGES.md) | **Authoritative** — how the client differs from Clash Verge Rev. |
| [`client/docs/FRONTEND.md`](../client/docs/FRONTEND.md) | The shipping two-tab UI. |
| [`client/docs/LOGIC-INVENTORY.md`](../client/docs/LOGIC-INVENTORY.md) | The module map — what each `locus/` module owns. |
| [`client/docs/UPDATE-ARCHITECTURE.md`](../client/docs/UPDATE-ARCHITECTURE.md) | Why the updater is hub-mediated. |
| [`client/docs/SIGNING.md`](../client/docs/SIGNING.md) | Key custody and release signing. |
| [`client/docs/IDENTITY-MIGRATION.md`](../client/docs/IDENTITY-MIGRATION.md) | The app-id migration and what must NOT be renamed. |
| [`client/docs/ARCHITECTURE.md`](../client/docs/ARCHITECTURE.md) | The pre-port plan and its rationale (design record). |
| [`client/docs/RESTRUCTURE.md`](../client/docs/RESTRUCTURE.md) | The 2026-09 layout change (design record). |

## Archived and historical

| Directory | What it is |
|---|---|
| [`archive/`](archive/) | The retired Wails client: architecture, build guide, backend API, migration history, and the mixed-era analyses that describe it. |
| [`history/`](history/) | Curated pre-V5 research: business plan, N4L attacker/defender analysis, blocklist research, and dated session records. |

---

## Layout history

`docs/` was reorganised in the 2026-09 overhaul: live material split into
`operate/` and `reference/`, status banners replaced by front-matter, and
volatile facts moved to `STATE.md`. The before/after map, and the earlier
2026-09-23 restructure, are recorded in
[`history/LAYOUT.md`](history/LAYOUT.md).
