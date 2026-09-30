# Claims — what this documentation is allowed to assert

```
audience:    all
status:      live
authoritative-for: how to write and verify any claim about the world or about the tree
verified-against: docs/state.toml
```

> **Read this before writing a fact into any document.** This project's
> documentation has a specific, recurring failure: a sentence states a value that
> is true when written and false a month later, phrased in the present tense as
> though it were a property of the code. A later agent reads it, believes it, and
> acts on it. This file exists so that class of sentence is written differently.

---

## 1. The failure this prevents

`docs/CONTEXT.md` told the reader, in the same file that warns agents to trust it:

> **DNS is managed by the hosting provider** — the VPS hostname
> `networkingguides.duckdns.org` resolves to the VPS IP automatically. No DuckDNS
> updates needed.

The *domain* is stable. The IP is not: the hub has moved at least three times
(`114.23.136.59` → `134.199.155.166` → `170.64.196.179`), each time recorded in
`docs/STATE.md` as **"Live hub host"** — a fact, in the present tense, crowned
"authoritative". By 2026-09-29 the last of those addresses **did not exist at
all**, and it was still there in the canonical home, in a code test, and in a
diagnostic script.

Nothing in that story is a logic error. Every document was correct when written.
The world moved. **There is no mechanism in this repository that notices.**

That is the defect class this file governs.

---

## 2. Two classes of claim

Every statement of fact in this repo is one of two kinds, and they must be written
differently because they fail differently.

### Class A — world claims

A claim about the deployed system, the network, or reality outside the tree. It
**cannot be proven by checking out the repository**, no matter how carefully it is
written.

Examples: the hub is reachable; the domain resolves; the database holds codes in
active use; a release is published; CI has actually run; the rolling
`build-preview` installer is current.

**Rules for a Class A claim:**

1. **Never present tense without a date.** Write *"as last verified on
   `<date>`"*, never *"the hub is online"*. The present tense is reserved for
   things that cannot change.
2. **One canonical home.** There is exactly one place each Class A claim lives —
   this file, §5. Everywhere else links here. Two copies of a world claim are two
   things that can disagree, and neither is the truth.
3. **Carry a re-verify command.** A claim nobody can check is a rumour. Each
   entry in §5 names the command that establishes it.
4. **An unverified claim is written as unverified.** If the last check failed or
   has never run, the document says so, in those words. "Probably fine" is not a
   status.
5. **Never identify a customer.** See §4.

### Class B — derived claims

A claim provable from the tree by running something: the client version (the
manifests agree), the console path (the deploy script installs it there), the
domain (it is a constant in `setup.sh`), the cipher (it is defined in one place).

**Rule for a Class B claim: do not write it in prose at all.** It belongs in
[`docs/state.toml`](../state.toml), where `server/scripts/check-consistency.sh` §9
recomputes it and fails the build when it stops agreeing with the code.

This is the same move that already fixed the version number, and it generalises:
**you cannot have drifted from a value you never wrote down.** A Class B fact
written into a sentence is not documentation; it is a future stale sentence.

---

## 3. The decision procedure

Before writing a sentence that contains a fact:

```
Is the value different if the hub has moved, a release has been published,
or a day has passed?
│
├─ yes ──> Class A. Date it, prove it or mark it unverified, and put it
│          in CLAIMS.md §5. Link to it from where you were about to write it.
│
└─ no ───> Can you recompute it from this checkout, with a command?
           │
           ├─ yes ──> Class B. It goes in docs/state.toml, not in a sentence.
           │          If a checker does not yet cover it, add one.
           │
           └─ no ───> Then it is not a fact about this project. Either it is
                      Class A after all, or it is opinion and should read
                      like opinion.
```

The two failure modes to watch for, because both have happened here:

- **A travelling value written as a property.** An IP address, a customer count,
  a byte size, a test count. These are measurements of a moment dressed as
  attributes of a system. The IP case is documented in §1; `514 tests passed` in a
  live reference is the same error.
- **A constraint of a dead thing read as a constraint of the live one.** An
  archived client's limitation, a retired host's config, a removed module's
  behaviour. Always ask *which* system the sentence is about before believing it.

---

## 4. Customer data never enters the repository

**Decision (2026-09-29): the hub's live customer data is described by rule, never
by value.**

Earlier revisions recorded *"11 real `strike` activation codes for middleman
'Wanzhen', one already bound"*. It was removed for two independent reasons, either
of which is sufficient:

1. **It is a Class A claim that decays in a day.** The count is wrong the next time
   a code is sold. It was already wrong when read.
2. **It is customer-identifying.** This product's core design decision is that the
   hub stores no identity (`docs/business/redesign/01-identity-without-pii.md`).
   A distributor's name in a versioned, pushed repository contradicts that decision
   in the one place it is cheapest to avoid.

What replaces it, and what must be preserved, is the **operative rule**, which does
not decay:

> The hub holds paid codes in active use. **Never bulk-delete `codes` or
> `code_events` rows.** Revoke access with **Suspend** (reversible). Move a student
> to a new device with **Unbind**. Re-verify the current state before any schema
> change.

That form carries every bit of the warning and none of the rot. If you need the
live figure for an operational decision, read it from the console — do not write
it here.

---

## 5. The register of Class A claims

Every one of these is an assertion **by a person, about the world**. The date is the
last time it was actually checked, and `verify-live.sh` is the only thing permitted
to advance it. **If the check cannot reach the network, the date does not move and
the claim is not verified.**

| # | Claim | Last verified | How to re-verify |
|---|---|---|---|
| A1 | The hub endpoint `https://networkingguides.duckdns.org` resolves and answers `/api/health` | **unverified** | `server/scripts/verify-live.sh` |
| A2 | The hub holds paid codes in active use (count deliberately not recorded — §4) | **unverified** | sign in to the console at `/admin/`, Codes & Clients |
| A3 | The rolling `build-preview` release is current for the pushed commit | **unverified** | `gh release view build-preview --json tagName,assets,createdAt` |
| A4 | The client CI workflow has actually run and gone green for a given commit | **unverified** | `gh run list --workflow=client.yml` |
| A5 | A published release is installable by an existing client end to end | **unverified** | needs real Windows/macOS hardware — see `../reference/STILL-OPEN.md` |
| A6 | The **Windows** NSIS installer completes on a clean machine (setup runs, WebView2 checks pass, service registers) | **unverified** | run the staged `installer-locus_*_x64-setup.exe` on real Windows; nothing in CI executes it — see `../reference/FIXES.md`, 2026-09-30 |
| A7 | The installed Windows client **opens its window** on a machine that carries a `verge.yaml` written by an earlier build | **unverified** | install on Windows with a pre-existing `%APPDATA%\com.locus.client\verge.yaml` containing a stale `start_page`; the code path is unit-tested, the real config is not |

### The honest limit of this file

**This file does not make any Class A claim true.** It makes them dated,
single-homed, and checkable, and it stops them being laundered into derived
sentences that read as facts about the code. A1–A5 above are **not verified**. When
this documentation overhaul was performed (2026-09-29) the environment had no route
to the hub: the previous address no longer existed and no credentials were
available. That is itself an example of the rule — the honest state is written
down, not assumed away.

---

## 6. Where the stable identifiers live

The **only** stable identifiers for the deployed system are:

| Thing | Stable identifier | Why it is stable |
|---|---|---|
| The hub | `networkingguides.duckdns.org` | A DNS name the operator controls; survives the box moving |
| The console | `https://networkingguides.duckdns.org/admin/` | Path within the above |
| The API | `https://networkingguides.duckdns.org` | Root of the above |

**An IP address is never one of them.** It is a Class A claim about a moment, and
it belongs in this file's register or nowhere. Writing one into a document, a test
fixture or a script is the exact defect §1 describes.

---

*Added 2026-09-29. When you change a rule here, that is a Class B change to this
file: it is enforced by `server/scripts/check-consistency.sh` §9 and §10.*
