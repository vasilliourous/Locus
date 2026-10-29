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

The domain is stable; the address behind it is not. An earlier `docs/STATE.md`
carried a present-tense "Live hub host" IP as the authoritative fact, and by
2026-09-29 the address it named **did not exist** — still asserted there, in a
code test and in a diagnostic script.

Nothing in that story is a logic error. Every document was correct when written.
The world moved, and **nothing in this repository notices.** That is the defect
class this file governs; the dated record is `../reference/FIXES.md`.

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
5. **Never identify a customer, and never write a count of them.** The rule is
   §4; the one-line version is *never bulk-delete `codes` or `code_events`*.

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

> ### ⚠️ Every A-claim below is currently **falsified**, not merely unverified.
>
> **Recorded 2026-10-18.** The hub was taken down and is being redeployed from
> scratch with B2-backed backups. That does not make the entries below *stale* —
> stale means "true when written, may have moved". It makes them **false about the
> world**: A1 named a reachable endpoint and there is no endpoint; A10–A13 named
> observations on a specific box that no longer exists.
>
> **The dates and values below are kept deliberately, and must not be deleted or
> "refreshed".** A claim's history is the evidence that it was once true and how;
> erasing it is how a register becomes a wish list. What changes is the reading:
> **treat every row as unverified until `verify-live.sh` re-establishes it on the
> new box.** Do not advance a date during the redeploy — the redeploy is the event
> that *invalidates* these, not the event that re-verifies them.
>
> **Two claims are invalidated by 3.3.0 regardless of the redeploy**, and must be
> re-argued rather than re-run:
>
> - **A5** (a published release is installable end to end) — 3.3.0 rotated the
>   update signing key. Any client still carrying the previous key series cannot
>   verify a 3.3.0 payload, so those devices need a manual reinstall; whether a
>   *reinstalled* client then updates is a new question with a new answer, not a
>   re-check of the old one. [`RELEASE-RECOVERY.md`](RELEASE-RECOVERY.md) §4.
> - **A3** (the rolling `build-preview` release is current) — that release and its
>   136 assets were **deleted on 2026-10-18** to cut storage. The claim's subject
>   no longer exists. Either the release is recreated as part of the redeploy and
>   the claim is rewritten against the new one, or the claim is retired. It must
>   not sit here as "unverified" implying a check would settle it.
>
> **A4, A6, A7, A8 are unaffected by the redeploy** — they are observations about
> CI and about artifacts on real hardware, not about the hub. Their dates stand at
> what they measured (`5ebfc89`, `v3.2.12`, `a8b0acf`); they are stale, not false.

| # | Claim | Last verified | How to re-verify |
|---|---|---|---|
| A1 | The hub endpoint `https://networkingguides.jadedns.uk` resolves and answers `/api/health` | **verified 2026-10-29** | `server/scripts/verify-live.sh` — `A1 PASS`, `/api/health` returned 200 (`{"message":"API is healthy.","code":200}`). **Canonical name only:** this row used to name the retired `*.duckdns.org` host, which is still *served* (A16/A17) but is no longer the hub's name — `docs/state.toml` `[hub.domain]` is the one home for that value. |
| A2 | The hub holds paid codes in active use (count deliberately not recorded — §4) | **unverified** | sign in to the console at `/admin/`, Codes & Clients |
| A3 | The rolling `build-preview` release is current for the pushed commit | **verified 2026-10-18** for `002d806` (released 2026-10-18T06:32:42Z) — note this claim was **falsified on 2026-10-18 morning**: the release had been deleted to cut storage, and it exists again because the pipeline recreated it | `gh release view build-preview --json tagName,assets,createdAt`; `verify-live.sh` reported PASS |
| A4 | The client CI workflow has run and gone green for a given commit | **verified 2026-10-29** for tag `v3.3.4` (`77b7204`, run `40785489912`: `Verify`, all four builds, `Report`, `Release` all success) | `gh run list --workflow=client.yml`; `verify-live.sh` reported PASS. The `v3.3.2` tag run **failed twice first** — on artifact storage quota, not code; see `../operate/CI-CD.md` |
| A5 | A published release is installable by an existing client end to end | **unverified** | needs real Windows/macOS hardware — see `../reference/STILL-OPEN.md` |
| A6 | The **Windows** NSIS installer completes on a clean machine — the setup runs interactively and installs Locus | **verified 2026-09-30** for the `v3.2.12` installer (`installer-Locus_3.2.12_x64-setup.exe`) | opened the NSIS setup on real Windows and completed the install; Locus installed. Re-check by running the setup on a clean machine and confirming the install finishes |
| A7 | The **installed Windows client stays installed** — it launches from `C:\Program Files\Locus\`, serves its own bundled assets, and is not relaunched out of its install directory by its own updater | **unverified** — and was **falsified in the field** on 2026-09-30 (see the note) | install on Windows, let it take an advertised update, then launch from the Start Menu and confirm the window renders the app. Nothing in CI launches either artifact, which is why three green releases shipped this broken — see `../reference/FIXES.md`, 2026-09-30 (second entry) |
| A8 | The **shipped client resolves its own bundled assets at run time** — a build carries no CI-runner path, so an installed app on a machine without the runner's drive renders the app rather than `ERR_FILE_NOT_FOUND` | **verified 2026-09-30** for `a8b0acf` (CI run `36686775049`) | the built `locus-windows-amd64.exe` was read back from the run's artifacts and the Tauri context string now carries the relative `../dist` where the broken `5ebfc89` build carried `d:/a/Locus/Locus/client/dist`. Re-check with `strings locus-windows-amd64.exe \| grep -c 'a/Locus/Locus'` (expect `0`) |
| A9 | The **macOS** update slot serves an installable `.app.tar.gz` — a macOS client is offered a payload its updater can apply | **unverified** — and was **falsified in the field** on 2026-10-05 for `3.2.26` (see the note) | `curl -s "https://networkingguides.duckdns.org/api/update?version=<prev>&platform=macos_arm"` must return `"url": …/locus-darwin-arm64.app.tar.gz`; and `curl -s …/updates/<v>/locus-darwin-arm64.app.tar.gz \| head -c2 \| od -An -tx1` must print `1f 8b` (gzip), not the Mach-O magic `cf fa ed fe`. Recovery: `RECOVER-MACOS-UPDATE.md` |
| A10 | **No live code carries `tier: "stealth"`** — the retired tier's row can be deleted without stranding a paying student | **verified 2026-10-29** | console → Codes & Clients → filter by tier; or `GET /api/collections/codes/records?filter=(tier='stealth')&perPage=1` with an admin token and read `totalItems`. A non-zero count means the disposition is "re-point, do not delete" — see `../business/04-tiers.md` §4.6. **Never** bulk-delete the rows either way. |
| A11 | The **live hub took the Free/Full merge** — port 8445 is capped at 100 Mbit, port 8444 is no longer shaped, and the retired service is stopped | **verified 2026-10-05** | read-only: `tc class show dev "$(ip -4 route show default \| awk '{print $5}' \| head -1)"` must show a `1:30` class at 100Mbit and **no** `1:20` class; `systemctl is-active shadowsocks-eco shadowsocks-strike` must be `active` and `systemctl is-enabled shadowsocks-stealth` must not report `enabled`. The tree is green either way — this is the box, not the repo. |
| A13 | **Free clients carry UDP** — the free plan's UoT listener answers on 8447 and the free tier rows advertise `uot_port` | **verified 2026-10-05** | after a `setup.sh` re-run: `ssh <hub> "systemctl is-active sing-box-uot-eco sing-box-uot-strike"` must both be `active`, `ufw status \| grep 8447` must show tcp+udp allowed, and `tc class show dev <iface>` must show a `1:20` class at 1mbit. Then activate a throwaway `free` code and confirm its heartbeat returns `server_config.uot_port: 8447` with `udp_relay: true`. The mechanism is a second instance of the paid path (which works), but "the second instance works" is not implied by "the first does". |
| A12 | A **paid code connects on the merged tier** — activation resolves `strike` and the heartbeat returns a `server_config` on 8445 with `udp_relay` true | **verified 2026-10-05** | activate a throwaway `strike` code (never a live student's) and inspect the `/api/activate` and `/api/heartbeat` responses; `server_config.server_port` must be 8445 and `udp_relay` must be `true`. This is the merge's central claim and it is the one thing that cannot be proven from the checkout. |
| A14 | The **3.3.0 release is published and signed with the current key** — CI built every platform, each asset carries a `.sig`, and the release is complete enough for the hub to fetch | **verified 2026-10-16** | `gh release view v3.3.0 --json assets` lists four platforms each with a `.sig` and a `manifest.json`; `gh run view 38830217320 --json jobs` shows `Verify`, all four `Build` jobs, `Report` and `Release` succeeded. Re-check with `gh release view v<current> --json assets`. This is the release the rotation made possible, not a property that persists. |
| A15 | A **124 MB `locus-linux-amd64` from the published release verifies against the new public key and is rejected by the retired one** — the rotation is live, not merely configured | **verified 2026-10-16** | download `locus-linux-amd64` (124,232,456 bytes) and its `.sig` for the current version, then `minisign -V -p <pubkey> -m …` must verify, and the same command against the retired key must fail. Command in `../../client/docs/SIGNING.md` §6. **Not** covered: that a client *installs* it — that is A5/A7. |
| A16 | **Both hub names present a currently-valid certificate and answer `/api/health`** — the canonical `networkingguides.jadedns.uk` *and* the legacy `networkingguides.duckdns.org` on which every pre-3.3.2 build is stranded | **verified 2026-10-29** | for each name: `curl -sf https://<name>/api/health` → 200, and `openssl s_client -connect <name>:443 -servername <name> </dev/null 2>/dev/null \| openssl x509 -noout -enddate` → a future date. **`verify-live.sh` runs exactly this and reported PASS on 2026-10-18**: canonical cert to `Jan 16 2027`, legacy cert to `Jan 15 2027`, both `/api/health` 200. **Two names, because they fail independently** — on 2026-10-18 the canonical name had no cert at all before the change (TLS alert 80) while the legacy one was valid throughout, and the reported "Expired" named the *legacy* one from a network path this register cannot see. A certificate that is valid is valid **on the date checked**; it expires, so this row decays faster than any other here. |
| A17 | **The previous hub name is served, not redirected** — it answers the full API surface from the same Caddy site block, so a pre-3.3.2 device can still activate, heartbeat and reach `/updates/` | **verified 2026-10-18** | `grep -nE '^networkingguides\.jadedns\.uk, networkingguides\.duckdns\.org \{' /etc/caddy/Caddyfile` → one site block naming **both**, and `curl -s -o /dev/null -w '%{http_code}' -X POST https://networkingguides.duckdns.org/api/activate -d '{}'` → 400 (a handler answered), **not** 308. **`verify-live.sh` reported PASS on 2026-10-18** (legacy name answered `400` directly). A 3xx here means the alias block swallowed the legacy name, which strands the fleet on the next certificate renewal. |

> **A10–A13 were verified on 2026-10-05 by the deploy that shipped them, and the
> redeploy of 2026-10-18 makes them false again** — the box they measured is gone.
> Kept as history, per the note above. What each one measured, in one line each:
>
> - **A10** — `codes` held a single tier value, `strike`. **No `stealth` code was
>   ever minted**, so the retired tier stranded nobody.
> - **A11** — tc showed `1:10 @ 1Mbit`, `1:20 @ 1Mbit`, `1:30 @ 100Mbit`, three
>   matching filters, **no 8444 filter**; the stealth unit and config were deleted.
> - **A13** — both UoT listeners (`8446` strike, `8447` eco) were `active`,
>   `enabled`, listening on TCP+UDP, and each tier row advertised its own
>   `uot_port`.
> - **A12** — one throwaway code per plan activated through the *public*
>   `POST /api/activate`, response read back:
>
>   | Plan | tier | TCP | `udp_relay` | `uot_port` |
>   |---|---|---|---|---|
>   | Free | `free` | 8443 | `true` | **8447** |
>   | Full | `strike` | 8445 | `true` | **8446** |
>
>   Both codes were minted for the test and deleted afterwards (never a live code,
>   never a bulk delete — §4). This exercised code → checksum → lookup → activate
>   → tier-config resolution → per-plan `uot_port`. **It still does not prove** that
>   a client tunnels real game traffic at the shaped rate: the response is the hub's
>   promise, and no machine has played through it.
>
> **Two defects that deploy found, both invisible in the tree** — worth keeping
> because each is a guard that read as passing:
>
> 1. **The UoT units were created but never started.** `systemctl enable` acts at
>    the next boot, and rewriting a unit does not restart it, so both plans had no
>    UDP while every check said "created". `02-shadowsocks.sh` now starts and
>    asserts them.
> 2. **`04-tc.sh`'s verification was a false negative.** It grepped `tc filter
>    show` for `sport <port>`, but a u32 filter prints as a hex match
>    (`match 20fb0000/ffff0000`) — so it warned "filter not found" for all three
>    ports on a correctly shaped interface. It now decodes and asserts the class,
>    with the retired 8444 checked as a negative.

> **The per-claim detail that used to sit below the table, compressed.** Each one
> records *why* a claim was carried as unverified, because the reason is the
> testable part:
>
> - **A7 (Windows client cannot update).** Falsified in the field 2026-09-30: a
>   `3.2.13` client accepted `3.2.14`'s download and refused it — *"not an
>   installer (51741696 bytes)"*, exactly `locus-windows-amd64.exe`. The client's
>   guard was **correct**; the hub's `fetch-release.py` was resolving the raw PE
>   while CI correctly advertised the NSIS name. **No Windows client could update
>   at all.** Three checks passed throughout: CI's manifest guard (it greps the
>   workflow, not the hub), the fetch service's all-or-nothing check (all four
>   *wrong* files were present), and the hash cross-check — which hashed whichever
>   file the manifest named, so it agreed with itself by construction. Fixed on
>   the hub with a `check-consistency.sh` §1a guard and a `smoke-publish.sh` case
>   shown failing against the pre-fix code. **A7 itself remains unverified:** no
>   Windows machine has taken an update and relaunched from the Start Menu.
>   *An earlier revision blamed a stale `start_page` in `verge.yaml`; withdrawn
>   2026-09-30 — that user's config carried `start_page: /`, so the path could not
>   fire. The property is real; only the cause was wrong.*
> - **A8 (the installed app resolves its own assets).** Verified on the
>   **artifact**, not the run colour, because four consecutive green runs preceded
>   the bug: the binary from run `36686775049` was downloaded and its Tauri
>   context string read directly — broken builds carry
>   `d:/a/Locus/Locus/client/dist`, fixed ones carry `../dist`
>   (`grep -c 'a/Locus/Locus'` returns `0`). *An earlier revision also attributed
>   this symptom to the `start_page` misdiagnosis and to the updater re-executing
>   the client; both are ruled out by the report itself — the installer was run
>   fresh from the release page.*
> - **A6 (the NSIS installer completes).** Verified 2026-09-30 by running
>   `installer-Locus_3.2.12_x64-setup.exe` interactively on real Windows. It does
>   **not** settle A7 or A8's interactive end — completing an install and
>   surviving an update are different properties.
> - **Green still proves nothing on its own.** A8 was advanced by reading an
>   artifact; A4's green run was not. Do not generalise one artifact check into
>   "the pipeline is trustworthy" — the next platform-specific leak would need its
>   own read-back.

---

## 6. Where the stable identifiers live

The **only** stable identifier for the deployed system is the **domain** — a DNS
name the operator controls, which survives the box moving. The console is a path
under it (`/admin/`); the API is its root. The domain's value is a derived fact
and lives in [`../state.toml`](../state.toml), not in this sentence.

**An IP address is never one of them.** It is a Class A claim about a moment, and
it belongs in this file's register or nowhere. Writing one into a document, a test
fixture or a script is the exact defect §1 describes.

---

*Added 2026-09-29. When you change a rule here, that is a Class B change to this
file: it is enforced by `server/scripts/check-consistency.sh` §9 and §10.*
