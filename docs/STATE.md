# STATE — where the live facts are, and why they are not here any more

```
audience:    all
status:      live
authoritative-for: the file-ownership map, and the rule for where a fact goes
data:        docs/state.toml    (derived facts — the machine-readable half)
claims:      operate/CLAIMS.md  (world claims — the dated, checkable half)
```

> **This file used to hold a table of "volatile facts".** It held a version, an
> IP address, host specs and a HEAD commit, and it called itself "the single
> source of truth for facts that change". It was wrong in a specific way that is
> worth understanding, because the same mistake is easy to make again — see
> [`operate/CLAIMS.md`](operate/CLAIMS.md) §1.
>
> The IP in that table went stale three times and finished by naming an address
> that **did not exist**, in the document other documents were told to trust.
> Nothing failed. Nothing noticed. That is not a proofreading problem, and it does
> not get fixed by writing more carefully.

---

## Where a fact goes now

There are two kinds of fact, and they have two homes. Neither of them is here.

| If the fact is… | It is a… | Its home | Who checks it |
|---|---|---|---|
| Recomputable from this checkout (version, domain, console path, platform list) | **Class B — derived** | [`state.toml`](state.toml) | `server/scripts/check-consistency.sh` §9, which fails the build || About the deployed system or the world (hub reachable, codes in use, CI ran, preview is current) | **Class A — world claim** | [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 | `server/scripts/verify-live.sh`, which may only ever mark them verified by reaching them |

**The rule in one line:** if it changes when the box moves, the world moves, or a
day passes, it is dated and checkable — never a present-tense sentence. If a
command can recompute it, it is data and a guard — never a sentence either.

The decision procedure, and the two failure modes to watch for, are in
[`operate/CLAIMS.md`](operate/CLAIMS.md) §3. Read it before adding a fact to any
document.

---

## The two facts that most often get written down anyway

Both of these were in this file as present-tense facts. Neither belongs in prose.
Writing either one into a document is now a build failure (`check-consistency.sh`
§10).

- **An IP address.** The only stable identifier for the deployed system is the
  **domain**, and it is in `state.toml`. The address behind it is a measurement of
  a moment. If you need it, resolve the domain.
- **A count of live codes or customers.** It decays before it is read, and it is
  customer-identifying in a product whose core design decision is to store no
  identity. The **rule** is what survives: *never bulk-delete `codes` or
  `code_events`; revoke with Suspend, move with Unbind.* See
  [`operate/CLAIMS.md`](operate/CLAIMS.md) §4.

---

## Where each topic lives (the ownership map)

One canonical home per topic. Everywhere else links here or to the owner — no
restatements. This map is also the de-duplication index: when two documents
disagree, the owner wins.

| Fact / topic | Canonical home |
|---|---|
| Derived facts — version, domain, console path, platform list | [`state.toml`](state.toml) *(not this file)* |
| World claims — hub up, data live, CI ran, preview current | [`operate/CLAIMS.md`](operate/CLAIMS.md) §5 |
| How to write any fact at all | [`operate/CLAIMS.md`](operate/CLAIMS.md) |
| What the product is; human orientation | `docs/CONTEXT.md` |
| Tiers and pricing | `docs/business/04-tiers.md`, `docs/business/05-pricing.md` |
| HTTP API contracts (hub) | `docs/reference/API.md` |
| Server architecture (live) | `docs/operate/DEPLOY.md`, `docs/operate/OPS.md`, `docs/reference/API.md` |
| Deploy / operate / backups | `docs/operate/DEPLOY.md`, `docs/operate/OPS.md` |
| PocketBase & the console | `docs/operate/POCKETBASE-SETUP.md` |
| Secrets | `docs/operate/SECRETS-MANAGEMENT.md` |
| Update path, end to end | `docs/operate/UPDATE-SYSTEM.md` |
| Publishing a release | `docs/operate/RELEASING.md` |
| Client CI pipeline | `docs/operate/CI-CD.md` |
| Client internals & upstream diff | `client/docs/UPSTREAM-CHANGES.md` (authoritative), `client/docs/FRONTEND.md`, `client/docs/LOGIC-INVENTORY.md` |
| Client release signing | `client/docs/SIGNING.md` |
| Defect log (dated, append-only) | `docs/reference/FIXES.md` |
| What is unfinished | `docs/reference/STILL-OPEN.md` |
| Business plan | `docs/business/` (start at `README.md`) |
| Retired client / pre-V5 research | `docs/archive/`, `docs/history/` |

---

## Rules that keep the doc set honest

1. **The code wins.** Where a document and the code disagree, the code is right
   and the document is a bug — fix it in the same change.
2. **No brittle numbers in prose.** A version, an address, a count and a HEAD
   belong in `state.toml` or in `CLAIMS.md` §5, not inlined in a sentence. Link
   instead; §10 of the consistency check fails the build if you do not.
3. **Status is structure, not a banner.** A document's status is a front-matter
   field (`status: live | reference | design-record`); if it is retired, it lives
   in `archive/` or `history/`, where the folder carries the status. This is
   **enforced**: `check-consistency.sh` §25 fails the build when a live document
   has no front-matter block or an invented `status:` value.
4. **A world claim carries a date and a method.** Present tense is for things that
   cannot change. Everything else is *"as last verified on `<date>`"*, and if it
   has not been verified, it says **unverified** in those words.

---

*Rewritten 2026-09-29 by the documentation-overhaul work that removed the volatile
table: the facts moved to `state.toml` (derived) and `CLAIMS.md` (world), and the
guards that keep them there are `check-consistency.sh` §9 and §10.*
