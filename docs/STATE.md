# STATE — where a fact goes, and why it is not written here

```
audience:    all
status:      live
authoritative-for: the rule for where a fact goes, and the ownership map (one canonical home per topic)
data:        docs/state.toml    (derived facts — the machine-readable half)
claims:      operate/CLAIMS.md  (world claims — the dated, checkable half)
```

> **This file is about *where a fact goes*, not what the fact is.** The map of the
> documents themselves is [`README.md`](README.md). The rule exists because facts
> written down in prose go stale silently — nothing fails when a sentence is
> wrong. [`operate/CLAIMS.md`](operate/CLAIMS.md) §1 is the incident record.

---

## Where a fact goes

There are two kinds of fact, and they have two homes. Neither of them is here.

| If the fact is… | It is a… | Its home | Who checks it |
|---|---|---|---|
| Recomputable from this checkout (version, domain, console path, platform list) | **Class B — derived** | [`state.toml`](state.toml) | `server/scripts/check-consistency.sh` §9, which fails the build |
| About the deployed system or the world (hub reachable, codes in use, CI ran, preview is current) | **Class A — world claim** | [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 | `server/scripts/verify-live.sh`, which may only ever mark them verified by reaching them |

**The rule in one line:** if it changes when the box moves, the world moves, or a
day passes, it is dated and checkable — never a present-tense sentence. If a
command can recompute it, it is data and a guard — never a sentence either.

The decision procedure is [`operate/CLAIMS.md`](operate/CLAIMS.md) §3; the two
claims that most often get written down anyway — an **IP address** and a **count
of live codes or customers** — are §1 and §4 there, and writing either one is now
a build failure (§10). The rule that survives: *never bulk-delete `codes` or
`code_events`; revoke with Suspend, move with Unbind.*

---

## The ownership map

**One canonical home per topic.** Everywhere else links to the owner — no
restatements. When two documents disagree, the owner wins and the other is a bug.
This is the de-duplication index: if you are about to write a fact down twice,
find its owner here first.

| Fact / topic | Canonical home |
|---|---|
| Derived facts — version, domain, console path, platform list | [`state.toml`](state.toml) |
| World claims — hub up, data live, CI ran, preview current | [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 |
| How to write any fact at all | [`operate/CLAIMS.md`](operate/CLAIMS.md) |
| The map of the documents themselves | [`README.md`](README.md) |
| What the product is; glossary, layout, naming and live-data rules | [`CONTEXT.md`](CONTEXT.md) |
| Tiers, ports, caps, the live UoT facts | [`business/04-tiers.md`](business/04-tiers.md) |
| Pricing and unit economics | [`business/05-pricing.md`](business/05-pricing.md), [`business/06-unit-economics.md`](business/06-unit-economics.md) |
| HTTP API contracts (hub) | [`reference/API.md`](reference/API.md) |
| Deploy / operate / backups | [`operate/DEPLOY.md`](operate/DEPLOY.md), [`operate/OPS.md`](operate/OPS.md) |
| PocketBase & the console | [`operate/POCKETBASE-SETUP.md`](operate/POCKETBASE-SETUP.md) |
| Secrets | [`operate/SECRETS-MANAGEMENT.md`](operate/SECRETS-MANAGEMENT.md) |
| Update path, end to end | [`operate/UPDATE-SYSTEM.md`](operate/UPDATE-SYSTEM.md) |
| Publishing a release | [`operate/RELEASING.md`](operate/RELEASING.md) |
| Recovering the signing key / CI secret / a stranded fleet | [`operate/RELEASE-RECOVERY.md`](operate/RELEASE-RECOVERY.md) |
| Client CI pipeline | [`operate/CI-CD.md`](operate/CI-CD.md) |
| The hub's two hostnames, and why the old one is still served | [`CONTEXT.md`](CONTEXT.md) §3 (naming rule); values in [`state.toml`](state.toml) `[hub.domain]` / `[hub.domain_legacy]` |
| How to diagnose a defect | [`reference/DEBUGGING-METHOD.md`](reference/DEBUGGING-METHOD.md) |
| The readiness chain ("stuck on connecting") | [`reference/EGRESS-READINESS.md`](reference/EGRESS-READINESS.md) |
| How a student keeps their entitlement | [`reference/DEVICE-IDENTITY.md`](reference/DEVICE-IDENTITY.md) |
| Client internals & upstream diff | [`client/docs/UPSTREAM-CHANGES.md`](../client/docs/UPSTREAM-CHANGES.md) (authoritative), [`client/docs/FRONTEND.md`](../client/docs/FRONTEND.md), [`client/docs/LOGIC-INVENTORY.md`](../client/docs/LOGIC-INVENTORY.md) |
| Client release signing | [`client/docs/SIGNING.md`](../client/docs/SIGNING.md) |
| Defect log (dated, append-only) | [`reference/FIXES.md`](reference/FIXES.md) |
| What is unfinished | [`reference/STILL-OPEN.md`](reference/STILL-OPEN.md) |
| The state of the doc set itself (dated audit) | [`reference/DOC-AUDIT.md`](reference/DOC-AUDIT.md) |
| Business plan | [`business/`](business/README.md) (start at `README.md`) |
| Retired client / pre-V5 research | [`archive/`](archive/), [`history/`](history/) |

---

## Rules that keep the doc set honest

1. **The code wins.** Where a document and the code disagree, the code is right
   and the document is a bug — fix it in the same change.
2. **No brittle numbers in prose.** A version, an address, a count or a HEAD goes
   in `state.toml` or `CLAIMS.md` §5, never inline in a sentence; §10 fails the
   build if you try.
3. **Status is structure, not a banner** — front-matter for live material, the
   folder for retired material (§25).
4. **One home per topic** — the table above is the index of owners.
5. **Every live document is reachable from the index** (§28).
6. **A world claim carries a date and a method.** Present tense is for things that
   cannot change; everything else is *"as last verified on `<date>`"*, or says
   **unverified** in those words.

*History: until 2026-09-29 this file was a fact table; it now answers only "where
does a fact go". [`README.md`](README.md) is the single index.*
