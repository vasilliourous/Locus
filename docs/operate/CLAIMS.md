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
| A1 | The hub endpoint `https://networkingguides.duckdns.org` resolves and answers `/api/health` | **verified 2026-10-05** | `server/scripts/verify-live.sh` — ran 2026-10-04T22:45Z (= 2026-10-05 NZ, the register's dateline), `A1 PASS`, `/api/health` returned 200 (`{"message":"API is healthy.","code":200}`) |
| A2 | The hub holds paid codes in active use (count deliberately not recorded — §4) | **unverified** | sign in to the console at `/admin/`, Codes & Clients |
| A3 | The rolling `build-preview` release is current for the pushed commit | **unverified** | `gh release view build-preview --json tagName,assets,createdAt` |
| A4 | The client CI workflow has run and gone green for a given commit | **verified 2026-09-30** for `5ebfc89` | run `36669449010` — `Verify`, all four builds incl. `windows-2022`, `Release`, `Report` — read from the GitHub REST API. Re-check with `gh run list --workflow=client.yml` |
| A5 | A published release is installable by an existing client end to end | **unverified** | needs real Windows/macOS hardware — see `../reference/STILL-OPEN.md` |
| A6 | The **Windows** NSIS installer completes on a clean machine — the setup runs interactively and installs Locus | **verified 2026-09-30** for the `v3.2.12` installer (`installer-Locus_3.2.12_x64-setup.exe`) | opened the NSIS setup on real Windows and completed the install; Locus installed. Re-check by running the setup on a clean machine and confirming the install finishes |
| A7 | The **installed Windows client stays installed** — it launches from `C:\Program Files\Locus\`, serves its own bundled assets, and is not relaunched out of its install directory by its own updater | **unverified** — and was **falsified in the field** on 2026-09-30 (see the note) | install on Windows, let it take an advertised update, then launch from the Start Menu and confirm the window renders the app. Nothing in CI launches either artifact, which is why three green releases shipped this broken — see `../reference/FIXES.md`, 2026-09-30 (second entry) |
| A8 | The **shipped client resolves its own bundled assets at run time** — a build carries no CI-runner path, so an installed app on a machine without the runner's drive renders the app rather than `ERR_FILE_NOT_FOUND` | **verified 2026-09-30** for `a8b0acf` (CI run `36686775049`) | the built `locus-windows-amd64.exe` was read back from the run's artifacts and the Tauri context string now carries the relative `../dist` where the broken `5ebfc89` build carried `d:/a/Locus/Locus/client/dist`. Re-check with `strings locus-windows-amd64.exe \| grep -c 'a/Locus/Locus'` (expect `0`) |
| A9 | The **macOS** update slot serves an installable `.app.tar.gz` — a macOS client is offered a payload its updater can apply | **unverified** — and was **falsified in the field** on 2026-10-05 for `3.2.26` (see the note) | `curl -s "https://networkingguides.duckdns.org/api/update?version=<prev>&platform=macos_arm"` must return `"url": …/locus-darwin-arm64.app.tar.gz`; and `curl -s …/updates/<v>/locus-darwin-arm64.app.tar.gz \| head -c2 \| od -An -tx1` must print `1f 8b` (gzip), not the Mach-O magic `cf fa ed fe`. Recovery: `RECOVER-MACOS-UPDATE.md` |
| A10 | **No live code carries `tier: "stealth"`** — the retired tier's row can be deleted without stranding a paying student | **verified 2026-10-05** | console → Codes & Clients → filter by tier; or `GET /api/collections/codes/records?filter=(tier='stealth')&perPage=1` with an admin token and read `totalItems`. A non-zero count means the disposition is "re-point, do not delete" — see `../business/04-tiers.md` §4.6. **Never** bulk-delete the rows either way. |
| A11 | The **live hub took the Free/Full merge** — port 8445 is capped at 100 Mbit, port 8444 is no longer shaped, and the retired service is stopped | **verified 2026-10-05** | read-only: `tc class show dev "$(ip -4 route show default \| awk '{print $5}' \| head -1)"` must show a `1:30` class at 100Mbit and **no** `1:20` class; `systemctl is-active shadowsocks-eco shadowsocks-strike` must be `active` and `systemctl is-enabled shadowsocks-stealth` must not report `enabled`. The tree is green either way — this is the box, not the repo. |
| A13 | **Free clients carry UDP** — the free plan's UoT listener answers on 8447 and the free tier rows advertise `uot_port` | **verified 2026-10-05** | after a `setup.sh` re-run: `ssh <hub> "systemctl is-active sing-box-uot-eco sing-box-uot-strike"` must both be `active`, `ufw status \| grep 8447` must show tcp+udp allowed, and `tc class show dev <iface>` must show a `1:20` class at 1mbit. Then activate a throwaway `free` code and confirm its heartbeat returns `server_config.uot_port: 8447` with `udp_relay: true`. The mechanism is a second instance of the paid path (which works), but "the second instance works" is not implied by "the first does". |
| A12 | A **paid code connects on the merged tier** — activation resolves `strike` and the heartbeat returns a `server_config` on 8445 with `udp_relay` true | **verified 2026-10-05** | activate a throwaway `strike` code (never a live student's) and inspect the `/api/activate` and `/api/heartbeat` responses; `server_config.server_port` must be 8445 and `udp_relay` must be `true`. This is the merge's central claim and it is the one thing that cannot be proven from the checkout. |

> **A10–A13 were verified on 2026-10-05 by the deploy that shipped them**, with
> the caveat each row records:
>
> - **A10** — `SELECT tier, COUNT(*) FROM codes GROUP BY tier` returned a single
>   row: `strike`, 8 codes (3 activated). **No `stealth` code has ever been
>   minted**, so the retired tier stranded nobody and its service was removed.
> - **A11** — the live tc state is `1:10 @ 1Mbit`, `1:20 @ 1Mbit`, `1:30 @ 100Mbit`
>   with three matching filters and **no 8444 filter**; `shadowsocks-stealth
>   .service` and `/etc/shadowsocks/stealth.json` were deleted and 8444 stopped
>   listening.
> - **A13** — `sing-box-uot-strike` (8446) and `sing-box-uot-eco` (8447) are both
>   `active` and `enabled`, both listening on TCP+UDP, both reachable from
>   outside this network, and both tier rows advertise their own `uot_port`.
> - **A12** — verified by activating **one throwaway code per plan** through the
>   *public* `POST /api/activate`, exactly as a student's client does, and reading
>   the response back:
>
>   | Plan | status | tier | TCP | `udp_relay` | `uot_port` |
>   |---|---|---|---|---|---|
>   | Free | Activation successful | `free` | 8443 | `True` | **8447** |
>   | Full | Activation successful | `strike` | 8445 | `True` | **8446** |
>
>   Both codes were minted for the test and deleted afterwards (never a live
>   code, and never a bulk delete — `CLAIMS.md` §4). This exercises the whole
>   chain: code → checksum → lookup → activate → tier-config resolution →
>   per-plan `uot_port`.
>
>   **What it still does not prove:** that a client *tunnels real game traffic* at
>   the shaped rate. The response is the hub's promise; no machine has yet played
>   through it. That observation needs real hardware
>   ([`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md)).
>
> **Two defects were found and fixed by this deploy, both invisible in the tree:**
>
> 1. **The UoT units were created but never started.** `systemctl enable` acts at
>    the next boot, and rewriting a unit file does not restart it — so a module
>    run on a live box left *both* plans with no UDP at all, while every check
>    said "created". Both listeners were started by hand, and
>    `02-shadowsocks.sh` now starts and asserts them.
> 2. **`04-tc.sh`'s own verification was a false negative.** It grepped
>    `tc filter show` for `sport <port>`, but a u32 filter is printed as a hex
>    match (`match 20fb0000/ffff0000`), so it warned "filter not found" for all
>    three ports on a correctly shaped interface. Now decoded and asserted
>    against the intended class, with the retired 8444 port checked as a
>    negative.

> **A7 is no longer merely unverified — it was observed FAILING on 2026-09-30, and
> the failure was a different one from the two already fixed.**
>
> A Windows client on `3.2.13` was offered `3.2.14`: the download reached 100%
> and then the client refused it with *"the update to 3.2.14 is not an installer
> (51741696 bytes); refusing to execute it."* That byte count is exactly
> `locus-windows-amd64.exe` — the raw PE, not the NSIS installer.
>
> The client's guard was **correct** and did its job: it stopped an unvetted PE
> from being ShellExecuted, which is precisely the 3.2.12 fix. The defect was
> upstream of it. CI had correctly advertised
> `installer-Locus_3.2.14_x64-setup.exe` in `manifest.json`, but the hub's
> `fetch-release.py` still resolved the raw binary for the `windows` slot and
> served it with the *raw binary's* signature. So the client was offered the
> wrong artifact at every version, and **no Windows client could update at all**.
>
> Three separate checks passed throughout: CI's manifest guard (§15, which greps
> the workflow, not the hub), the fetch service's all-or-nothing check (all four
> *wrong* files were present), and the manifest hash cross-check — which hashed
> whichever file the *manifest* named, so it agreed with itself by construction.
> The lesson is the one `docs/README.md` rule 3 states for prose, applied to
> guards: **a check that cannot distinguish a right answer from a wrong one reads
> exactly like a check that passed.**
>
> Fixed on the hub side (`fetch-release.py`, `publish-release.sh` now resolve the
> installer and verify the filename against `manifest.json`; the fetch service
> also refuses a Windows PE that carries no NSIS header), with a new guard in
> `check-consistency.sh` §1a pinning the name on both sides, plus a
> `smoke-publish.sh` case shown to fail against the pre-fix code. **A7 itself
> remains unverified**: a real Windows machine has still never taken an update
> and relaunched from the Start Menu. That is the test, and it cannot be run from
> a Linux CI box.

> **A7 was previously written to claim the opposite thing** — that a stale
> `start_page` in `verge.yaml` produced a blank window. That was a misdiagnosis,
> withdrawn on 2026-09-30: the reporting user's config carried `start_page: /`,
> so the described code path could not fire. The claim is retained at this number
> because the *unverified property* it names is real and load-bearing; only the
> proposed cause was wrong. `docs/README.md` rule 3 — the code wins, and the doc
> was the bug.

> **A8 is the third attempt at one symptom, and the report that exposed it.**
> The same `ERR_FILE_NOT_FOUND` was attributed first to a stale `start_page`
> (withdrawn) and then to the updater re-executing the client's own binary (real,
> fixed in `v3.2.12`, but not this report). This report differs on two facts that
> rule both out: the installer was **pulled from the release page and run fresh**
> (no update involved), and `verge.yaml` carried `start_page: /` (so no config
> path could fire). What remains is the log line `url=file:///D:/` — the Windows
> CI workspace `D:\a\Locus\Locus`, compiled into the binary by an absolute
> `frontendDist` override added by the "better workflow" speed change. Normalizing
> that path's separators (`17ef21a`) made CI green without removing the defect.

> **A8 is now VERIFIED, and the way it was verified is the point.** The earlier
> entry here read "do not read A8 as verified because the guard passes — the guard
> is on the tree, and the artifact is the thing that shipped broken." That warning
> was correct, and the distinction it drew is exactly what the verification had to
> cross: a **green run proves nothing** (four consecutive green runs preceded this
> bug). So verification was done on the **built artifact**, not the run colour —
> the raw `locus-windows-amd64.exe` was downloaded from run `36686775049` and the
> Tauri context string inside it was read directly:
>
> ```text
> broken  (5ebfc89, v3.2.12):  ...localhost:3000/ d:/a/Locus/Locus/client/dist icons/128x128.png...
> fixed   (a8b0acf, run #17):  ...localhost:3000/ ../dist                        icons/32x32.png...
> ```
>
> The defect string is **present** in the broken binary and **absent** (`grep -c`
> returns `0`) in the fixed one, which carries the committed relative `../dist`
> instead. This is the class-level check the project kept saying it lacked: the
> build's own output was inspected, so a defect that only exists *after* "the
> bundle compiled" is now visible. See `FIXES.md` for the full entry.

> **Green still proves nothing on its own.** A8 was verified by reading the
> *artifact*; A4's green run and A6/A7 below it were not advanced by it. Do not
> generalise one artifact check into "the pipeline is trustworthy" — the next
> platform-specific leak (a macOS path, a Linux path) would need its own read-back.

> **A6 is now VERIFIED — the installer is confirmed to install.** The
> `installer-Locus_3.2.12_x64-setup.exe` was run interactively on real Windows and
> completed, installing Locus. This is the first Windows install this project has
> ever confirmed; A6 had been carried as unverified since 2026-09-30 because
> nothing in CI executes an installer. Note what it does **not** settle: the
> install completing is the installer's job, and A7 (the installed app surviving an
> update) and A8's interactive end — the window rendering after a real install —
> are separate properties. The artifact read-back covers A8's side; A7 remains
> unverified.

> **A6 was NOT advanced by A4 going green, and that distinction still stands.**
> The `v3.2.12` run is green and its published `manifest.json` names
> `installer-Locus_3.2.12_x64-setup.exe` rather than the raw executable — but
> neither proves an installer *installs*. It took an actual interactive run on
> Windows to move A6, which is exactly why the row sat unverified while three
> preceding green runs shipped unusable releases. Do not read the ✓ beside A4 as
> progress on any row below it.

### The honest limit of this file

**This file does not make any Class A claim true.** It makes them dated,
single-homed, and checkable, and it stops them being laundered into derived
sentences that read as facts about the code. A5 and A7 above are **not verified**;
A4, A6 and A8 each carry a single dated verification of one artifact, and A1 a
single dated reachability check. Nothing more.

**Reachability is itself a dated observation, and it is recorded as one.** When
this documentation overhaul was performed (2026-09-29) the environment had no
route to the hub: the previous address no longer existed and no credentials were
available, and every world claim was written `unverified` accordingly. On
2026-10-05 the environment *could* reach the hub (`verify-live.sh` → `A1 PASS`),
so A1's date moved and nothing else did — A3/A4 stayed `SKIP` because `gh` is not
installed, and a `SKIP` is never a pass. Both states are the rule working: the
honest current state is written down, not assumed in either direction.

**A verification is a dated observation of one run, never a property.** A4 says
"verified for `5ebfc89`", not "the workflow is green"; A6 says "verified for the
`v3.2.12` installer", not "the installer works"; A8 says "verified for `a8b0acf`",
not "the client resolves its assets". The next release invalidates each, and a
reader who needs the current state must re-run the check in the right-hand column.
This is the distinction the file exists to preserve; a "verified" cell without an
artifact or commit attached is the drift this table is here to prevent.

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
