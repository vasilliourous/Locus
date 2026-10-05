# Locus documentation index

```
audience:    all
status:      live
authoritative-for: the map of the doc set (what each document is, and where it lives)
verified-against: docs/state.toml, docs/operate/CLAIMS.md
```

This is the **single index** for the whole doc set, including `client/docs/`.
Every live document appears below exactly once. If you are looking for something
and it is not here, the index is wrong — fix it in the same change
(`check-consistency.sh` §28 fails the build when a live document is orphaned
from this page).

**How to read anything here:**

1. **Every live document opens with a front-matter block** naming its `audience`,
   `status` (`live`, `reference`, or `design-record`) and `verified-against`.
   Retired documents live in `archive/` / `history/`, where **the folder carries
   the status** and no front-matter block is needed.
2. **Nothing current lives in `archive/` or `history/`.** Those are reference
   material — read them for *why*, never for *what is*.
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

| Document | `status` | What it is |
|---|---|---|
| [`../AGENTS.md`](../AGENTS.md) | live | **Agents read this first.** Repo map, the fact rules, and the release procedure (bump → commit → tag). |
| [`STATE.md`](STATE.md) | live | **Where a fact goes.** The two claim classes and the ownership map — one canonical home per topic. Not a fact table. |
| [`state.toml`](state.toml) | data | The derived facts (version, domain, console path, platform list) as **data**, each with a `source` and `provenance`. Guard-enforced by §9. |
| [`operate/CLAIMS.md`](operate/CLAIMS.md) | live | **Read before writing any fact.** The two claim classes, the decision procedure, and the dated register of world claims (§5). |
| [`CONTEXT.md`](CONTEXT.md) | live | The project in full: what it is, its history, the repository layout, the naming rules, the live-data warning. |
| [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) | live | What is unfinished or unvalidated, and what was deliberately left alone. |

## Operate the hub — `docs/operate/`

| Document | `status` | What it is |
|---|---|---|
| [`operate/DEPLOY.md`](operate/DEPLOY.md) | live | Deploying the hub to a VPS, from scratch or to a new host. |
| [`operate/OPS.md`](operate/OPS.md) | live | Running it: health checks, backups, restore, the things you do at 2am. |
| [`operate/POCKETBASE-SETUP.md`](operate/POCKETBASE-SETUP.md) | live | PocketBase specifics, including the admin console at `/admin/`. |
| [`operate/SECRETS-MANAGEMENT.md`](operate/SECRETS-MANAGEMENT.md) | live | age-encrypted secrets: what is plaintext, what must never be committed. |
| [`operate/UPDATE-SYSTEM.md`](operate/UPDATE-SYSTEM.md) | live | How an update reaches a client, end to end, and what is verified vs not. |
| [`operate/RELEASING.md`](operate/RELEASING.md) | live | **Cutting a client release** (bump → commit → tag → CI) and publishing it to the hub. |
| [`operate/RECOVER-WINDOWS-UPDATE.md`](operate/RECOVER-WINDOWS-UPDATE.md) | live | **When Windows clients cannot update** — the hub serving the raw PE instead of the installer, and the ordered recovery (sign → deploy → publish). |
| [`operate/RECOVER-MACOS-UPDATE.md`](operate/RECOVER-MACOS-UPDATE.md) | live | **When macOS clients cannot update** — the hub serving a bare Mach-O instead of the `.app.tar.gz`, and the ordered recovery. |
| [`operate/CI-CD.md`](operate/CI-CD.md) | live | The client CI pipeline (`.github/workflows/client.yml`). |
| [`../server/scripts/README.md`](../server/scripts/README.md) | live | Index of every script in `server/scripts/` — what it does, and whether it is safe to re-run. |

## Reference — `docs/reference/`

| Document | `status` | What it is |
|---|---|---|
| [`reference/API.md`](reference/API.md) | live | HTTP API reference for the hub (activation, heartbeat, code lookup, releases). |
| [`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md) | live | **How a student keeps their entitlement** across an update, an uninstall or a reset, and why device identity was removed. Read before touching the activation gate. |
| [`reference/DEBUGGING-METHOD.md`](reference/DEBUGGING-METHOD.md) | live | **How to diagnose a defect here** — the reproduce→locate→prove→fix→guard order, two-sided contracts, and checks that cannot fail. Read before fixing anything that "should work". |
| [`reference/EGRESS-READINESS.md`](reference/EGRESS-READINESS.md) | live | **Why the button says "connecting"** — the readiness chain from probe to rendered phase, and how to localise a stall. Read before touching `probe.rs`. |
| [`reference/THEMES.md`](reference/THEMES.md) | live | **The theme registry** — what a theme may and may not change, and the checklist for adding one. Read before touching `_themes.ts`. |
| [`reference/FIXES.md`](reference/FIXES.md) | live | Append-only dated log of every real defect and its fix. **Historical** — later entries sometimes correct earlier ones. |
| [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) | live | What is unfinished or unvalidated. |

## Business — `docs/business/`

The commercial plan: the product as it ships, and how it is run as a business.

| Document | `status` | What it is |
|---|---|---|
| [`business/README.md`](business/README.md) | live | The playbook index — start here. |
| [`business/01-overview.md`](business/01-overview.md) | live | The business in one page. |
| [`business/02-market.md`](business/02-market.md) | live | Who the customer is, and why the incumbents fail them. |
| [`business/03-product.md`](business/03-product.md) | live | What the product is: client, hub, activation code. |
| [`business/04-tiers.md`](business/04-tiers.md) | live | **The tier table** — Free and Full, ports, caps, the free tier's design in full. The owner of the live UoT facts. |
| [`business/05-pricing.md`](business/05-pricing.md) | live | The price ladder, re-derived, with reasoning and sensitivity. |
| [`business/06-unit-economics.md`](business/06-unit-economics.md) | live | Cost base, breakeven, and the one unvalidated estimate. |
| [`business/07-billing.md`](business/07-billing.md) | live | Monthly vs term pass. |
| [`business/08-distribution.md`](business/08-distribution.md) | live | The middleman network. |
| [`business/09-console.md`](business/09-console.md) | live | The operator console — what the business runs on. |
| [`business/10-lifecycle.md`](business/10-lifecycle.md) | live | Expiry, suspension, unbind, audit trail. |
| [`business/11-growth.md`](business/11-growth.md) | live | Growth channels, and the referral verdict. |
| [`business/12-scale-and-ceiling.md`](business/12-scale-and-ceiling.md) | live | How big this can get, and the honest ambition. |
| [`business/13-competition.md`](business/13-competition.md) | live | The moat, and the free-VPN competition. |
| [`business/14-risks.md`](business/14-risks.md) | live | Actionable risks and mitigations. |
| [`business/15-continuity.md`](business/15-continuity.md) | live | What survives what. |
| [`business/16-metrics.md`](business/16-metrics.md) | live | What the console actually reports. |
| [`business/17-not-built.md`](business/17-not-built.md) | live | Legacy claims that are prose only. |
| [`business/18-open-items.md`](business/18-open-items.md) | live | Proposals awaiting a decision. |
| [`business/19-cross-reference.md`](business/19-cross-reference.md) | live | Question → file index. |

### The design record — `docs/business/redesign/`

**Read it for *why*, never as a spec for *what is*.** It is `status: design-record`
throughout, and parts of it were **superseded rather than shipped**: its
device-binding design was **removed** in 2026-10 and now sits in
[`business/redesign/archive/`](business/redesign/archive/), separated by folder
like every other retired material. The live model is a single-use, unbound code
that the client keeps in a machine store —
[`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md).

| Document | `status` | What it is |
|---|---|---|
| [`business/redesign/00-README.md`](business/redesign/00-README.md) | design-record | **Start here.** The entry point: what is built, what was removed, what is not yet true. |
| [`business/redesign/01-identity-without-pii.md`](business/redesign/01-identity-without-pii.md) | design-record | Identity without PII. |
| [`business/redesign/02-term-and-renewal.md`](business/redesign/02-term-and-renewal.md) | design-record | Term model and renewal. |
| [`business/redesign/04-card-and-credential.md`](business/redesign/04-card-and-credential.md) | design-record | The card and the credential. |
| [`business/redesign/05-migration-and-live-data.md`](business/redesign/05-migration-and-live-data.md) | design-record | Migration, and the live data it must not damage. |
| [`business/redesign/06-console-and-operator.md`](business/redesign/06-console-and-operator.md) | design-record | The operator surface. |
| [`business/redesign/07-wire-contract-and-compatibility.md`](business/redesign/07-wire-contract-and-compatibility.md) | design-record | Wire contract and compatibility. |
| [`business/redesign/08-metrics-and-instrumentation.md`](business/redesign/08-metrics-and-instrumentation.md) | design-record | Metrics and instrumentation. |
| [`business/redesign/09-impact-on-the-live-plan.md`](business/redesign/09-impact-on-the-live-plan.md) | design-record | Impact on the live plan. |
| [`business/redesign/10-open-questions.md`](business/redesign/10-open-questions.md) | design-record | Open questions. |
| [`business/redesign/11-phased-implementation.md`](business/redesign/11-phased-implementation.md) | design-record | Phased implementation. |
| [`business/redesign/implementation/00-README.md`](business/redesign/implementation/00-README.md) | design-record | The implementation reference — entry point for the detail below. |
| [`business/redesign/implementation/01-data-model.md`](business/redesign/implementation/01-data-model.md) | design-record | Data model — what was added, and who touches it. |
| [`business/redesign/implementation/02-term-and-renewal.md`](business/redesign/implementation/02-term-and-renewal.md) | design-record | Term and renewal — the arithmetic, exactly. |
| [`business/redesign/implementation/04-operations.md`](business/redesign/implementation/04-operations.md) | design-record | Operations — the job, in order. |
| [`business/redesign/implementation/05-not-yet-true.md`](business/redesign/implementation/05-not-yet-true.md) | design-record | **What is not yet true** — the largest unverified surface in the repo. |
| [`business/redesign/implementation/06-verification.md`](business/redesign/implementation/06-verification.md) | design-record | Verification — the commands, and what each one proves. |
| [`business/redesign/archive/`](business/redesign/archive/) | retired | Device binding: the design that was **removed, not shipped**. Folder carries the status. |

## The client's own documents — `client/docs/`

| Document | `status` | What it is |
|---|---|---|
| [`client/CONTRIBUTING.md`](../client/CONTRIBUTING.md) | live | **How to set up and submit a change to the client** — the human-facing setup and submission guide. |
| [`client/docs/UPSTREAM-CHANGES.md`](../client/docs/UPSTREAM-CHANGES.md) | live | **Authoritative** — how the client differs from Clash Verge Rev, and the load-bearing details that look like mistakes. Read before changing anything in `client/`. |
| [`client/docs/FRONTEND.md`](../client/docs/FRONTEND.md) | live | The shipping two-tab UI, its routing, and the rules each surface obeys. |
| [`client/docs/SIGNING.md`](../client/docs/SIGNING.md) | live | Key custody, rotation, and the release signing pipeline. |
| [`client/docs/IDENTITY-MIGRATION.md`](../client/docs/IDENTITY-MIGRATION.md) | live | The app-id migration and what must NOT be renamed. |
| [`client/docs/CONTRIBUTING_i18n.md`](../client/docs/CONTRIBUTING_i18n.md) | live | Adding or changing a translated string. |
| [`client/docs/LOGIC-INVENTORY.md`](../client/docs/LOGIC-INVENTORY.md) | design-record | The module map — what each `locus/` module owns. Partly a to-do: where it and the code disagree, the code wins. |
| [`client/docs/UPDATE-ARCHITECTURE.md`](../client/docs/UPDATE-ARCHITECTURE.md) | design-record | Why the updater is hub-mediated, and how it differs from Tauri's. Implemented, with two documented corrections. |
| [`client/docs/ARCHITECTURE.md`](../client/docs/ARCHITECTURE.md) | design-record | The pre-port rework plan and its rationale (what to cut from upstream, where Locus logic goes). |
| [`client/docs/RESTRUCTURE.md`](../client/docs/RESTRUCTURE.md) | design-record | The 2026-09 repo layout change. |

## Archived and historical

| Directory | What it is |
|---|---|
| [`archive/`](archive/) | Retired material: the Wails client's architecture and build guide, a completed deployment packet, and the Gaming-UDP implementation plan. **Read for why, never for what is.** Links into it are one-way — nothing in `archive/` is current. |
| [`history/`](history/) | Curated pre-V5 research: business plan, N4L attacker/defender analysis, blocklist research, and dated session records. Includes [`history/LAYOUT.md`](history/LAYOUT.md), the before/after record of every documentation reorganisation. |

---

## How this doc set is organised

`docs/` is split by **what you are trying to do**, and the split is load-bearing:

| Directory | For | Contents |
|---|---|---|
| `docs/` (root) | Everyone | Navigation and rules only — this index, `STATE.md`, `CONTEXT.md`, `state.toml`. No procedure lives here. |
| `docs/operate/` | The operator | Running, deploying, releasing, recovering the hub. |
| `docs/reference/` | The builder | Contracts, internals, and the diagnosis method. |
| `docs/business/` | The operator | The commercial plan. |
| `client/docs/` | The builder | Client internals, kept beside the client. |
| `docs/archive/`, `docs/history/` | Nobody, yet | Retired material. **The folder carries the status.** |

Three rules keep it from fragmenting again, and each is enforced:

1. **One home per topic.** [`STATE.md`](STATE.md) holds the ownership map. When two
   documents disagree, the owner wins and the other is a bug.
2. **Every live document is reachable from this index.** §28 fails the build on an
   orphan — the index is a guard, not a habit.
3. **Status is structure.** Front-matter (§25) for live material; the folder itself
   for anything retired.

---

## Layout history

`docs/` was reorganised twice before this: the 2026-09 overhaul split live
material into `operate/` and `reference/`, replaced status banners with
front-matter, and moved volatile facts into `state.toml` + `CLAIMS.md`; the
2026-09-23 restructure separated `client/`, `server/` and `legacy/`.

The 2026-10-05 pass rebuilt **this page** as the single index (it previously
covered only part of the set, and `STATE.md` carried a competing map), moved the
retired material into `archive/`, and named the misleading titles. The
before/after map is in [`history/LAYOUT.md`](history/LAYOUT.md).
