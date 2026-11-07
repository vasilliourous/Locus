# Fixes

```
audience:    builder
status:      live
authoritative-for: the dated defect log — what broke, what fixed it, and the rule it produced
verified-against: docs/state.toml (version/release facts)
```

Every defect below is a dated entry, newest first, naming the release it went out
in. **Treat entries as historical** — later entries sometimes correct earlier ones.
For what is *still* broken, read [`STILL-OPEN.md`](STILL-OPEN.md); for the
commercial gaps, [`../business/17-not-built.md`](../business/17-not-built.md).

> **The server-side halves do not reach the live host until `setup.sh` re-runs.**
> `setup.sh` deploys from `/root/server/` on the VPS, not from this repo, so every
> hook and script change here is inert until that re-run. A heartbeat that lacks
> `enforcement_version` in its response is proof the hook is still stale.

> **This log is a summary index, not the transcript.** Each entry keeps what broke,
> what fixed it, and the rule it produced. The full investigation behind any entry
> is in git: `git log -S'<heading phrase>' -- docs/reference/FIXES.md` finds the
> commit that introduced it.

---

## THREE THINGS THAT MADE CI SLOW, FLAKY, OR SILENT (2026-11-07, unreleased)

Found while getting the pipeline green before a release. None of these is a
product defect; all three made CI lie about the code, which is the same thing
from the seat of whoever reads a red run.

**1. The Rust cache was keyed on the GitHub job id, so it was never reused.**
`Swatinem/rust-cache`'s `add-job-id-key` defaults to **true**. A job id is unique
per run, so the key a run *saved* under could never be the key the next run
*looked up*. The action still reported `Cache restored successfully … full match:
true` by falling back to an older entry, and Rust then ignored the restored
`target/` because the fingerprints did not match. **Restored-but-unused is worse
than no cache**: it spends the download and buys nothing. Every run compiled the
dependency tree from scratch. Fixed with `add-job-id-key: false` and an explicit
`shared-key` per triple; the Linux cache went from 174 MB to 3,108 MB.

*The rule:* a cache's own success line does not mean the artifacts were used.
Count `Compiling` lines. See [`../operate/CI-CD.md`](../operate/CI-CD.md).

**2. The tests compiled in a different profile from the build.**
The comment above the test step claimed they "reuse the target directory `tauri
build` just populated, so the compile is largely already done." False, and
always had been: `Build` writes `target/<triple>/fast-release`, bare
`cargo test` writes `target/debug`, and those share nothing. ~660 crates were
compiled a second time on every run. The tests now pass the same `--profile` the
build used. **Combined with (1), the whole run went from 39 minutes to 21**, and
the Linux `Build` step from 29 m 07 s to 16 m 28 s.

*The rule:* a comment that explains why something is cheap is a claim about
timing, and timing is measurable. Measure it before repeating it.

**3. `a_warm_engine_is_reused_…` was flaky, and had been.**
It arranged the process-wide `tunnel_slot()` and asserted on it, while
`a_stale_engine_is_not_reused` called `drop_warm_engine()` on the same static.
Cargo runs tests in parallel by default, so one test could clear the slot between
another's setup and its assertion — `a warm engine must be handed back`. Measured
at **9 failures in 15 runs** without a guard. Both tests now take an async mutex
(`tokio::sync::Mutex`, because the guard is held across `.await`; a blocking
guard is rejected by `await_holding_lock`). **0 failures in 15 runs** with it.

*The rule:* a test that mutates a process-wide static and a test that clears it
must be serialised, or the suite is a coin toss. Found only because an unrelated
profile change made it fail more often — a flake is not evidence the *change*
broke something until it is reproduced on the old code.

**Also closed here — a port reservation that reserved nothing.** `free_port()` in
`tunnel_end_to_end.rs` bound port 0, read the number, and dropped the listener,
leaving the port free but *unreserved*. Three consecutive calls returned the same
port twice in about 1 of 1000 triples (measured), and another starting test could
take the port in the gap. It surfaced as `connect to the mixed port: Connection
refused`, which reads as a broken tunnel. Each port is now held until the process
that binds it starts.

---

## FOUR DEFECTS THAT MADE THE DEPLOY EITHER LIE OR DIE (2026-11-07, unreleased)

A hub was rebuilt from a blank slate to deploy the school-network fix. All four
defects below are in **deployment scripts**, and every one of them passed a green
tree — `cargo test` 665, `clippy`, `check-consistency.sh` — because none of them
is reachable from a test. This is the repository's oldest lesson, stated at the
top of `URGENT-FIX-NOW.md` and now with four more instances: *a repo fix is inert
until the piece that runs it is redeployed*, and a deploy script can be broken
while every check that reads the tree is green.

**1. `setup.sh` let the secrets bundle overrule an explicit `DOMAIN`.**
`set -a; source "$SECRETS_TMP"` makes every bundle line an assignment, and an
assignment overwrites what the caller passed. `DOMAIN=<canonical> setup.sh` with a
bundle containing `DOMAIN=<legacy>` deployed the **legacy** name, silently. The
committed bundle still names `networkingguides.duckdns.org` — the name only
pre-3.3.2 builds dial — while `contract::HUB_URL`, which every current client
dials, is `networkingguides.jadedns.uk`. The result would have been a correct hub
at a hostname no client asks for: green deploy, zero users. Fixed by capturing
`DOMAIN` before sourcing and restoring it after.

**2. The same `source` expanded `$` inside `VPS_PW`.** The password contains a
literal `$`; `source` parses it as a variable and, under `set -u`, the deploy died
with `line 49: Zy6N: unbound variable` — after installing `age` and decrypting,
before installing anything. A generated password may contain any printable
character; the secrets format does not get to forbid it. Fixed by reading the
bundle literally (`KEY=value`, split on the first `=`, no expansion) in **both**
`setup.sh` and `restore.sh`, and reporting a malformed line instead of skipping
it. Recorded in `state.toml`-style prose nowhere: the rule is *the secrets file is
data, not shell*.

**3. `modules/09-activation-tunnel.sh` sourced a script that calls `exit 0`.**
`00-env.sh` is written to be **run** as `setup.sh`'s first module and ends with
`exit 0`. Module 09 did `. 00-env.sh 2>/dev/null || true`. Sourcing a file that
calls `exit` terminates the **calling** shell, and `|| true` cannot catch it —
`exit` is not a failed command, it is the end of the process. So the module
printed `00-env`'s validation and exited **0**, having installed nothing: no unit,
no config, no key, no listener. `setup.sh` printed
`✓ Module 09-activation-tunnel completed successfully`. That is a fourth
independent reason the activation tunnel could never have worked, on top of the
three recorded on 2026-11-06, and it is the only one that made the deploy lie
rather than fail. Fixed by running `00-env.sh` as a **child process** and
inspecting its status.

**4. The hourly backup timer was life-coupled to PocketBase.**
`pocketbase-backup.timer` carried `Requires=pocketbase.service`. systemd's
`Requires=` propagates **stop** as well as start, so the first planned
`systemctl stop pocketbase` — which `restore.sh` does on *every* restore, and any
upgrade or DB swap does — deactivated the timer permanently. `systemctl
is-enabled` still reported `enabled`, because that only re-arms it at boot. The
only symptom is that the newest backup in B2 quietly ages, and you discover it
when you need the backup. Found immediately after a data restore, which is exactly
the operation that triggers it. Fixed with `After=` (the ordering that was
actually wanted) and, separately, by rewriting an already-deployed timer that
still carries the bad directive — the "file exists, leave it" early return would
otherwise have made the fix unreachable on any host that had ever run the module.

**The rule the set produces — a deploy script's failure mode is *silence*, so
check the artefact, not the exit code.** Every one of these reported success or
died somewhere unrelated to its cause. What caught them was reading the deployed
system rather than the deploy log: `systemctl is-active` on every unit,
`ss -lntp` for the listeners that should exist, the sha256 of the key the box
actually runs, and one request against a real code through the live API.

**Guards, each shown failing first:** `check-consistency.sh` §43 (DOMAIN
precedence and a literal `$` round-trip, both by lifting the real block out of
`setup.sh` and running it) and §44 (no `Requires=` on the timer unit, and an
existing bad timer is rewritten). §43 was itself fixed after it misreported
correct code: its harness ran via `bash - <<HEREDOC`, which collides with the
heredoc being the script.

---

## THE ALIAS SERVED THE API AND COULD NOT DELIVER THE UPDATE (2026-11-07, unreleased)

The hub alias (`HUB_ALIAS_DOMAIN`) exists for one network: N4L, which blocks the
canonical name outright (`CLAIMS.md` §5 A21). Its block was a single
`redir https://<canonical>{uri} 308`, and that was reasoned about correctly for
the *API* — one implementation, one rate-limit zone, one admin check.

It was wrong for **downloads**, and downloads are the only thing keeping the
device there alive. The redirect sends the client to the canonical name, which on
that network is the name the filter blocks. So a device could activate, heart
beat, and then be permanently unable to fetch the release that would relieve it.
The client now refuses to follow the redirect and reports it, but that is a
diagnostic — it turns a silent stall into a named failure, not into an update.

**Fix:** the alias block serves `handle_path /updates/*` from the same document
root (`/var/www/updates`) and keeps a catch-all `handle { redir … 308 }` after it
for everything else. Caddy evaluates mutually exclusive `handle` blocks in order,
so the downloads handle wins and the API still exists in exactly one place.

**The rule it produced — a redirect is only neutral when the target is reachable
from the network doing the redirecting.** That is the question to ask before
reusing one, and it is network-dependent, which is why this survived a review
that was looking at the API. Deliberately narrowed, not dropped: `FIXES.md`'s
neighbour entry above is about a check that could not fail, and the alias guard
had the same shape — `awk … | grep -q 'handle'` returned "no handle" both when
the block held none and when the awk matched nothing at all.

**Guards, each shown failing first:**
`check-consistency.sh` §32 now extracts the alias block once and asserts (a) it
serves `/updates/*`, (b) it serves *no* handle other than `/updates/*`, (c) its
redirect is a catch-all `handle {`, and (d) it proxies nothing. Proven against
four mutations of the real block: the pre-fix bare-`redir` block fails (a) and
(c); adding a duplicate `/api/*` handle fails (b); deleting the block fails on the
empty extraction; and replacing the catch-all `redir` with `reverse_proxy` —
which passed all three original checks — fails (d), and is why (d) exists.
On the box the assertion is an artifact fetch that must be **`200`, not `308`**;
`modules/05-caddy.sh` also probes it after reload and warns on a 3xx.

**Not verified:** no Caddy instance has parsed the new block. `caddy validate`
was not run — Caddy is not installed on the workstation this was written on, and
the block has only been rendered and read. The first `setup.sh` run on the hub is
where it is either accepted or refused.

---

## A HOOK FILE WAS ON THE HUB AND ITS ROUTE WAS NOT, AND EVERY CHECK PASSED (2026-11-06, unreleased)

`POST /api/release-entitlement` — the student sign-out record — answered
`404 {"code":404,"message":"Not Found.","data":{}}` on the live hub. That is
**PocketBase's own router**, not the hook: `device_release.pb.js` answers its own
404 with `{"code":404,"message":"Nothing to release","released":false}`. A route
that is not registered looks exactly like a code that does not exist, so the
symptom was "sign-out silently fails to be recorded" with no error anywhere.

`hooks-sync.sh` was **green** throughout. Its checks were: is the service running,
do the files on disk match the repo, and does `GET /api/release` answer. The
sign-out hook is not `/api/release`, and no check ever asked a route that this
hook owns. A corroborating measurement: eleven consecutive POSTs to the route
returned `404` and never `429`, while the same hammer against `/api/activate`
produced `429` on the fifth — so Caddy's `release` rate-limit zone is not loaded
either, which a redeploy of `05-caddy.sh` provides.

**Guard:** `hooks-sync.sh` now probes every route a hook owns and reports
"answered by PocketBase (not a hook)" when the router — not the hook — replied. It
was **shown firing against the live hub for exactly this route** while passing the
three that work.

**A guard that could not fail, caught the same hour.** The first version keyed on
"the body carries no `message`" — and PocketBase's not-found *does* carry one, so
it passed the broken hub. It now keys on `"data": {}`, which the router's 404
always has and no hook in this directory produces. Measured, not assumed.

---

## THE UPDATE PATH NAMED A HOST THE CLIENT COULD NOT FETCH, ON THE ONE NETWORK THAT NEEDS IT (2026-11-06, unreleased)

Found while tracing why a school device could not recover. The update path is the
**only** way out of a network that blocks the hub's name — a device that cannot
reach the hub cannot fetch the build that would fix it — and it was broken in
three independent ways, all measured against the live hub with `curl`.

1. **The offer named a different host from the one the client dials.**
   `/api/update` and `/api/release` both returned
   `https://networkingguides.duckdns.org/updates/3.3.5/…` (the *legacy* name)
   while the client dialled `networkingguides.jadedns.uk`. The client's own origin
   check would have refused the artifact — **on an uncensored network too**.
   Fixed on both sides: the hub rewrites the offer's origin to the host the
   request actually arrived on (`update.pb.js`), so a device moved to an alias is
   offered a URL on that alias; and `signal::OfferHosts` accepts the dialled hub
   *plus* the compiled-in and legacy names, so the published record still works.

2. **But an absolute URL built from ANY hub hostname is unfetchable on that
   network**, because both names are blocked. So the alias had to become usable
   for downloads, which it was not: `apply::download` ignored `hub_url()` and
   fetched the offer verbatim. Now the hub-side rewrite and the client-side
   allowlist meet, and a redirect is **refused rather than followed** — the alias
   block `308`s to the canonical name, and following it would move the request to
   a host the allowlist never checked, turning an explainable refusal into an
   unexplained timeout.

3. **The update CHECK bypassed every hub-client rule.** `updater_builder()` gave
   the plugin a plain `reqwest` client: OS certificate store only, the
   environment's proxy, redirects followed. That is the exact client `locus::hub`
   exists to replace (an unpatched Windows store cannot verify the hub's chain and
   reports it as `Expired`), and the update path is where that trap bites hardest.
   `install::check` now overrides it with `configure_client` — no proxy, no
   redirects, and the same ceiling as an activation call. The artifact fetch also
   goes through `locus::hub::client_refusing_redirects`, and the consistency
   checker's "no hub-facing module builds its own client" rule refused the
   hand-built one I first reached for, which is the guard doing its job.

**Guards.** `an_artefact_may_come_from_the_dialled_hub_or_the_compiled_one` and
`refuses_everything_else` (observed failing when the allowlist was reduced to one
name, and when it was relaxed to accept any `https` host — including a
`contains`-style look-alike `vpn.example.org.evil.example`), plus three executed
assertions in `check-consistency.sh` §42.

**A guard that could not fail, caught and fixed the same day.** The first version
of §42(a) grepped the hook for `offerHost` and `url.substring(cutoff)` — tokens
that survive a `if (false) {` in front of the block, so it reported OK against a
hook that rewrote nothing. It now lifts the block out and **evaluates it** with
`node` against four shapes. §42(c) had the same flaw in a different form: it
grepped for `LEGACY_HUB_URL` anywhere in the file and was satisfied by a doc
comment. Both were observed failing only after the rewrite.

> The hub half is inert until `hooks-sync.sh` runs against the hub — the deployed
> copy is what serves `/api/update`. Measured 2026-11-06: the live hook served the
> legacy origin, so that deploy had not happened.

---

## THE ACTIVATION TUNNEL COULD NOT HAVE WORKED, AND A GREEN TREE SAID "BUILT" (2026-11-06, unreleased)

Reported: *"on the school network a tunnel cannot form at all — fine if you
connect on a hotspot and switch, fine if you bring it from home, but it cannot
connect at school, and you cannot get past the sign-in page."* The operator also
confirmed the hub's canonical name is blocked there outright.

Everything needed for this existed and was documented as built, tested and
guarded. Three defects, each independently sufficient, made it inert. Each was
found by **running against the live hub**, not by reading the source.

### 1. THE FALLBACK NEVER FIRED ON A `Refused`

`Failure::retryable_over_tunnel()` returned true for `ResetDuringHandshake` only.
A filtering appliance that proxies the ClientHello and tears the flow down reaches
the client as a reset *or* as a refusal, depending on how far it got — so on the
day it answered with a refusal, the one transport built for that network was never
attempted. The exclusion's reasoning ("a different transport to the same address
will be refused too") holds only when the same party answers on both, and on this
network the party that answers is the filter. `Refused` is now in the set; the
cost of being wrong is one refused SOCKS dial, reported in the same sentence as
the original failure.

**Guard:** `a_refusal_must_reach_the_activation_tunnel` — observed failing when
the set was reverted to `ResetDuringHandshake` alone.

### 2. THE OUTBOUND DIALLED THE STUDENT'S OWN LOOPBACK

`render_config` wrote `server: 127.0.0.1` into the Shadowsocks outbound, under a
comment claiming mihomo rewrote it to the SS server's host. It does not: the
outbound connects to exactly the value it is given. So the bootstrap engine tried
to reach a listener on the student's own machine, on every platform, and could
only ever have worked against a local one. Now `contract::ss_hub()` — the host of
the resolved hub URL, so a device moved to an alias moves its bootstrap dial too.

**Guard:** `the_config_dials_the_hub_and_not_the_local_machine` — observed failing
against the old `127.0.0.1` value.

### 3. THE KEY WAS ABSENT FROM EVERY SHIPPED BUILD, SILENTLY

`option_env!("LOCUS_ACTIVATION_TUNNEL_KEY")` was read directly in
`activation_tunnel.rs`, and `build.rs` did not re-emit it. Cargo does not reliably
rebuild a crate for an `option_env!` that was absent last time, so a key that
appeared after a cached build could leave the constant stale; and CI reported a
missing secret as a **warning**, so a tagged release shipped keyless. Three did.
Nothing in the artifact said so, and the student's only evidence was "the tunnel
never helps".

`build.rs` now re-emits the value as a `rustc-env` with an explicit
`cargo:rerun-if-env-changed`, and stamps presence so the binary can report it.
`activation_tunnel::build_stamp()` is logged at startup (a warning when absent),
and CI **fails a tagged build** without the key.

**Guard:** the CI step itself, plus `build_stamp`'s consistency check. The
consistency checker cannot test a release's environment; the startup line can, and
that is the one a support log carries.

### 4. THE ENGINE WAS SPAWNED PER CALL, SO THE ADVISORY LOOKUP DISABLED ACTIVATION

Enrolment is two requests — the advisory `/api/code-lookup`, then `/api/activate`.
The failure flag is session-scoped, so a transport failure on the lookup refused
the activation *before* attempting it; and each call paid a full start of a 60 MB
sidecar. The engine now lives in one slot, reused while warm and killed on a
transport failure.

**Guard:** `a_warm_engine_is_reused_so_two_requests_cost_one_start` — observed
failing when the slot is not reused.

### What this says about the tree, kept because it recurs

Every one of these read as "built" from the repository: the module exists, its
tests pass, `check-consistency.sh` §40 is green, and the docs describe the
mechanism accurately. What none of them covered was whether the assembled artifact
could reach the hub. **A guard on each half does not guard the join**, and the join
is where all four defects were.

---

## FIVE FIELD REPORTED DEFECTS, ONE OF WHICH HAD NO SENTENCE AT ALL (2026-11-06, unreleased)

Five reports arrived together from an operator running the client on a school
network and on an unofficial Windows install. Four of them turned out to be
defects this repository could see; the fifth is a world claim about a third
party's DNS and does not live here. Recorded together because they share a
cause: **a failure the backend could not name was reported as one the student
could not act on.**

### 1. A MISSING PRIVILEGED HELPER WAS REPORTED AS "FAILED TO INSTALL THE SERVICE"

Field report: *"it was an unofficial Windows installation, and when trying to
install the service a pop-up message with the exact same phrase was there, and
did not go away until I tried installing after installing the Microsoft VC
package."*

- **What broke.** The elevated installer is **not linked into the app** — it is a
  file inside the bundle (`clash-verge-service-install.exe`, from the pinned
  `clash-verge-service-ipc` v2.7.4 release) spawned by
  `clash_verge_service_ipc::management::install`. When it cannot be loaded, the
  Windows loader shows `VCRUNTIME140.dll was not found` **before any of our code
  runs**, so the dialog is unattributable by our own instruments: the spawn
  returns an `io::Error`, `install_service`'s `?` discarded it, and the student
  saw `settings.feedback.errors.clashService.installFailed` — *"Failed to
  install the service"* — which names a service fault that does not exist and
  tells them nothing they can do.
  **Fix.** Two halves, in order of what a student needs first:
  `require_packaged_tool` checks the helper exists **before** spawning it and
  raises a typed `MissingServiceTool` naming the file and its path; and a spawn
  that fails with `ERROR_MOD_NOT_FOUND` (126 — the missing-DLL code) is
  re-wrapped with `MISSING_RUNTIME_CONTEXT` and logged at `Type::Service` with
  the path that failed. `cmd::service::classify_privileged_failure` downcasts the
  type first and matches the constant second, so the student gets
  `SERVICE_HELPER_MISSING` with a sentence naming the file, the reinstall, and
  the C++ runtime as the second possibility. It is deliberately **not** reported
  as an elevation failure: "approve the prompt" is the wrong instruction when
  there is nothing to launch.
  **Guards, each observed failing** against the pre-fix classifier: three of five
  `cmd::service` tests (missing-helper naming, ordering against an elevation
  sentence in the same chain, the runtime sentence) fail when
  `missing_helper_failure` is removed from the chain.
- **Still unverified.** That `VCRUNTIME140.dll` is what the reported machine
  lacked is the operator's report plus the loader-behaviour argument — **not** a
  measurement of the binary. The service release archive could not be inspected
  from this checkout (the download was refused by policy), so the PE import table
  was never read. See [`STILL-OPEN.md`](STILL-OPEN.md).

### 2. A HUB HOSTNAME THAT DID NOT RESOLVE HAD NO REMEDY

- **What broke.** `hub::Failure::classify` folded DNS failures into `Transport`,
  and `Transport::remedy()` returns **`None`**. A student on a school network
  therefore got `could not reach the Locus hub at …: error sending request for
  url (…)` and nothing else — a sentence that reads as "the hub is down" when the
  true state is "your network did not translate the name, so nothing was
  dialled". The remedy needed to exist and could not be written for a bucket that
  also contained timeouts and drops.
  **Fix.** `Failure::Dns` and `Failure::Refused`, classified from
  `io::ErrorKind` in the error chain (`NotFound`/`HostUnreachable` for naming;
  `ConnectionRefused` separately) — the same walk `reset_in_chain` already does,
  and ordered **after** the reset and certificate checks so neither becomes
  unreachable. `retryable_over_tunnel()` stays `ResetDuringHandshake` only: the
  tunnel is dialled by name, so a resolver that answers nothing answers nothing
  for it. Both are now appended to the activation error (`post_json`) and to the
  heartbeat's `Unreachable { reason }`, so the two reported modes — "cannot reach
  the hub" and "forcibly closed (os error 10054)" — are two different sentences
  instead of one.
  **Guards.** `io_kinds_in_the_chain_decide_between_dns_and_refusal` was observed
  failing when `is_name_resolution` was pointed at `ConnectionRefused`; the
  remedy tests assert the DNS sentence names the network, does **not** promise
  the tunnel, and differs from the refusal sentence. Removing the `Dns` arm from
  `remedy()` does not compile (non-exhaustive match), which is a stronger guard
  than a test.
- **Still a world claim.** Whether the school's DNS is the cause is about a third
  party and lives in [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5. No client
  change can settle it.

### 3. A DAMAGED STORED CODE WAS NEVER VALIDATED, AND BLOCKED ITS OWN REMOVAL

Field report, macOS arm: *an invalid code stored on the client did not sign out
the client; the client then connected with it and failed silently; the only
recovery was uninstalling and wiping app data.*

- **What broke, in two places.** `validate_code` ran **only on a code the
  student typed** — nothing checked the value read back from `verge.yaml` or the
  machine mirror, so a damaged one was sent to the hub and its refusal surfaced
  as a connection problem. And `store::read_code_rehydrating` **adopted the
  mirror value back into the config** with no check at all. Worst of all,
  `signout::sign_out` calls `activation::release` first (the documented order),
  and `release` validates locally — so with a damaged code the whole sign-out
  errored before a single byte was deleted. That is why uninstall was the only
  exit: the recovery path required the very credential that was broken.
  **Fix, three parts.** `store::read_valid_code` and
  `store::stored_code_is_damaged` apply the local checksum to what is stored;
  `read_code_rehydrating` no longer adopts a value that is not a code. `sign_out`
  now validates the code itself and, on failure, skips the hub request and
  completes the **local half** — the destructive statements were already outside
  the hub arm, and a guard now pins that. `locus_connect` refuses with its own
  `LOCUS_STORED_CODE_DAMAGED` rather than `LOCUS_NOT_ACTIVATED`, because the
  student *did* activate; `locus_status` carries `storedCodeDamaged` and the
  Account page says the sign-in is damaged instead of showing a fresh-device
  screen.
  **Guards, each observed failing.** Removing the `validate_code` call from
  `read_valid_code` fails
  `a_damaged_stored_code_is_not_offered_as_a_credential`; the companions
  (`a_valid_stored_code_is_still_offered`,
  `a_device_with_nothing_stored_is_not_reported_as_damaged`) stop that guard
  passing for a function that refuses everything, and
  `a_damaged_stored_code_does_not_block_the_local_half_of_a_sign_out` pins the
  ordering property.
- **Verified vs unverified.** The behaviour is proven at the unit level on Linux.
  **Nobody has run it on a Mac**, which is where it was reported.

### 4. A DEFERRED START WAS REPORTED AS A BROKEN CORE

Field report: *the configuration failed to apply with `core did not become ready
after restart`; disconnecting and reconnecting fixed it, but only an admin knew
to try.*

- **What broke.** `restart_core_during_config_update` tested
  `mode == NotRunning || no readiness generation` and `bail!`ed one sentence for
  both. But the two facts mean different things: `start_core_inner` legitimately
  returns `Ok(())` having started nothing when the start was **deferred**
  (`StartupDecision::Wait` — the Service is absent and the install could not be
  raised) or the app was quitting. A deferred start left `NotRunning`, so it
  produced *"core did not become ready"* — false, since nothing was wrong with
  the Core — and named no cause. "Disconnect and reconnect" worked because a full
  stop/start cycle clears the deferred state, which is precisely why an admin
  found the workaround and a student could not.
  **Fix.** `restart_readiness_failure` splits the two: still `NotRunning` after a
  start is reported as a start that did not happen, naming the Service and the
  permission prompt or the Repair control; running without a readiness generation
  gets its own sentence. The mode is read into a local so the two tests cannot
  race. A `warn` at `Type::Core` records the mode the start actually settled in,
  which is the line a support log will need.
  **Guards, observed failing** against the single `bail!`: the deferred-start
  test and the own-sentence test both fail, and the healthy-restart test stops
  the two arms being unconditional.

- **The rule all four produced.** A failure state the backend can name is
  worth more than a message added at the top: `NotRunning`-after-a-start, a
  damaged credential, a name that did not resolve, and a helper that never
  launched are four different facts, and collapsing any two of them into one
  sentence hands the student a remedy for a fault they do not have.

## A CONNECT THAT COULD NOT START WAS INDISTINGUISHABLE FROM ONE THAT FAILED SILENTLY (2026-11-05, unreleased)

Field report: *"it tries to connect, then just fails and cancels with no error
message"*, on macOS arm. Three separate defects shared that one sentence, and the
sentence is why it stayed unfixed — none of them produced an `error` to log.

- **What broke, 1 of 3 — a refusal travelled as text.** `apply_tier_waiting_for_the_guard`
  decided whether the engine's start had been *deferred* (another caller holds the
  config-update guard) or *refused* with
  `message.contains("already running")`. That is a string match across a module
  boundary against a sentence raised two modules away. Any rewording moved a
  scheduler collision into a hard `LOCUS_CONNECT_FAILED`, or a genuine refusal
  into a 30 s wait. Neither direction fails a test.
  **Fix:** `CoreStartRefusal::{Held, Refused}`, raised by `start_core` and
  classified by the caller. The text is still what a log shows; nothing *decides*
  on it. `the_guard_refusal_is_held_not_a_refusal` was observed failing against the
  old substring mechanism before it was kept.

- **What broke, 2 of 3 — a cancellation leaked into the next attempt.** The
  `ApplyWait::Cancelled` branch returned with the comment *"consume the flag the
  way the readiness loop does"* — but that branch returns **before** the readiness
  loop, so nothing consumed it. `CANCEL_CONNECT` stayed set, the command ended, and
  the student's **next** connect cancelled itself on its first pass and reported
  `"Cancelled"` for a press they had not made on that attempt. On screen: a connect
  that starts and immediately aborts, with no error.
  **Fix:** `cancelled_outcome()` consumes the flag, and the real branch calls it.
  The first version of the guard test open-coded the consume and therefore **passed
  while the shipped code leaked** — a check that cannot fail. Extracting the body
  is what made the test able to fail, and it was then observed failing with the
  consume removed.

- **What broke, 3 of 3 — a start that started nothing looked successful.** On macOS
  `prepare_startup` returns `StartupDecision::Wait` when the Service is absent and
  `require_install_for_session()` errors — a denied elevation, or a macOS "remember"
  rule auto-cancelling the prompt. `start_core_inner` turns `Wait` into `Ok(())`,
  so every layer above treated it as a start in progress. The student waited out
  the whole 30 s readiness budget and was told the tunnel was "taking longer than
  usual", which is not what happened. Nothing was logged and nothing was on screen.
  **Fix:** the `Wait` path now logs a warning naming the cause, and `locus_status`
  carries `connect_blocker` — computed from **the same run-state snapshot the
  connect gate reads**, so the screen and the gate cannot disagree. The Connection
  screen renders it with the OS prompt as the next step, because "try again in the
  app" is the wrong instruction when the app already asked.
  **Still unverified:** that this is *the* macOS-arm path. The link from a denied
  prompt to `require_install_for_session()` returning `Err` is inferred from the
  code, not observed on a machine. One log line from an arm host closes it.

- **The rule.** A failure that never happened — because the thing it would have
  failed at was never started — has no error to report. That is not a missing
  message to be added at the top; it is a state the backend must name, or every
  layer above reports the absence of a fault as progress.

## THE EGRESS UNIT GUARD COULD NOT FAIL, AND ITS RED MESSAGE NAMED THE WRONG CAUSE (2026-11-05, unreleased)

- **What broke.** `egress_probe_engine::the_delay_timeout_field_is_milliseconds`
  returned `404 {"message":"Resource not found"}` instead of the `400` it asserts,
  and its failure message claimed *"the field's parsing has changed and every unit
  assumption in probe.rs needs re-deriving"*. **That was false.** The parsing had
  not changed; the test had asked too early.
- **The mechanism, from the engine's own log.** `RESTful API listening at …` is
  logged **29 ms before** `Start initial compatible provider "Locus Auto"`.
  `wait_for_controller` polls `/version`, which is served at the first line, so a
  test that runs before the proxy table exists gets a `404` for
  `/proxies/<name>/delay` — the route is real, the outbound is not registered yet.
  It reproduces under load and not in isolation, which is why the same file passed
  on a quiet machine and in a single-test run.
- **Why the guard did not catch its own uselessness.** The first assertion was
  `assert_ne!(at_max, 400)`. A `404` is not a `400`, so it **passed** — the
  assertion written to prove "the engine parses `i16::MAX`" proved nothing. Only
  then did the second assertion fire, and its message borrowed the authority of the
  one that never ran.
- **Fix.** Wait for `/proxies` to answer before asking, and assert the route
  actually responded (`200`/`503`/`504`) before making any claim about the field.
- **The rule, restated because this is its third appearance.** A red check must
  name the mechanism it observed, not the one it expected. And a "not equal" guard
  is satisfied by every wrong answer except one — assert the *shape of a real
  answer* first, then the property you care about.

---

## THE SUPPORT ROUTE EXISTED ON ONE SCREEN, AND THE SCREEN THAT NEEDED IT HAD NONE (2026-11-04, unreleased)

- **What broke.** The support address lived in exactly one place: a one-line
  caption under the Connect button, with no instruction attached. The **activation
  gate** — the screen a student is stuck on by definition, and the only screen a
  person who has not activated will ever see — rendered nothing at all, and did so
  **on purpose**: a comment there argued that the card is the support route
  ("the code is printed on it, beside whatever contact details the reseller puts
  there"), so a placeholder address would only move the problem.
- **Why the argument was wrong.** Its premise is that the student still has the
  card, and the student who reaches that screen is exactly the one who may not: a
  code that was refused, a card lost, a device handed over without it, or a
  middleman they no longer have a number for. The comment also silently mixed two
  claims — that the *hub hostname* must not be shown (correct, and still enforced)
  and that *nothing* should be shown (which left the funnel's last step as a dead
  end). Nobody could act on it, and the resulting support conversations start with
  a student who had no stated way to reach anyone.
- **The fix.** One component (`components/shared/support-block.tsx`) rendered by
  three screens: the gate (always), Connection (only when something is wrong —
  refused, lapsed, an error, or `!canConnect`), and Account (always, beside the
  Device ID a report needs). It names the middleman first, gives the address as a
  `mailto:` action, says what to include, and states the reply promise.
- **Guards, each observed failing.** `check-consistency.sh` §41 — the address is
  duplicated in source (reported), a screen stops rendering the block (reported
  for each of the three), the promise loses its stated window (reported), and a
  `supportBlock` key is emptied (reported). Its first version was
  `grep -q SupportBlock`, which the **import line** satisfied: it passed with the
  component deleted from the page body, and was rewritten to match the JSX tag
  after being observed failing only on a hunk that kept the import.
  `connectionNeedsSupport` is a pure rule with tests in
  `pages/connect-notice.test.ts`, proven to fail when forced to `true` (a working
  session must not show the block) and to `false` (a stuck one must).
- **The rule.** A support route is part of the product, not a footnote on the
  screen where the operator happened to think of it. If a surface can leave
  someone unable to proceed, the way out of it belongs on that surface — and the
  sentence promising a response time is a **world claim** about the operator, so
  it is registered in `operate/CLAIMS.md` (A18) rather than asserted as a
  property of the tree.

---

## A SIGN-OUT WOULD HAVE UNDONE ITSELF, AND THE OBVIOUS FIX WOULD HAVE LET STUDENTS RESET THEIR OWN TERM (2026-11-04, unreleased)

- **What broke.** There was no sign-out at all. A student moving to a new computer
  had no way to say so, and the operator's only lever was a reinstall — which did
  not even work: `credential.rs` deliberately mirrors the code to a
  machine-scoped path that **survives an uninstall**, so a reinstall kept the
  old code and the "few key issues" reports had no self-service answer.
- **The two bugs hidden in the obvious implementation.**
  1. **Clearing `verge.yaml` alone is undone by the mirror.** The next launch
     re-adopts the code from the machine-scoped copy. `sign_out` therefore calls
     `store::clear_entitlement` — the one function that also removes the mirror —
     and `store.rs` keeps a **counter-example test**
     (`clearing_only_the_config_would_be_undone_by_the_mirror`) proving the partial
     clear really is undone, so the claim is falsifiable rather than prose.
  2. **A deletion is not enough while a beat is in flight.** `runtime::stop()` asks
     the loop to stop; it does not prove the task has. A beat already inside
     `read_code_rehydrating` could re-read the mirror and rewrite it a moment
     after the sign-out reported success. A `signed-out` marker is now written
     **before** the deletion and makes `credential::write_code` refuse, and the
     next successful activation clears it.
- **What the hub half must not do.** The obvious server-side "release the key" is
  `admin_unbind.pb.js`, which clears `codes.activated_at`. Doing that from the
  client would (a) reset the base a re-activation derives the term from, letting a
  student restart their own clock, and (b) be reachable by anyone holding a code —
  which is printed on every card and sits in middleman stock — so one student
  could release another's. `POST /api/release-entitlement` therefore records an
  audit fact (`released_at`, `release_count`, a `code_events` row) and touches no
  entitlement field. **Re-activation of a redeemed code was already allowed**, so
  the student's next device does not depend on the call succeeding — which is why
  a hub failure reports `notConfirmed` instead of failing the sign-out.
- **The order is asserted, not just documented.** `signout.rs` keeps the sequence
  as data (`ORDER`) **and** reads its own source to compare the call sites' byte
  offsets, because a test over the enum alone passed with `clear_entitlement`
  moved above the release. Observed failing against exactly that reordering.
  A hub notification sent after the code is forgotten can never be made, and the
  local sign-out still looks like it succeeded.
- **The generator was lower-casing enum variants.** `gen-ts-contract.py` emitted
  `v["rust"].lower()` under `rename_all = "camelCase"`, so `NotConfirmed` became
  `'notconfirmed'` while serde sent `notConfirmed`. Every unit variant until
  `HubOutcome` happened to be one word, which is why it survived: `Active` →
  `active` is right by accident. Fixed to camel-case, with a Rust test pinning the
  serde side and `--check` proven to fail on a hand-edited generated file.
- **The rule.** A "forget this" feature is only as complete as its most durable
  copy of the secret, and an operation authenticated by a bearer credential must
  never be able to destroy someone else's entitlement. Both halves are stated as
  tests, not as comments.

---

- **What broke.** The readiness probe's through-tunnel check called mihomo's delay
  route with `EGRESS_ATTEMPT_TIMEOUT.as_secs()` — the integer `5`. mihomo parses
  that field with `strconv.ParseInt(q.Get("timeout"), 10, 16)` and applies it as
  `time.Millisecond * timeout` (`hub/route/proxies.go`), so every delay test ran on
  a **5-millisecond** budget, which no round trip can meet. It answered
  `504 {"message":"Timeout"}` in about **7 ms**; the plugin maps a non-2xx to
  `Ok(ProxyDelay { delay: 0 })`; `delay_is_a_measurement` correctly rejects `0`.
  **Every member failed on every platform from 2026-10-02 until this fix.** Since
  `decide` reaches `Ready` only on `egress == Ok` or `traffic == Flowing`, the app's
  readiness had been resting entirely on the traffic websocket: the egress probe
  was not a second source of evidence, it was dead weight.
- **Why it survived a month.** Three reasons, none of them "nobody looked":
  `Timeout` is the exact string a dead tunnel produces, so every diagnosis was a
  guess and a wrong guess explained the symptom as well as the right one; the
  integration test passed a literal `5` with a parameter named `timeout_secs`, so
  the test and the code were wrong in the same direction and only the engine
  disagreed; and the single assertion that could have failed needed egress, so its
  redness was read as an environment problem and excluded.
- **The tell that was in hand the whole time.** A `504` in 7 milliseconds did not
  time out — its budget was too small to lose. The failure had been printed with
  its elapsed time for two days.
- **Reproduced before fixing.** Against `verge-mihomo v1.19.32`, through a real
  Shadowsocks-2022 outbound: `timeout=5` → `504 Timeout` in 0.007 s; `timeout=5000`
  → `200 {"delay":27}` in 0.029 s. The field is a **signed 16-bit** integer in
  milliseconds, so `32767` parses and `32768` is `400 {"message":"Body invalid"}`.
- **The fix.** `delay_timeout_millis` converts the budget with `as_millis` and
  clamps to `1..=i16::MAX`; the call site reads a single `DELAY_BUDGET_MILLIS`
  constant, which two compile-time `assert!`s bound. The test files stopped
  duplicating the wrong number.
- **Guards, each observed failing.** `the_delay_budget_is_sent_in_milliseconds_and_fits_mihomo`
  (asserts the value the **call site** reads — the first version asserted only on
  the helper and still passed with the defect restored, which is exactly the class
  of guard this log exists to warn about); the compile-time range assertions; and
  `the_delay_timeout_field_is_milliseconds`, an **egress-free** engine test that
  pins `32767`/`32768` against the real binary so a no-network runner cannot skip
  it into silence.
- **Three second-order defects found by fixing it, two of them in the new guard.**
  `tunnel_end_to_end.rs` also sent `timeout=5` and passed only because loopback
  round trips are ~2 ms — a test that was green for the wrong reason. And the new
  unit test failed **twice** for reasons that were not the unit: first it pointed
  the delay test at the engine's own controller, which is `503` while
  `Start initial compatible provider` is still running (`start_engine` returns on
  `/version`, before that); then — the real mistake — it asserted that a loopback
  target **yields a measurement**, through the `direct` outbound this file's config
  builds. A `direct` member reports `delay: 0` for loopback because loopback
  bypasses the proxy path, `0` is correctly not a measurement, and the guard went
  red in CI while the shipped behaviour was already fixed. The file had documented
  that exact fact at line 549 for two releases. The guard now asserts only the
  `32767`/`32768` parse boundary, which needs no round trip and cannot be defeated
  by a target.
- **The rule.** A wire field's unit is a contract, and both sides of an integration
  boundary can be wrong in the same direction while agreeing perfectly with each
  other. When a clock is involved, **measure what came back** — a timeout that
  returns faster than any network could is not a timeout — and pin the *engine's*
  acceptance behaviour, not the caller's intent.

---

## THE EGRESS TEST WAS BLAMED ON THE RUNNER FOR TWO DAYS, AND THE RUNNER WAS FINE (2026-11-04, unreleased)

- **What broke.** `a_reachable_member_yields_a_usable_delay` failed every
  `Build (linux)` run from 2026-11-02 onward. The recorded cause — a "torn
  runner" — was a guess nobody had measured, and the guidance it produced was
  "re-run the job".
- **Why the guess survived.** The test asserts on mihomo's
  `/proxies/<name>/delay`, which answers `504 Timeout` **both** when a dial fails
  and when it merely exceeds `timeout`. A runner with no network and a broken
  probe therefore return the same string, so every diagnosis was unfalsifiable
  from the failure message. `client/docs/UPSTREAM-CHANGES.md` had already written
  this trap down for the sibling tunnel test and it was not applied here.
- **The measurement, taken on the runner.** `curl` to both probe hosts returned
  `204` in 10 ms and 30 ms; `getent` resolved both; a plain GET proxied through
  the engine's mixed port returned `204`; the delay route returned `504 Timeout`.
  The runner has egress and the engine can dial from it. **The delay path is at
  fault.**
- **Two fixes attempted before the measurement, both wrong, both reverted or
  superseded.** Removing `ipv6: false` from the test config changed nothing; adding
  `dns.nameserver: [system]` changed nothing. Both were reasoned from the symptom
  rather than measured, which is the rule this entry exists to restate.
- **A third wrong turn worth naming.** A local reproduction appeared to implicate
  the engine's built-in resolver, because this workstation's own resolver answers
  those hosts with fake-IP addresses (`198.18.0.x`) that nothing can dial. Every
  local observation inherited that artefact, so local evidence was worthless for
  this fault and said so only after the runner's log showed a real address
  (`142.251.34.67`).
- **What the test now does about it.** The failure carries its own control — the
  HTTP code of a proxied GET through the engine's mixed port — and the last 40
  lines of the engine's debug log. `client.yml` gained a *Diagnose the runner's own
  egress* step. The next red run names the failing side instead of prompting a
  second investigation.
- **The rule.** A check that cannot distinguish "wrong" from "unreachable" will be
  resolved in favour of whichever explanation is cheaper and it will be wrong. Attach
  the control to the assertion, in the assertion's own failure message, where the
  person reading the red build will see it.

---

## THE §21 GUARD COULD NOT REPORT A TRUE RESULT, AND COULD NOT FAIL (2026-11-03, unreleased)

- **What broke.** CI run `41446549157` failed `Verify` on
  `BAD prepare_startup no longer calls require_install_for_session()` — against a
  tree where the call is present at `lifecycle.rs:508`. The same script on an
  identical clean checkout of the same commit passed locally, which is the detail
  that made it a defect rather than a flake.
- **The mechanism, reproduced before the fix.** The sub-check matched with
  `printf '%s' "$x" | grep -q PATTERN`. `grep -q` exits on its first match, which
  closes the pipe while `printf` is still writing; `printf` dies on SIGPIPE and,
  because `check-consistency.sh` runs under `set -uo pipefail`, the **pipeline**
  takes `printf`'s status (141) rather than grep's 0. A match therefore read as
  "not found". The log carries the footprint on the BAD line itself:
  `printf: write error: Broken pipe`. It is input-size dependent — 37 KB of
  stripped `lifecycle.rs` fits a pipe buffer on a desktop and does not on the
  runner — so the verdict depended on how much text preceded the match.
- **The second defect, found only by falsifying the first fix.** The check
  searched the whole file, and `prepare_startup` calls that function **twice**: the
  pre-existing `service_required`/TUN branch, and the macOS call the section
  exists to protect. Deleting the macOS call left the guard printing `OK`,
  satisfied by the TUN call 40 lines above it. **A guard that cannot fail**, which
  is the exact shape AGENTS.md rule 3 was written for; the previous proof suite
  missed it because its defect (`if false && …`) tripped the other sub-check.
- **Fix.** Matching is `case` (a builtin: no process, no pipe, no signal), and the
  search is **scoped** to the macOS call site, sliced from the
  `#[cfg(target_os = "macos")]` attribute by line, with a `WARN` — never a `BAD`
  — when the slice cannot be made. `test-contract-guard.sh` gained the case that
  keeps it scoped.
- **The rule.** A guard that matches by piping into `grep -q` under `pipefail` can
  report a match as a miss. And a search for "the code calls X" must be scoped to
  the call site that matters, or a second call elsewhere in the same function
  silently stands in for the one that was deleted.

---

## THE GEO DATABASES ARE NOW VENDORED (2026-11-03, unreleased)

- **What was still broken.** The four files were pinned by `meta-rules-dat`
  release-asset ID and fetched at build time. That is the dependency that failed
  five times in nine days (see the entries below). The sixth repin was made on
  2026-11-02 and CI failed again the same day — `Build (macos-arm)` could not
  fetch `geosite.dat` at all, and `Build (linux)` only got past it because the
  runner's own cache happened to hold the bytes. That is not a build; it is a
  cache that was warm.
- **What changed.** `ASN.mmdb`, `Country.mmdb`, `geoip.dat` and `geosite.dat` are
  committed under `vendor/build-inputs/resources/`. The `vendored.resources`
  entry in `sidecar.lock.json` already named that directory and had no bytes in
  it; it now does. `prebuild.mjs` needed no change — its vendored branch already
  wins over the URL — so the effect is that the network is no longer consulted at
  all for these four.
- **Verified.** Bytes match the existing pins exactly, so the lock is unchanged
  and no repin happened. `prebuild.mjs` with `HTTPS_PROXY` pointed at a dead port
  reported all four as *"is vendored in the repository and matches its pin"* with
  no `download finished` line naming them — the distinction that only a dead proxy
  makes visible. The real vendored engine then loaded them: `GeoSite cn records:
  110177`, `GeoIP cn records: 10012`.
- **What it costs.** 35.8 MiB of committed data, on top of the 464.7 MiB the
  vendored engines already spend. This is the trade `STILL-OPEN.md` recorded and
  the 2026-11-02 entry said should be made instead of a sixth repin.
- **What it does not fix.** The alpha engine and the stable engine are still
  fetched from upstream *when a checkout lacks the vendored copy*, and the
  `clash-verge-service` IPC bundle is still downloaded. Those upstreams are stable
  by comparison; the present defect was these four.

---

## THE GEO DATABASES WERE DELETED UPSTREAM A FOURTH TIME, AND THIS TIME IT WAS THE CONTENT THAT MOVED (2026-11-02, unreleased)

- **What broke.** Every `Build (platform)` job failed in *Fetch sidecars and geo
  data* with `Failed to download …/meta-rules-dat/releases/assets/683454259:
  status 404` (403 on macos-intel). All four `resources` pins — `ASN.mmdb`,
  `Country.mmdb`, `geoip.dat`, `geosite.dat` — returned 404: the whole release
  had been deleted, not just one asset.
- **Why it was not caught by a guard.** The pins were correct when written. This
  is a **world** fact — an upstream release is a thing that can be deleted — and
  no check in this repository can read it. `check-consistency.sh` §39 verifies the
  bytes *against the lock*; both agreed, and both were about a URL that had
  stopped existing. The first evidence is CI, which is why the `Verify` job is
  green while `Build` is red.
- **What is different this time.** The three earlier repins (see the 2026-10-28
  entries) replaced IDs for the *same* bytes. `meta-rules-dat` rebuilt the release
  on **2026-11-01**, so the new assets carry new content: `Country.mmdb` grew
  1290 bytes, `ASN.mmdb` 18994, `geosite.dat` 997, and `geoip.dat` *shrank* 20810.
  A repin is therefore not a safe no-op here — the size and hash changes are the
  review artifact.
- **How the new bytes were verified, rather than assumed.** All four were
  downloaded and checked with `file`, then loaded by the **real vendored engine**
  (`verge-mihomo v1.19.32`): `geosite.dat` was accepted as `Load GeoSite rule: cn
  … records: 110177`, and `Country.mmdb`/`ASN.mmdb` each logged `Load MMDB file`
  and `Load ASN file` while resolving real traffic through a `GEOIP,CN` rule.
  A stub or truncated download would fail to parse; these parse and match.
- **Resolved by a second repin the same day, which then also 404'd within the
  hour.** MetaCubeX republished again at 2026-11-02 01:42 and deleted the
  2026-11-01 IDs before CI finished. That makes five repins in nine days and
  settles the question this entry opened: repinning is not the fix. See
  `STILL-OPEN.md` — the next occurrence should be met with mirroring, not a sixth
  repin. The 2026-11-02 upload was byte-identical for three of four files
  (verified by sha256), with only `geosite.dat` moving.
- **The rule.** A pin buys reproducibility, not availability. When a build input
  is pinned by upstream asset ID, that ID is still an upstream fact that can be
  withdrawn, and the newest content is not necessarily the old content — verify
  the *behaviour* (the engine loads it) before repinning, not only the hash the
  lock will then record. `docs/reference/DEBUGGING-METHOD.md`'s "the artifact
  exists" is not the same claim as "the artifact works".

---

## THE VENDORING GUARD ACCUSED A CORRECT TREE, BECAUSE IT LOOKED IN THE WRONG PLACE (2026-11-01, unreleased)

- **What was being done.** The activation tunnel's client half, its guards, and a
  theme fix. Running the release gate — `pnpm check:release`, which is
  `check-consistency.sh` invoked from `client/` — reported
  `BAD the lock declares vendored inputs but vendor/build-inputs does not exist`.
- **Why.** §39b set `VENDOR_ROOT="vendor/build-inputs"` and read
  `server/data/sidecar.lock.json` **relative to the caller's working directory**,
  while the script's own usage line says it is runnable *from anywhere*. From
  `client/`, the `[ ! -d ]` test failed on a directory that exists, and the guard
  went on to accuse the code. Inside that block `Path.cwd()` and a relative
  `git ls-files` made the rest of it cwd-dependent too.
- **How it was caught.** By the failure disagreeing with itself: the same commit
  passed from the repo root and failed from `client/`, so the guard was reporting
  a property of the shell rather than of the tree. Reproduced deterministically
  by running both.
- **The fix.** Anchor the root to `$REPO`, derive the repository root from
  `git rev-parse` rather than assuming the cwd, and normalise every path to
  repo-relative before comparing — `git ls-files` prints repo-relative names, so
  comparing them with absolute paths would have marked *every* vendored file
  untracked. An untracked canary file dropped into `vendor/build-inputs/` still
  produces `BAD`, from either directory.
- **The rule.** A guard that reads a path relative to the caller inherits the
  caller's mistakes. `$REPO` is computed at the top of this script for exactly
  this reason; use it. And a guard's verdict must not depend on where it was
  invoked from — that is a property of the guard, never of the code it judges.

---

## THE VENDORED COPY WAS NEVER READ, AND ONLY A DEAD PROXY COULD TELL (2026-10-29, unreleased)

- **What was being done.** Vendoring the build inputs — the eight binaries the
  client build needs, which three upstream deletions in one week had proved
  cannot be left to a URL. The bytes were committed to
  `vendor/build-inputs/`, the lock grew a `vendored` table, and `prebuild.mjs`
  was given a branch that copies the committed file into place and skips the
  download.
- **What the first proof reported.** Every file was present, every hash matched,
  and `prebuild` printed *"verge-mihomo is vendored in the repository and matches
  its pin"*. A green line, the right bytes, and **the wrong provenance** — the
  same run's log also contained `download finished:
  …/mihomo-linux-amd64-v2-v1.19.32.gz`. It had gone to the network.
- **Why.** `lockedSidecarVendored()` gated the vendored branch on a **per-entry**
  `vendored: true` flag that only the alpha carried. The new `vendored` table —
  which is what says *why, when and where* — was read by the helper that builds
  the path and by nothing that decides whether to take the branch. Adding
  `verge-mihomo` to the table therefore changed a path nobody consulted.
- **How it was caught, and why nothing cheaper would have.** Checking the files
  proved nothing: the downloaded bytes and the committed bytes are *identical by
  construction*, so every hash in every check agreed. The only observation that
  separates the two is the one that makes the download impossible — `HTTPS_PROXY`
  pointed at a dead port — and the `download finished` line that then appeared
  for a file that could not have been fetched.
- **Second defect, same change, found the same way.** `sidecar-lock.py --check`
  only looked in the gitignored `client/src-tauri/sidecar/`, so a fresh clone —
  the exact state the vendoring exists to serve — reported all four already
  committed alphas as `MISSING`. A check that fails on a correct tree teaches its
  reader to ignore it.
- **Third, and the narrowest.** The alpha had been committed in
  `client/src-tauri/sidecar/` **and** was now committed in `vendor/`. Two
  committed copies of the same large binary can disagree, and the build bundles
  whichever the copy step wrote last. The first was untracked.
- **The fix.** One function answers "is this vendored" for sidecars and for
  resources; `--check` verifies whichever copy exists, committed first; and both
  claims are now asserted by `test-contract-guard.sh`, each observed failing
  before being kept.
- **Rule kept.** *A vendored file must be proven to have been read, not merely to
  have the right hash.* Two identical copies cannot be told apart by content, so
  a test that only compares hashes will report green while the build does
  something else entirely — and "matches its pin" printed by a build that
  downloaded it is the most convincing possible way to be wrong.
- **The CI version of that proof was wrong twice, in the two ways a proof can be
  wrong.** Both were caught by running it against a tree known to be correct and
  refusing to accept red as "the tree is wrong":
  1. **It ran from the repository root.** `prebuild.mjs` resolves every path from
     `process.cwd()`, so from the root it read `../server/data/sidecar.lock.json`
     as the lock, found nothing, and fell back to `releases/latest` — announcing
     `v1.19.33`, a version the lock does not pin, and downloading both engines.
     The step now pins `working-directory: client`.
  2. **It searched for the wrong string.** `prebuild` logs
     `"verge-mihomo" is vendored …` — the SIDECAR name — while the check searched
     for the TARGET FILE (`verge-mihomo-x86_64-unknown-linux-gnu`) inside the
     quotes. It could never match, so it reported both engines as unvendored on a
     tree where both demonstrably were. Three attempts at "fixing the pattern"
     chased quoting and regex quirks before the actual mistake surfaced: **the
     assertion described what the log would say if the program had been written by
     whoever wrote the assertion.**
- **Verified failing, then passing.** With the vendored branch removed
  (`if (vendoredInputFor(targetFile)) return true` deleted — the real bug, put
  back deliberately), the step exits 1 naming the engine; restored, it exits 0.
  A check that cannot fail would have printed the same thing for both.

- **Fourth, found while committing: nine of eleven.** The root `.gitignore`
  excludes `*.exe` to keep build output out, and that rule silently excluded the
  two Windows engines — so `git add -A` staged 9 of the 11 vendored binaries and
  the tree still looked vendored. Nothing local catches it: the bytes are on disk,
  every pin passes, and every build on the committing machine succeeds. A fresh
  clone is the only state that differs, and it would have downloaded its Windows
  engine again. `check-consistency.sh` §39b now asserts that every vendored file is
  **tracked**, which is the check the eye cannot do.
- **And the guard for it crashed silently on first writing**, which is the same
  defect one layer down: a literal NUL byte written into the script raised
  `ValueError: empty separator`, the caller read empty output as "no problems", and
  the section printed `OK`. Fixed to split on `chr(0)` explicitly and to report a
  non-zero inner exit as `BAD`. **A guard whose crash is indistinguishable from its
  pass is worse than no guard, because it also launders trust.**

## THE ALPHA ENGINE'S TAG WAS RE-ROLLED, AND ONLY ONE PLATFORM SURVIVED (2026-10-29, release 3.3.4)

- **What broke.** With the engine pin fixed, `Build (linux)` passed and the other
  three failed fetching the alpha core:

  ```
  mihomo-darwin-arm64-go122-alpha-6780eee.gz: status 404
  ```

  MetaCubeX re-rolled the `Prerelease-Alpha` release to `alpha-ca8d7c4` and
  deleted every `alpha-6780eee` asset. This is the same failure
  `STILL-OPEN.md` recorded on 2026-10-26, and the fix from then had a gap: only
  **linux-x86_64** was vendored into the repository.
- **Why that gap cost three platforms.** `tauri.conf.json` lists
  `sidecar/verge-mihomo-alpha` in `externalBin`, so every platform needs the
  file. Linux had a committed copy and built from it. macOS and Windows had
  nothing but the dead URL, so the one platform that was vendored was the one
  platform that survived — vendoring a single platform converts a fleet outage
  into a single-platform success and hides the rest.
- **The fix.** `engine_versions.mihomo_alpha` is now `alpha-ca8d7c4`, and the
  lock records the alpha's sha256 for all four shipped triples. The vendored
  linux binary was replaced so it, too, reports `alpha-ca8d7c4` — a vendored
  sidecar is never re-downloaded, so a stale one fails the pin forever.
- **Hashes are of the extracted binary, not the archive.** `assertPinned`
  compares the file that lands in `sidecar/`, so the pins were computed from the
  decompressed `.gz` and the unzipped `.exe`. Pinning the transport would have
  passed locally and failed in CI for a reason that reads like a corrupt
  download.
- **What was still not solved at the time.** macOS and Windows remained
  network-dependent: they were pinned, which makes the fetch *checkable*, not
  *reproducible*. Vendoring all four was the only fix that does not depend on a
  third party not re-rolling a tag.
  **Closed the same day** — all four triples are committed under
  `vendor/build-inputs/`, and the CI step below proves the build reads them. See
  the entry at the top of this file, and
  [`../operate/VENDORED-BUILD-INPUTS.md`](../operate/VENDORED-BUILD-INPUTS.md).

## THE ENGINE PIN WAS HALF-WIRED, SO THE ENGINE DRIFTED AND EVERY BUILD STOPPED (2026-10-29, release 3.3.4)

**This is the real reason `v3.3.4` first produced nothing.** The geo-database
404 above was the first failure; once repinned, the builds still failed, at a
different file, for an unrelated reason.

- **What broke.** Every `build` job died with:

  ```
  "verge-mihomo" does not match its pin:
    got 07e79c1eb24cab62, lock pins 90529d00b84df144
  ```

  The pin was **right** and the download was **right**: they described different
  engines. `sidecar.lock.json` recorded `"engine_versions": {"mihomo": null}`, and
  `getLatestReleaseVersion()` reads a null pin as "no pin — ask the network":

  ```js
  const pinned = lockedEngineVersion('mihomo')
  if (pinned) { META_VERSION = pinned; return }   // taken for alpha…
  // …falls through to releases/latest for the stable engine
  ```

  So the *hash* was pinned to the bytes of **v1.19.32** while the *version* was
  re-resolved from `latest` on every run. When upstream published v1.19.33 the
  two disagreed and the build refused to proceed. The alpha core was pinned
  (`alpha-6780eee`) the whole time; only the stable one was left null.
- **Established by direct download, not by inference.** Fetching
  `mihomo-linux-amd64-v2-v1.19.32.gz` hashes to `90529d00b84df144…` — exactly the
  recorded pin — while `v1.19.33.gz` hashes to `07e79c1e…`. The pinned binary
  reports `v1.19.32` from its own `-v`. Three independent facts agree.
- **Why it looked fine for months.** Any machine with a warm
  `node_modules/.verge/` cache short-circuits before the pin is consulted, so
  local builds passed while every cold CI runner failed. That is the same
  "a local green run is evidence about your working tree" trap `RELEASING.md`
  §3.3.1 already records.
- **The fix.** `engine_versions.mihomo` is now `"v1.19.32"`, so the lock decides
  which engine to fetch and the network only decides where to get it — the
  behaviour the code's own comment already claimed.
- **Observed failing, then passing.** With the lock temporarily pinned to
  `v1.19.33` and the sidecar deleted (the CI condition), prebuild reproduced the
  CI error exactly and left the previous binary in place; with the correct pin it
  downloads and verifies cleanly. A guard that cannot fail would have shown the
  same output for both.
- **Rule kept.** *A pin has to pin the thing that is chosen, not just the bytes
  that arrive.* The repository already knew this for one engine and not the
  other, which is why the defect survived: half a pin reads like a pin.

## THE PINNED GEO DATABASES 404'D, AND THE BUILD STOPPED RATHER THAN SHIP NEW DATA (2026-10-29, release 3.3.4)

- **What broke.** Both `Build (linux)` and `Build (windows)` failed at "Fetch
  sidecars and geo data". Not a code defect: all four assets pinned in
  `server/data/sidecar.lock.json` returned **404**. MetaCubeX republished the
  rolling `latest` release on 2026-10-28 (release name `Release 2026-10-28 09:30`),
  and a republish mints new asset IDs and deletes the old ones.
- **The pin worked, in the sense that matters.** It refused to build instead of
  silently downloading whatever `latest` pointed at. That is the property the
  lock was built for, and this is the first time it earned it.
- **What the pin could not tell us.** Whether the republish was cosmetic or a
  real data change. Answering it required fetching the new bytes and hashing
  them: three of four were re-uploaded **byte-identical** (sha256 still equal to
  the old pins), and only `geosite.dat` had changed — 4,307,247 → 4,353,539 bytes,
  a genuine upstream routing-data update.
- **The fix.** All four asset IDs repinned and `resources_release` advanced to
  `Release 2026-10-28 09:30`. Three sha256 values are unchanged, so the diff
  isolates the one file whose content moved. Re-downloaded through
  `prebuild.mjs`, which reported "on disk does not match its pin — re-downloading"
  and then verified the new bytes; `sidecar-lock.py --check` is green.
- **Rule kept.** `STILL-OPEN.md` had already named the residual risk two days
  earlier — "an asset ID is only as durable as the repository that hosts it" —
  and it was realised immediately. A predicted risk that is written down is
  cheaper than one that is discovered, but only if it also says what to do. It
  now says: expect to repin whenever `resources_release` moves, and mirroring
  these four files into this project's own release assets is a defect to fix,
  not an optional hardening.

## CI COULD NEVER PASS, BECAUSE THE GUARD PROOF NEEDED A FILE CI NEVER FETCHED (2026-10-29, release 3.3.4)

**Found by cutting the release, not by reading the workflow.** `verify-live.sh`
claim **A4** ("a release is published") was red, and the reason is the whole
entry: the run for `v3.3.3` failed, so no release was ever created for that tag
and the hub went on serving 3.3.2.

- **What broke.** `test-contract-guard.sh` proves the §39 build-input pin can
  FAIL by substituting a mihomo core that is still a valid gzip and asserting
  `sidecar-lock.py --check` rejects it. The test needs that sidecar on disk. The
  `Verify` job never fetched one — the sidecars are a gitignored download that
  only the four `build` jobs performed — so every run printed

  ```
  MISSED a substituted engine (§39) — the sidecar is not present, cannot test
  ```

  and the script exits 1 on any `MISSED`. **The release path could not produce a
  green run at all**, on any commit, and nothing said so: the last *successful*
  run predates this test.
- **Reproduced before fixing.** Hiding the sidecar locally reproduced the exact
  `MISSED` line and exit 1; restoring it (lock check green) made the full proof
  pass, 10 assertions caught. The fix is therefore aimed at the observed failure,
  not at a guess about it.
- **The fix.** `Verify` now caches and fetches the build inputs before the proof,
  and the two steps are ordered **after `pnpm install`** — `prebuild.mjs` imports
  `adm-zip`, `glob`, `https-proxy-agent` and `tar` at module scope, so placed
  earlier it fails with `Cannot find module`, which `continue-on-error` would
  then have rendered identical to the missing-sidecar case this step exists to
  repair.
- **Rule kept.** *A guard that cannot run is not a green guard — and a test that
  skips when its input is absent is a check that cannot fail.* The §39 branch
  already reported `MISSED` rather than skipping, which is why the defect was
  visible at all; the missing half was giving it the input.

**A second CI gap, found the same way: the suite CI ran was not the suite.**

- **What broke.** `sysopt.rs` quoted a log line a student actually saw, indented
  four spaces:

  ```
  ///     Failed to compensate proxy apply failure (… lock preferences) with direct mode.
  ```

  Rustdoc reads a four-space-indented block as Rust, tried to compile it as an
  expression, and failed on the `…` character. The crate's doc-tests therefore
  did not compile **at all**.
- **Why nothing caught it.** CI runs `cargo test --all-targets`, which excludes
  doc-tests. That is cargo's documented behaviour, not a mistake in the line —
  but it meant a broken doc-test could sit in a green tree indefinitely, and this
  one did. The 2026-10-26 entry above claimed the suite "Verified"; what was
  actually run was `--lib` and `--all-targets`, and neither reaches a doc-test.
  The claim was true about what it ran and misleading about what it covered.
- **The fix.** The quote is now a fenced `text` block, so rustdoc renders it and
  does not compile it. `cargo test --doc` is green.
- **The guard.** `Doc-tests` is a new linux-only step in `build (linux)`, next to
  clippy and `cargo fmt --check`. Without it the fix is a one-off: the next
  indented quote in any doc comment reintroduces the same failure silently.
- **Rule kept.** *A suite is only as complete as the command that runs it.*
  `--all-targets` reads like "everything"; it excludes doc-tests, and the gap
  between a flag's name and its behaviour is where this defect lived.

## 3.3.3 — THE WINDOWS TLS FIX SHIPS, AND THE BUILD BECOMES REPRODUCIBLE (2026-10-26, release)

- **What went out.** The `locus::hub` TLS fix from 2026-10-18, which sat in the
  tree labelled *unreleased* for eight days and three commits. The student it was
  written for was still affected; the fix was correct and never shipped. That gap
  is the one this entry exists to name: work is not done when it is in `main`.
- **Proven before tagging, not after.** Both live-hub tests pass on 2026-10-26 —
  the bundled-roots fallback verifies `networkingguides.jadedns.uk/api/health`,
  and the platform-store path does too, so the probe is not proving a tautology.
  A **Windows** machine was still not available: that this fixes *the reporter's*
  specific machine remains structural reasoning, not an observation.
- **Build inputs are now data.** The mihomo engine, the service bundle and the
  four geo databases are pinned by sha256 in `server/data/sidecar.lock.json`;
  `prebuild.mjs` takes the version *from the lock* rather than from
  `releases/latest`. The four databases are pinned by immutable release-asset ID,
  because `meta-rules-dat` publishes only a moving `latest` tag — observed live
  when `Country.mmdb` changed between two runs minutes apart.
- **The alpha engine is vendored.** `tauri.conf.json` requires
  `sidecar/verge-mihomo-alpha` in `externalBin`, and upstream deletes old alphas,
  so its URL is not stable. It is committed and hash-verified. Before this the
  client would not compile at all with a cold cache.
- **Rule kept.** A build that cannot be reproduced from a cold cache is a build
  that has never been reproduced. Two of the three inputs here were only
  discoverable by clearing the cache and trying, which is why the failure was
  invisible for as long as it was.

## THE GUARDS THEMSELVES HAD FOUR DEFECTS, AND ONE OF THEM WAS BLIND (2026-10-26, unreleased)

A run of the full check suite from a cold tree found no defect in the product and
four in the things that check the product. Recorded together because they share a
shape: **a check that reads correct and does not do what its comment says.**

- **`cargo clippy -- -D warnings` failed on two lints, so the gate could not
  pass.** `logging_error!` expanded to a bare `if let` statement with a trailing
  semicolon, which is rejected in expression position
  (`semicolon_in_expressions_from_non_local_macros`) — the macro's only use there
  was a `match` arm. And `AtomicU64::fetch_update` is deprecated in favour of
  `try_update`. The macro now expands to a block, which is correct in both
  positions; the atomic uses a CAS loop, because `try_update` is newer than the
  crate's `rust-version = "1.95"` floor and the loop compiles on both.
  *Rule:* a macro used as an expression must expand to a block. A `-D warnings`
  gate is only a gate if it has been run against the toolchain CI actually uses.
- **`test-contract-guard.sh` judged its own patch step by the wrong status.**
  Both helpers ran `python3 … <<PY` and then tested `[ $? -ne 0 ]` — but `$?` at
  that point was the heredoc **assignment**, not the patch, so a patch that
  silently failed to apply was reported as caught. It now captures the status
  immediately.
- **The same script's restore check could pass on files it never checked.** The
  fingerprint was `grep "  $f\$"`, which treated the path as a regex (dots
  match anything) and reported OK for an unmatched file; there was also a dead
  `want=` line whose value was computed and never used. It now snapshots the
  files to a temp directory *before* touching them and compares with `cmp`, so a
  file that was not snapshotted fails loudly instead of passing. **Observed
  failing**: skipping one restore produced three `BAD` lines, where the old code
  produced one.
- **`check-consistency.sh` §7 declared `TRIAGED_BASIS` — the documented,
  per-path list of files whose older version is correct as written — and never
  applied it.** The filter was `grep -vE "$TRIAGED_BASIS"`, which is an Extended
  regex, so `\.` matched any character and the anchors behaved as alternations of
  substrings. It did not crash; it silently did not exclude what the comment above
  it says it excludes. Replaced with an exact-path list and `grep -vF`, plus the
  container-directory prefixes as a regex. The report is unchanged on this tree,
  which is the point: the list is the mechanism, and it was inert.
- **Nothing checked `cargo fmt`.** `client/rustfmt.toml` has been the style since
  the fork and no workflow ran it, so 82 files were unformatted. All were
  formatted here and a `cargo fmt --check` step was added next to clippy.
  *Rule:* formatting is a function of the toolchain and must be bound to the same
  toolchain, or it drifts into a commit nobody can review.
- **`prebuild` did not restore the executable bit on a sidecar that already
  matched its pin.** The chmod lived at the bottom of the download path only, so
  every early return — "matches its pin, skipping download", "already exists
  (UNPINNED)", and the vendored path, which chmods *after* its return — left the
  file as it was. `verge-mihomo` was found with the pinned sha256 to the byte and
  mode 0644. The bytes were right, so the lock check passed; the file could not
  execute, so all five engine integration tests panicked with
  `PermissionDenied` and the two engine suites read as failures in a tree whose
  content was correct. Mode is not content, so the hash pin cannot cover it; it
  is now asserted on every path, by `ensureExecutable()`.
- **The engine tests' skip check tested `is_file()` and then `spawn`ed.** A
  present-but-0644 sidecar passed the check and panicked inside `start_engine`,
  blaming the test. Both suites now treat "not executable" as "not fetched",
  which prints the `SKIP` naming `prebuild.mjs` — observed skipping after
  `chmod 644`, and observed passing (5/5, 10.2s) after the bit was restored.
  *Rule:* a precondition a test relies on must be checked in the form the failure
  takes. `spawn` needs an executable, not a file.
- **No guard existed for the goja builtin trap.** `admin_console.pb.js` refused
  every release because it called `atob`, which PocketBase's goja does not
  define; the crash was caught and turned into `false` for every input. The fix
  was already committed but nothing prevented it recurring. §6(f) now fails on
  `atob`/`btoa`/`TextDecoder`/`TextEncoder`/`Buffer.*` in any hook, ignoring
  comments (the file explains the trap in prose at the call site). Observed
  failing against a reintroduced `atob(`. `STILL-OPEN.md` was corrected in the
  same change — it still described the fix as uncommitted.



- **What broke.** A 3.3.1 client failed activation with `invalid peer certificate:
  Expired`; the certificate had not expired. The response to the N4L block had never
  been finished — `hub_alias` was opt-in and empty, so every client was still
  pinned to the blocked hostname.
- **The fix.** The canonical hub moved; the legacy name is served, not redirected;
  the console, the heartbeat and the seeder agree on the new host. §32 turns the
  two-hostname rule into a guard.
- **Rule kept.** A defect whose mechanism cannot be observed from either end is
  recorded as *not established*, not as a theory that was convenient.

## ACTIVATION FAILED ON WINDOWS WITH "INVALID PEER CERTIFICATE: EXPIRED", ON EVERY ATTEMPT (2026-10-18, release 3.3.3)

- **What broke.** A Windows student could not activate, on either plan, on every
  attempt: `client error (Connect): invalid peer certificate: Expired` against a
  hub certificate issued that morning and valid until 2027-01-16. The log said
  `failed to verify TLS certificate: invalid peer certificate: Expired` four
  times — once per verification attempt.
- **What it was not.** The hub. Measured: `/api/health` returns 200, the leaf is
  valid, the served chain is complete (leaf → `YE1` → `Root YE` → `ISRG Root X2`
  → cross-signed by `ISRG Root X1`), and both TLS 1.2 and 1.3 verify against
  OpenSSL and Python on a machine with a current trust store.
- **The mechanism.** `locus/activation.rs` built its own `reqwest::Client`, and a
  bare builder uses `rustls-platform-verifier`, which on Windows reads **only the
  Windows certificate store**. The chain's top anchor is `ISRG Root X2`, an ECDSA
  root that reaches Windows machines solely through root-certificate update
  servicing. `rustls` does not chase the certificate's AIA extension, so a machine
  without that root has no path to an anchor — and `webpki` reports that as
  `InvalidCertificate(Expired)`. The message accuses the hub's certificate of
  something that is false, which is why it read as a hub outage.
- **The fix.** One shared client in `locus::hub`, used by activation, heartbeat
  and the update download: try the platform store, and on a **certificate** error
  only, retry with the compiled-in `webpki-roots` store, which ships with the
  binary and needs no OS update. The failure is logged with the store that
  refused, so the next support log names the cause instead of borrowing rustls's
  word for "no path to an anchor". §36 keeps a hand-built client out of the
  hub-facing modules.
- **The fix's own near-miss.** The bundled-roots client first used
  `rustls::ClientConfig::builder()`, which **panics** unless exactly one crypto
  provider feature is enabled — this crate enables only `ring`. A live test
  caught it; without network, `both_clients_build_without_panicking` now catches
  it. The path that exists to rescue a broken machine would have crashed the
  screen it was meant to fix.
- **Proven, not assumed.** `the_bundled_roots_verify_the_live_hub` fetches the
  real hub through the fallback and passes; the platform path does too on a
  machine with a current store. **Unverified:** that this is the specific
  reporter's mechanism — no Windows machine was available, so the chain of
  inference is structural. The observation that would settle it is the absence of
  `ISRG Root X2` in that machine's root store.
- **Rule kept.** A policy copied into several places drifts, and the copy that
  drifts is the one nobody re-read. Three hub clients had diverged on the only
  setting that matters; `utils::network` had the fallback and none of the others
  did.

## 3.3.1 SHIPPED, AND THE THREE THINGS THAT STOPPED IT (2026-10-17, release)

- **What broke.** No defect in the shipped build; three defects in the road to it
  (a cache that blocked the release job among them).
- **Rule kept.** CI must be *able* to fail loudly before a release means anything.

## A REPO-WIDE SCAN: A RETRY THAT NEVER RETRIED, AND THREE CROSS-LANGUAGE GAPS (2026-10)

- **What broke.** One real defect (a retry wrapper that could never retry) plus
  three checks that could not fail.
- **Rule kept.** Capability must be called, not awaited; an await on a non-promise
  discards the return value.

## THE FREE PLAN'S VERIFIER COULD NOT SEE THE FREE PLAN (2026-10, hub scripts)

- **What broke.** `verify-ss2022.sh` checked one retired UoT listener against a
  config the per-plan split had deleted, and had no concept of the free plan's
  listener — a dead free tier passed every check.
- **The fix.** The verifier now names each per-plan unit and pairs `eco.json` with
  `sing-box-uot-eco`; §31 asserts that pairing.
- **Rule kept.** A check that cannot fail, guarding the one pairing a student
  would notice.

## THE HUB HOSTNAME WAS UNREACHABLE ON A SCHOOL NETWORK, AND NOTHING COULD REPOINT IT (2026-10, hub + client)

- **What broke.** `HUB_URL` was compiled in, and the same hostname serves
  activation, heartbeat **and** `/updates/*` — so a network that refuses that name
  removes the ability to activate *and* the ability to download the fix.
- **The fix.** A hub alias (`HUB_ALIAS_DOMAIN`, 308 to the canonical domain) plus
  a client override (`contract::resolve_hub_url`) delivered through the heartbeat,
  the only channel that reaches an installed device without a release.
- **Rule kept.** A failure that prevents escaping the failure is the worst
  topology; a second delivery path is not redundancy, it is escape.

## A CONCURRENCY GUARD WAS REPORTED AS A REFUSAL ON A FINE MACHINE (2026-10, client)

- **What broke.** "Configuration validation is already running" was surfaced as a
  refusal, abandoning the connect it was in the middle of.
- **The fix.** `apply_tier` distinguishes "no verdict" from "rejected".
- **Rule kept.** A lock held is not a verdict on the thing it locks.

## THE macOS BUNDLE CARRIED AN ENTITLEMENT IT COULD NOT HONOUR (2026-10, client packaging)

- **What broke.** `com.apple.security.inherit` was true on a bundle without
  `app-sandbox`, and `application-groups` named a team the bundle does not belong to.
- **The fix.** The plist claims only entitlements the bundle owns; §30 guards it.
- **Rule kept.** An entitlement is a claim the OS enforces; a false one is a
  signing-time lie that surfaces at runtime.

## THE CREDENTIAL RECOVERY LEFT FIVE CLAIMS FALSE, AND ONE GUARD COULD NOT FAIL (2026-10-05, docs + a guard)

- **What broke.** A recovery pass left five world claims asserting state that no
  longer held, and a guard that reported OK while checking nothing.
- **Rule kept.** A guard that crashes or short-circuits reads exactly like a guard
  that passed. Prove it fails before keeping it.

## THE DOC SET WAS GREEN AND FRAGMENTED: TWO INDEXES, LYING TITLES, RETIRED MATERIAL (2026-10-05, docs)

- **What broke.** Two competing indexes, a `status: live` document announcing its
  own retirement, and retired material living in live folders — all green.
- **The fix.** One index ([`../README.md`](../README.md)), status as structure, and
  §28 to hold both.
- **Rule kept.** Status belongs to structure, not to a banner a document writes
  about itself.

## §8 WAS SILENTLY BROKEN, AND THE STALE-PATH SCAN NEVER LOOKED AT CODE (2026-10-05, guards)

- **What broke.** §8 scanned only `.md` files, so two code comments pointed at a
  moved document for a year. Widening it broke it silently: joining two generators
  with `+` raised `TypeError` and §8 reported OK while checking nothing.
- **Rule kept.** When you widen a guard's scope, re-prove that it can fail. See
  [`../reference/DOC-AUDIT.md`](../reference/DOC-AUDIT.md).

## THE DOCS DESCRIBED REMOVED MECHANISMS AS LIVE, AND TWO PROSE COPIES OF A CAP WERE WRONG (2026-10-05, docs + guards)

- **What broke.** Documents described removed mechanisms as current, and a guard
  *required* `README.md` to restate the tier caps — enforcing the duplicate it
  existed to prevent.
- **The fix.** The guard now asserts the negative: no cap string in `README.md`;
  the cap has one home backed by `state.toml`.

## A MACOS STUDENT WAS OFFERED NOTHING, AND NOTHING SAID WHY (2026-10-04, v3.2.27)

- **What broke.** The update path had no macOS payload, and the failure was silent.
- **Rule kept.** A missing remedy must say it is missing — see
  [`../operate/RECOVER-MACOS-UPDATE.md`](../operate/RECOVER-MACOS-UPDATE.md).

## THE FREE TIER'S ADVICE FIELDS COULD BREAK THE WHOLE HEARTBEAT, AND FOUR OTHER PRE-EMPTIVE FIXES (2026-10-04, v3.2.26)

- **What broke.** A malformed advice field could fail the entire heartbeat rather
  than the field, taking the paid path down with the free one.
- **Rule kept.** A degradation path must not be able to fail the path that pays.

## THE FREE TIER HAD NO ENFORCEMENT, AND THE BUSINESS PLAN READ AS IF IT DID (2026-10-04)

- **What broke.** The plan described enforcement that did not exist in the code.
- **Rule kept.** [`../business/17-not-built.md`](../business/17-not-built.md) is the
  anti-drift guard: anything not built is listed there.

## macOS SHIPPED A TERMINAL-ONLY REMEDY: A README THE STUDENT COULD NOT ACT ON (2026-10-04)

- **What broke.** The remedy for a macOS failure was a text file telling the
  student to run commands.
- **Rule kept.** A remedy a customer cannot act on is not a remedy.

## A NEW LOOKUP STATUS NAME LEFT REINSTALLING STUDENTS UNABLE TO RE-ENTER THEIR CODE (2026-10-03)

- **What broke.** A hub-side status name the client did not know left reinstalled
  students unable to re-enter their code.
- **Rule kept.** Every new status name is a wire contract: both sides must learn it
  in the same change, and the client's unknown-status default must be permissive.

## A CODE WAS BOUND TO A RE-DERIVED HARDWARE HASH, SO AN UPDATE COULD TELL A STUDENT THEIR OWN CODE BELONGED TO SOMEONE ELSE (2026-10-02)

- **What broke.** The `boundFp !== incomingFp` guard fired correctly — the device
  presented a different value, because a reinstall could not re-derive it.
- **The fix.** Device binding was **removed**, not repaired; the durable identity
  work that followed became [`client/docs/IDENTITY-MIGRATION.md`](../../client/docs/IDENTITY-MIGRATION.md).
- **Rule kept.** Never make a student's entitlement depend on a value you can only
  compute once.

## THE "CONNECTING" BUG — THE PROBE ASKED THE GROUP, WHICH ANSWERS FOR A MEMBER (2026-10-02)

- **What broke.** The readiness probe asked the *proxy group* for a delay; a group
  reports the delay of whichever member it selected, so a working tunnel could
  read as not ready, and vice versa.
- **Rule kept.** Probe the thing you are claiming about, not a handle that
  describes it.

## THE "CONNECTING" BUG CAME BACK A THIRD TIME, AND THE TEST PINNING IT WAS STALE (2026-10-02)

- **What broke.** The regression test still encoded the old probe's shape, so it
  passed against the defect it was written for.
- **Rule kept.** A test pinning a fix must be re-proved when the code it describes
  is rewritten.

## THE CODE WAS "ALREADY IN USE ON ANOTHER DEVICE" AFTER A REINSTALL, AND AUTO-SIGN-IN NEVER FIRED (2026-10-02)

- **What broke.** Reinstall produced the stale-binding refusal above, and the
  automatic sign-in path never ran.
- **Rule kept.** The recovery path for a refusal needs its own test; "the happy
  path is green" says nothing about the path a student actually takes.

## "THE UPDATE TO 3.2.18 IS NOT AN INSTALLER" — THE CLIENT REFUSED A GENUINE INSTALLER (2026-10-02)

- **What broke.** The client rejected a valid signed installer.
- **Rule kept.** An installer check must be loosened by evidence, and re-proved
  against a real signed artifact — not against a fixture.

## RECOVERY FROM A LOST SERVICE OWNER RESET THE WRONG PLATFORM'S PROXY (2026-10-01)

- **What broke.** A recovery routine keyed on the wrong platform and rescinded the
  wrong thing.
- **Rule kept.** A recovery is platform-specific; make the platform explicit at
  the call, never inferred.

## DEVICE RECOGNITION WORKED, AND THE STUDENT WAS LEFT ON THE CODE PROMPT FOR FIVE MINUTES (2026-10-01)

- **What broke.** Recognition succeeded but nothing advanced the UI.
- **Rule kept.** A successful background result must have a path to the surface.

## DEVICE IDENTITY WAS NOT DURABLE — CODES "STOPPED BEING RECOGNISED", AND A REINSTALL LOST THEM (2026-10-01)

- **What broke.** The identity lived in the app config — the file an uninstall
  deletes — so a reinstall made the device a stranger to its own code.
- **The fix.** A machine-scoped stored identity, reused verbatim, with an
  app-config fallback that **reports that it does not survive** a reinstall.
- **Rule kept.** Never let the UI over-promise persistence a storage tier cannot
  deliver. Detail: [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

## A GUARD THAT WAS CITED IN SIX DOCUMENTS DID NOT EXIST (2026-10-01)

- **What broke.** `activation_contract_test` was named in six live documents and
  two code comments as pinning the 403/409 refusal contract. It did not exist.
- **The fix.** `client/src-tauri/tests/activation_contract.rs` now reads the hook
  source and asserts the literal codes and messages the client classifies on, and
  a meta-guard makes the claim self-checking.
- **Rule kept.** A guard described in prose but absent is worse than no guard: it
  stops anyone from checking.

## THE WINDOWS INSTALLER WAS NEVER SIGNED: TWO DEFECTS IN ONE STEP (2026-10-01)

- **What broke.** The signing step silently did nothing; the installer shipped
  unsigned.
- **Rule kept.** A signing step must be shown to change its input, or it is a
  comment.

## NO WINDOWS CLIENT COULD UPDATE: THE HUB SERVED THE RAW BINARY, NOT THE INSTALLER (2026-10-01)

- **What broke.** The Tauri updater executes **any** PE as an installer, so the hub
  must publish the NSIS installer for Windows — it published the raw `.exe`.
- **Rule kept.** Two-sided contracts are asserted as *agreement*, not per side. The
  ordered recovery is [`../operate/RECOVER-WINDOWS-UPDATE.md`](../operate/RECOVER-WINDOWS-UPDATE.md).

## THE BUTTON SAT ON "CONNECTING" WHILE THE TUNNEL WAS CARRYING TRAFFIC (2026-10-01)

- **What broke.** The egress check judged the tunnel on **one** 3 s round trip and
  classified `delays.values().any(|&d| d > 0)` — accepting mihomo's failure and
  timeout sentinels as measurements, and flipping a working tunnel back to
  "connecting" on a single transient miss.
- **The fix.** A bounded retry (two attempts, 5 s each, 12 s shared deadline), one
  classifier (`delay_is_a_measurement`) mirroring the frontend's `classifyDelay`,
  a 1.5 s coalesced cache keyed to `core_started`/`core_stopped`, and a re-entrancy
  guard on the status poll.
- **Rule kept.** Two implementations of one rule are free to drift; the sentinel
  case must be one function, and every sentinel gets a test **shown to fail**.
- Detail: [`EGRESS-READINESS.md`](EGRESS-READINESS.md).

## THE APP RE-EXECUTED ITSELF INSTEAD OF UPDATING, AND SHOWED AN EDGE "FILE NOT FOUND" (2026-09-30)

- **What broke.** The app installed a copy of the *running* executable outside its
  install directory and relaunched that.
- **Rule kept.** An updater must install into the location the OS will launch from,
  and the check for that is `strings … | grep -c 'a/Locus/Locus'` returning `0`.

## THE WINDOWS INSTALLER ABORTED WITH AN EDGE "FILE NOT FOUND" DIALOG (2026-09-30)

- **What broke.** A raw Windows path was interpolated into build JSON, so the
  installer referenced a file that was never produced.
- **Rule kept.** Never interpolate a Windows path into a JSON build input (§13).

## THE UPDATE DOWNLOADED INTO RAM AND KILLED THE APP (2026-09-30)

- **What broke.** The updater buffered the whole payload in memory; on a large build
  it exhausted the process.
- **Rule kept.** The update path uses the streaming downloader (§12), and an update
  payload is never read into a buffer.

## THE GUARD THAT REFUSED EVERY RELEASE: goja HAS NO `atob` (2026-09-30)

- **What broke.** A guard written against a browser-style primitive did not exist in
  the PocketBase hook runtime and refused every release.
- **Rule kept.** Confirm the runtime's real capability before building on it.

## VOLATILE FACTS WERE ASSERTED AS PROPERTIES, AND NOTHING NOTICED (2026-09-29)

- **What broke.** A present-tense IP address and similar facts had gone stale
  silently.
- **The fix.** The claim classes and the dated register —
  [`../operate/CLAIMS.md`](../operate/CLAIMS.md); §10 fails a build that inlines one.

## UPDATES UNINSTALLABLE, AND "CONNECTED" WITH NO INTERNET (2026-09-29)

- **What broke.** A false `ready` was reported on a machine with no usable egress
  (the mirror image of the 2026-10-01 entry above).
- **Rule kept.** Both directions of the readiness judgement need a test: a false
  ready and a false not-ready fail the student differently.

## TERM-BASED ACCESS, RENEWAL, AND ONE CODE PER DEVICE (unreleased)

- **What broke.** Subscriptions had no term model; the code carried no
  materialised expiry.
- **The fix.** `term_days` on the code, materialised into `expires_at`, with
  renewal and an audit trail.
- **Not shipped as designed:** the one-code-per-device half was later removed. The
  design is [`../business/redesign/`](../business/redesign/00-README.md); the live
  model is [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

## THE UPDATE COULD NOT BE INSTALLED: THE SIGNATURE WAS UNDECODABLE (2026-09-29)

- **What broke.** The signature was not the base64 of the whole `.sig` file.
- **Rule kept.** The wire format of a signature is a contract; §1b asserts it.

## EXPIRY INVISIBLE TO THE UI, AND THE UPDATE TOGGLE WIRED TO THE WRONG SETTING (2026-09-28)

- **What broke.** An expired code looked identical to a working one, and the update
  toggle wrote a setting nothing read.
- **Rule kept.** A setting with no reader is dead code wearing a switch.

## MIGRATED THE CIPHER STACK TO SHADOWSOCKS 2022 (2026-09-28)

- **What changed.** SS2022 replaced the legacy cipher stack.
- **Rule kept.** The cipher is defined once (§4); ten copies with nothing binding
  them is how the previous one drifted.

## A SUSPENDED CODE SAID NOTHING, AND THE UPDATER WAS DEAD CODE (2026-09-28, third round)

- **What broke.** Suspension was indistinguishable from a network failure, and the
  updater path was never reached.
- **Rule kept.** Every refusal state needs a distinct, actionable message.

## THE BUTTON STILL LIED, THE EXPIRY STILL DID NOTHING, AND THE CHECK CAME OUT (2026-09-28, second round)

- **What broke.** The connect state was rendered from a value that did not reflect
  the tunnel.
- **Rule kept.** Render connection state from one backend field, never from a
  frontend guess.

## SUBSCRIPTIONS NEVER EXPIRED, AND NOTHING TOLD THE STUDENT THEY WERE ABOUT TO (2026-09-28)

- **What broke.** Expiry existed in the schema and nowhere in the flow.
- **Rule kept.** A lifecycle state the UI cannot show does not exist for the user.

## FOUR HONESTY BUGS, AND THE SIDECAR THAT CAUSED TWO OF THEM (2026-09-27)

- **What broke.** Four surfaces reported something other than what was true; a
  sidecar process caused two.
- **Rule kept.** The app reports what the engine *said*, not what it hoped.

## WINDOWS CI COULD NOT CHECK OUT THE REPO — 62 FILES WITH A COLON IN THE NAME (2026-09-27)

- **What broke.** Tracked filenames contained Windows-illegal characters, so a
  Windows runner could not check out the repo.
- **The fix.** §5 fails the build on a Windows-illegal tracked path.

## THE TRAFFIC DISPLAY WAS TOO THIN, AND "CONNECTED" COULD NOT BE TRUSTED (2026-09-27, 3.2.0)

- **What broke.** Both directions of the connection surface — traffic and status —
  were guesses.
- **Rule kept.** A status indicator is a claim; it must be traceable to a
  measurement.

## CONNECT DID NOTHING, AND THE APP NEVER ASKED FOR ELEVATION (2026-09-27)

- **What broke.** The connect path needed elevation and never requested it.
- **Rule kept.** Request elevation before the operation, not after it fails.

## THE APP NOW LOOKS LIKE LOCUS, NOT LIKE CLASH VERGE REV (2026-09-26)

- **What changed.** The rebrand: the shipping two-tab UI —
  [`../../client/docs/FRONTEND.md`](../../client/docs/FRONTEND.md).

## THE STUDENT'S REPORT: "THE APP IS NOT WORKING AND THE REBRANDING IS INCOMPLETE" (2026-09-26)

- **What broke.** A customer-visible list of failures, several of them rebranding
  leftovers.
- **Rule kept.** A customer report is a specification; work from it, not from the
  changelog.

## THE BLANK WINDOW, PART 2: THE LOADING OVERLAY COVERED THE ACTIVATION SCREEN (2026-09-26)

- **What broke.** A second, independent blank-window path: the overlay never
  dismissed.

## THE CLIENT RENDERED A BLANK WINDOW — ASSET URLS RESOLVED AGAINST THE PROTOCOL ROOT (2026-09-26)

- **What broke.** Relative asset URLs resolved against the protocol root, so the
  bundle loaded nothing.
- **Rule kept.** §16: no absolute `frontendDist` is compiled into the client.

## "LOCUS HAS NO LINUX ELEVATION PATH" WAS A CLAIM ABOUT THE RETIRED CLIENT, NOT THE FORK (2026-09-26)

- **What broke.** A limitation of the retired client was read as a constraint of
  the live one.
- **Rule kept.** Always ask *which* system a sentence is about before believing it
  ([`../operate/CLAIMS.md`](../operate/CLAIMS.md) §3).

## `/api/hiddify` HAD NEVER WORKED, AND ITS `all=1` LEAK WAS NEVER EXPLOITABLE (2026-09-25)

- **What broke.** An endpoint that never worked, with a suspected data leak that
  was not reachable.
- **Rule kept.** Distinguish "broken" from "exploitable" before spending an
  afternoon on either.

## RELEASE MACHINERY REMOVED, NOT REPAIRED (2026-09, current)

- **What changed.** Root `VERSION`, `bump.sh`, the syso scripts and
  `.github/workflows/build.yml` versioned the **retired** Wails client. They were
  deleted rather than fixed.
- **Rule kept.** The client has its own version sites and its own CI —
  [`../operate/RELEASING.md`](../operate/RELEASING.md),
  [`../operate/CI-CD.md`](../operate/CI-CD.md). Never re-add a root version file.

## RELEASE TAGGING — THE .syso WENT STALE AGAIN, AND THE GATE THAT KNEW (2026-09-20)

- **What broke.** A regenerated build resource was not re-committed before tagging.
- **Rule kept.** Generated artifacts that ship must be regenerated in the same
  change that invalidates them.

## CLIENT UPDATE — THE PORTABLE BINARY COULD NOT REPLACE ITSELF (2026-09-20)

- **What broke.** A running portable binary cannot overwrite itself on Windows.
- **Rule kept.** Self-update on Windows goes through an installer, not a file copy.

## CLIENT TUNNEL/RECOVERY — NORTON KILLS THE SOCKET; THREE STATE BUGS IT EXPOSED (2026-09-20)

- **What broke.** An antivirus killed the tunnel socket, and the state machine had
  no path back from it.
- **Rule kept.** Recovery must be reachable from every state the app can enter —
  including the ones only a third party can put it in.

## RELEASE TOOLING — VERSION DRIFT AND A PUBLISH THAT WROTE AN UNUSABLE ROW (2026-09-19, later pass)

- **What broke.** The tag and the version files disagreed, and a publish wrote a
  row no client could use.
- **Rule kept.** `pnpm release-version`, then commit, then tag — in that order;
  the tag must name the version being built.

## ELEVATION — "ELEVATED COPY DID NOT HAVE PERMISSION" ON EVERY CONNECT (2026-09-19)

- **What broke.** The elevated copy lost the setup it needed on the way up.
- **Rule kept.** An elevated child must be handed its state, not rediscover it.

## TIERS PAGE — STALE WARNING CONTRADICTED THE STRIKE DESIGN (2026-09-19)

- **What broke.** A warning in the console contradicted the shipped tier design.
- **Rule kept.** Console copy is documentation; it rots the same way.

## UDP-OVER-TCP WAS NEVER CONNECTED, AND THE .syso SHIPPED STALE (2026-09-19, later pass)

- **What broke.** `uot_port` on the hub and `server_port_uot` on the client — one
  value, two names, a silent no-op. UoT was dead fleet-wide while both sides were
  individually "correct".
- **Rule kept.** Frozen wire names (§2). Two sides agreeing on a value is asserted
  as agreement, never per side.

## TWO PUBLISHERS DESCRIBED update_config DIFFERENTLY (2026-09-19, later pass)

- **What broke.** Two publish scripts wrote `update_config` inconsistently, and one
  handed a Windows client a Linux binary.
- **Rule kept.** `download_*` → `update_*` is a frozen name (§1, §35); both writers
  are asserted to agree.

## CONSOLE RELEASES — AUTOMATIC ARTIFACT VERIFICATION (2026-09-19)

- **What broke.** The console trusted that a published artifact matched its row.
- **The fix.** The console verifies the artifact automatically before showing it as
  live.

## RELEASE PIPELINE — `manifest.json` WAS NEVER PRODUCED (2026-09-19)

- **What broke.** The release pipeline named a manifest it never wrote, so the
  updater had nothing to read.

## CLIENT UPDATE SYSTEM — BUILT END-TO-END + PB HIDDEN BREAKAGE (2026-09-19)

- **What broke.** The end-to-end update path exposed PocketBase breaking on fields
  it silently discarded.
- **Rule kept.** A hook writing a column that does not exist looks implemented and
  records nothing — the `unbound_at` / `unbind_reason` defect (§3).

## SECRETS WIRING & BACKUP VERIFICATION — TWO MORE BUGS (2026-09-19, later pass)

- **What broke.** Two defects in the secrets wiring and the backup verification that
  only a restore could reveal.
- **Rule kept.** A backup is verified by restoring it, not by writing it.

## BLANK-VPS DEPLOY — SEVEN BUGS FIXED (2026-09-19)

- **What broke.** A deploy to a blank box surfaced seven defects that a re-run over
  an existing box had hidden.
- **Rule kept.** A deploy procedure is proved on a blank box; skip-if-exists steps
  hide exactly the defects a fresh install has.

## THE LOG WAS TRIMMED: THE RETIRED CLIENT'S HISTORY IS ONE SECTION NOW (2026-10-05, docs)

- **What broke.** ~1,090 lines described a client that no longer ships, in a log a
  reader takes as the record of *this* product.
- **What was done.** The eleven pre-2026-08-17 entries were replaced by the durable
  rules they produced, which are stated where they are enforced. Nothing was
  deleted from git.
- **Rule kept.** A date is not enough of a label for "retired"; the folder is — the
  same rule [`../README.md`](../README.md) rule 1 applies to documents.

---

## The rules this log produced, in one place

Each is enforced somewhere in the tree rather than remembered:

| Rule | Enforced by |
|---|---|
| Agreement between two sides, not each side | §1, §24, §31, §32, §33, §34, §35 |
| A guard must be shown to fail | [`DEBUGGING-METHOD.md`](DEBUGGING-METHOD.md) |
| No volatile fact in a sentence | §10, [`../operate/CLAIMS.md`](../operate/CLAIMS.md) |
| No statement of a fact twice | §9, [`../STATE.md`](../STATE.md) |
| The code wins over the document | §8, [`../README.md`](../README.md) rule 2 |
| A remedy the customer can act on | [`../operate/RECOVER-WINDOWS-UPDATE.md`](../operate/RECOVER-WINDOWS-UPDATE.md), [`../operate/RECOVER-MACOS-UPDATE.md`](../operate/RECOVER-MACOS-UPDATE.md) |
| A committed fix is inert until redeployed | [`DEBUGGING-METHOD.md`](DEBUGGING-METHOD.md), [`../../AGENTS.md`](../../AGENTS.md) |
