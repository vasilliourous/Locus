# CI/CD — the client pipeline

```
audience:    human-operator
status:      live
authoritative-for: the client CI pipeline (.github/workflows/client.yml)
verified-against: .github/workflows/client.yml
```

> **This pipeline replaced one that never built the shipping client.** Do not
> re-add a root `VERSION`, a root `bump.sh`, or committed `.syso` resources: they
> versioned the retired client, and the shipping client's version sites are
> `[client.version_sites]` in [`../state.toml`](../state.toml).

---

## The client pipeline

**Jobs:** `verify` → `build` (4 platforms) → `release` (tags only), plus `preview`
(pushes to `main`) and `report` (always).

**Where it must live:** `.github/workflows/` at the **repository root**. GitHub
ignores a workflow placed under `client/.github/`, which is why the fork's
earlier plan to put one there would never have triggered.

### What triggers a run — read this before assuming a preview exists

`on.push` is **path-filtered**:

```
client/**   server/**   scripts/**   docs/**   AGENTS.md   README.md
.github/workflows/client.yml
```

A push outside those paths **starts no workflow at all**. That is indistinguishable
from success by looking at the tree, and it is how a person comes to believe "push
to main publishes a preview" while their change was in a path that cannot start a
run. `scripts/**` was added on 2026-09-29 and `docs/**` + `AGENTS.md` + `README.md`
on 2026-10-17, each time after a commit silently produced no run.

**Docs are in the list because the pipeline checks documents**: §8 (dead
cross-references), §25 (front-matter) and §28 (index reachability) all fail the
build. A docs-only commit that skips CI is one whose most likely defect is never
tested — and that is not theoretical: the 3.3.1 doc commit was rejected by §8 for
two files that existed only in a working tree.

**`pull_request` carries the identical list.** The two were the same before
2026-10-17 and would otherwise drift one edit at a time.

**A `v*` tag push ignores `paths` entirely.** GitHub applies path filters to branch
pushes only, so a release is never blocked by this filter — only the preview is.
That asymmetry is why a tag build can succeed on a commit whose `main` run never
started.

### Billing stops CI before any job code runs

**Added 2026-10-17, from a real outage.** The repository was **private on a Free
plan**, where GitHub Actions has **no included minutes**. Every billing SKU read
`$0.00 / 100% Usage blocked` and `verify` died in seconds with:

```
##[error]Failed to CreateArtifact: Artifact storage quota has been hit.
           Unable to upload any new artifacts.
```

**That message is misleading and cost an hour.** It names artifact storage, so the
obvious remedy is deleting artifacts — and deleting all 1.9 GB of them changed
nothing, because the repository then held **0 bytes** and runs still failed. The
real cause was the account-level block, not the size.

What made it fatal rather than cosmetic: `verify` uploads `frontend-dist`, every
build job *downloads* it, and `release` downloads artifacts too. So the failure
cascaded — no `frontend-dist`, four failed builds, no release.

**Diagnose it like this:**

```sh
gh api repos/:owner/:repo -q .private          # true + Free plan = no minutes
gh api repos/:owner/:repo/actions/artifacts -q '[.artifacts[].size_in_bytes]|add//0'
gh api repos/:owner/:repo/actions/caches -q '.actions_caches|length'
gh api repos/:owner/:repo/actions/caches -q '[.actions_caches[].size_in_bytes]|add//0'
```

If the artifact number is small (or zero) and uploads still fail, look at the
**caches before concluding it is billing** — added 2026-10-18 from a recurrence.

**Caches and artifacts share one storage allowance.** On this repo the artifacts
were 0 bytes and the caches were **22 entries / ~7.4 GB**, almost all of it four
`rust-*-build` caches at 0.9–2.2 GB each. That alone exceeds the Free plan's 500 MB,
so every upload failed on a repository that stored no artifacts at all. Delete them
by id — the collection endpoint requires a `key` parameter, so "delete all caches"
is a loop:

```sh
gh api repos/:owner/:repo/actions/caches -q '.actions_caches[].id' |
while read -r id; do gh api -X DELETE "repos/:owner/:repo/actions/caches/$id"; done
```

> **Deleting them does not unblock immediately.** Usage is recalculated every
> **6–12 hours**, so the next run still fails with the same message and it is easy
> to conclude the deletion did not work. It did; the meter has not moved.
> Confirm with `gh api /users/<user>/settings/billing/usage` and look for
> `Actions storage` on this repository.

**The durable fix is the `purge-caches` job** (added 2026-10-18, at the bottom of
`client.yml`). `Swatinem/rust-cache` has no size limit of its own — it waits for
GitHub's 10 GB eviction, which a 500 MB allowance never reaches — so the workflow
now deletes caches older than seven days every Monday.

It runs on `schedule` and `workflow_dispatch` **only**, never on a push: on a push
it would race the builds writing those caches. That is also why the purge is not
tied to a release — by the time a release fails on a full quota, the upload that
needed the space has already gone.

**Remedy:** restore billing, or make the repository public — public repos get
Actions minutes and storage on the Free plan, which is what unblocked 3.3.1.

**Artifact retention cleaning is still worth doing, for a different reason.** Each
release run leaves ~800 MB (seven artifacts across four platforms). They are not
needed afterwards — the GitHub Release carries its own copy of every asset, and
deleting Actions artifacts does **not** touch a published release. Clear them once
a release is confirmed:

```sh
gh api repos/:owner/:repo/actions/artifacts -q '.artifacts[] | "\(.id)"' \
  | while read -r id; do gh api -X DELETE repos/:owner/:repo/actions/artifacts/$id; done
```

**Do not do this before the release job has run** — it downloads those artifacts to
assemble the release.

**Do not infer that a preview was published — ask.** Every run writes a `report`
job summary stating what it actually published, read back from the GitHub API. Read
that rather than a green tick. `server/scripts/verify-live.sh` claim A3 checks the
preview independently.

### Runner images are pinned, on purpose

The build matrix uses `ubuntu-24.04`, `windows-2022` and `macos-14` — **not**
`*-latest`. A moving label is a world claim nobody re-checks: this project already
lost every Intel macOS run to the retirement of `macos-13` ("Waiting for a runner
to pick up this job", for every run, while the other three platforms finished, and
nothing automated noticed). Pinning means a retirement fails loudly. **When you bump
a label, update this note and the comment in `client.yml`.**

### macOS runners are refused by the account, and a label change does not fix it

**Open, as of 2026-11-07.** Every macOS job is rejected in about two seconds with
zero steps run:

```
The configuration 'macos-14-arm64-us-default' is not supported
```

What has been **ruled out**:

- **Not a change to this repository.** The last green macOS run (`41746038397`,
v3.3.5, 2026-11-04) used `macos-14` as well, and nothing in the workflow changed
between that run and the first rejection. The build went red without a commit.
- **Not a minutes quota.** This account billed `Actions macOS 3-core` in every
month on record — 281 minutes in 2026-11, 1628 in 2026-10 — and the repository is
public.
- **Not a missing image.** `actions/runner-images` carries both
`images/macos/macos-14-Readme.md` (x64) and `macos-14-arm64-Readme.md` (arm64).
- **Not fixable by a label edit.** `macos-14-x64` was tried on 2026-11-07 (run
`42342311491`). It is not an image name: the jobs sat **queued with no runner**
rather than being rejected, which burns the job timeout instead of failing fast.

The label `macos-14` currently resolves to the ARM configuration
`macos-14-arm64-us-default`, and that configuration is refused for this
repository. Billing shows only the `Actions macOS 3-core` (x64) SKU, so whatever
grants the ARM configuration is not what it was on 2026-11-04. **The fix is on the
account side** — Settings → Billing, or GitHub support: ask which hosted macOS
configurations this repository may claim.

**This blocks releasing.** The hub's fetcher requires both darwin artifacts, so a
run without them produces nothing publishable.

A macOS job cancelled after roughly two seconds with **zero steps** is this, not a
build failure. A macOS job that sits **queued indefinitely** is a label that names
a configuration GitHub will not hand out for that label — `macos-14-x64`, tried on
2026-11-07, is exactly that: not an image name, so the job waits instead of
failing. A job that starts and then fails is a build failure. Read the job's
annotations before touching any build step.

The register entry is A22 in [`CLAIMS.md`](CLAIMS.md) §5.

### Which job runs which suite

Two jobs, two suites, and they are not interchangeable:

| Job | Runs | Notes |
|---|---|---|
| `verify` | `check-consistency.sh`, the tag/version check, `pnpm run web:build`, `pnpm run lint`, `pnpm test`, `test-contract-guard.sh` | **Frontend and docs only.** No Rust is compiled here. |
| `build` (linux only) | installs `sing-box` and `shadowsocks-rust`, then `cargo test --all-targets --profile fast-release` on a branch (no `--profile` on a tag), `cargo clippy`, doc-tests, `cargo fmt --check` | See "The Rust suite cannot skip" and "The same profile, or two full compiles" below |
| `build` (all four) | the platform build, signing and packaging | |

The Rust half was added 2026-09-29 because it existed and passed locally while
nothing in CI invoked it, which is how the updater's signature encoding mismatch
reached a real install. A clippy error now fails the gate; `cargo fmt` is still
not run.

> **`cargo test` is in the matrix, not in `verify`.** This note previously said
> the opposite, and gave a reason that the workflow contradicts: the claim was
> that the Rust suite is in `verify` "so a failure costs seconds rather than four
> cross-platform builds". It is in the matrix, so a Rust failure is found
> **after** paying for four builds — which is the cost the sentence was written to
> avoid. `verify` deliberately stays frontend-only so the fast gate is fast.
>
> Corrected 2026-10-01. Nothing behavioural changed; the document was wrong about
> the pipeline it describes. The lesson is that *what CI runs* is a claim like any
> other, and nothing recomputes it — see `../reference/DEBUGGING-METHOD.md` §1.3.

**What it produces**, because the hub's publish path resolves these by name and
requires every one of them:

| Asset | Notes |
|---|---|
| `locus-linux-amd64` | raw executables, for the auto-updater |
| `locus-windows-amd64.exe` | the raw PE, for the **retired portable client** only — no longer what a Windows client updates from |
| `locus-darwin-amd64` | the *filename* says darwin while the *platform key* says `macos_intel` — a frozen asymmetry, not an oversight |
| `locus-darwin-arm64` | |
| **`.sig` for each of the four** | minisign signatures; without them no client can install the update |
| `installer-Locus_<v>_x64-setup.exe` | the Windows NSIS installer — **this is what a Windows client installs from itself** |
| `installer-Locus_<v>_x64-setup.exe.sig` | its own minisign signature; minisign signs exact bytes, so the raw binary's cannot substitute |
| `manifest.json` | version + per-platform filename, SHA-256 and signature |

> **Windows updates from the installer, not the raw binary.** A Windows client does
> not run the payload it downloads — `tauri_plugin_updater` ShellExecutes it and
> accepts *any* PE — so being handed `locus-windows-amd64.exe` made an installed
> client relaunch a copy of itself outside its install directory (`FIXES.md`, "THE
> APP RE-EXECUTED ITSELF"). CI's `manifest.json` has named the NSIS setup
> executable since v3.2.12; the hub's `fetch-release.py` did not follow, and every
> Windows client refused the raw binary it was served. Nothing could update until
> the hub was corrected — `check-consistency.sh` §1a now pins the name on both
> sides of that contract.

**Signing is mandatory, not best-effort.** The Tauri updater verifies a minisign
signature over every download and offers no bypass, so a release published
without signatures is installable by nobody while looking perfectly healthy on
the hub. The build fails loudly if `LOCUS_UPDATE_KEY` is absent. See
`client/docs/SIGNING.md`.

**The secret is checked up front, in the cheap gate.** A missing
`LOCUS_UPDATE_KEY` is detected in `verify`, before any of the four platform
builds start, so it costs seconds rather than four ~15-minute builds. Note that
signing runs on **every push to `main`**, not only on tags — a branch preview also
needs the key. Actions secrets live on the repository and do **not** come with a
copied tree, so a migrated or recreated repo must have the secret re-set before
its first push; the exact command is in `client/docs/SIGNING.md` §4a.

**The raw binaries are staged by hand.** `createUpdaterArtifacts` is `false`, so
`tauri build` emits the *installed* forms (NSIS, `.app`) but not the bare
executables the updater consumes. The workflow copies the compiled binary
(`target/<triple>/release/locus[.exe]`) under the name the hub expects.

**Why a partial release is impossible:** `manifest.json` generation fails if any
binary or signature is missing, and a second check verifies every
`locus-*` artifact has a `.sig` before the release is created.

### The same profile, or two full compiles — and a cache key that was never reusable

**Added 2026-11-07.** A run took 39 minutes and the Linux job was 29 of them.
Both causes were in the log rather than in argument; the numbers below are from
runs `42346173855` (before) and `42416704701` (after).

**Cause 1 — `cargo test` ran in a different profile from `cargo build`.** The
comment above the test step used to claim the tests "reuse the target directory
`tauri build` just populated, so the compile is largely already done". They did
not, and never had. The `Build` step compiles into
`target/<triple>/fast-release`; `cargo test` with no `--profile` compiles into
`target/debug`. Those two share no artifacts, so the whole dependency tree was
compiled **twice**: ~630 crates on `Build`, ~660 more on the tests.

The tests now pass `--profile fast-release` on a branch build — the same profile
`Build` used — so the tree is compiled once. That profile is not an optimised one
being asked to behave like a debug one: `Cargo.toml` sets `debug = true` and
`opt-level = 0` for it, so assertions, backtraces and `debug_assert!` still work.
A **tag** build uses `release`, so the tests there run without `--profile` and
compile their own; that path is unchanged and deliberately so.

**Cause 2 — the cache was keyed on the GitHub job id.**
`Swatinem/rust-cache` has `add-job-id-key` defaulting to **true**, and a job id is
unique per run. So the key a run *saves* under can never be the key the next run
*looks up*. The action still logs `Cache restored successfully … full match:
true`, because it falls back to an older entry with the same prefix — and Rust
then **ignores that restored `target/`**, since the fingerprints inside it do not
match this build. A cache that is restored but never reused is worse than no
cache: it spends the download and buys nothing.

The workflow now sets `add-job-id-key: false` with an explicit
`shared-key: locus-${{ matrix.triple }}`. It stays per-triple because
`macos-intel` and `macos-arm` are independent compiles and would evict each other.

**Measured effect.** The first run after the change is cold — a new key has
nothing to restore — so the second run is the measurement:

| Step | Before | After (warm) |
|---|---|---|
| Whole run | 39 min | **21 min** |
| `Build` (linux step) | 29 m 07 s | **16 m 28 s** |
| `Rust tests` | 304 s | **67 s** |
| `Rust lints` | 96 s | 11 s |
| Linux Rust cache | 174 MB | **3,108 MB** |

180 MB of that before-figure was the *entire* Linux cache, against 1337 MB for
Windows on the same action and settings — the size difference alone was the
tell that one platform's cache was never holding a usable tree.

**The trap to remember.** A restored-but-unused cache and a genuinely warm cache
are indistinguishable from the action's own log line; both say the restore
succeeded. The difference is visible only in how many crates compile. If a run is
slow, count `Compiling` lines before blaming the build.

### The Rust suite cannot skip, and that is enforced from outside the tests

**Added 2026-10-29.** The engine suites had a silencing problem of two kinds, and
both made a green run mean less than it appeared to:

- **A skipping test is a passing test.** `tier_profile_engine.rs` and
  `egress_probe_engine.rs` called `eprintln!("SKIP: …")` and returned when the
  sidecar was absent, so `cargo test` printed `ok` either way. The three tests
  that prove *a working config is accepted* and the one that proves *a working
  tunnel reads as connected* could all be "verified" by not running.
- **The knob that would have failed instead was never turned.** The suites honour
  `LOCUS_REQUIRE_LIVE_ENGINE`, and no workflow set it.

Both are now closed in the Linux `build` job, which sets:

| Variable | Effect | Why here |
|---|---|---|
| `LOCUS_REQUIRE_LIVE_ENGINE=1` | every skip becomes a failure | `sing-box` is installed by the step above and `prebuild` has fetched the sidecar, so nothing in the engine suites is allowed to be absent |
| `LOCUS_REQUIRE_EGRESS=1` | "this runner has no network" becomes a failure for the one public-round-trip case | the same job demonstrably has egress — it fetched the registry and the build inputs one step earlier |

**The two requirements are not the same, and conflating them wasted the suite.**
Engine availability is a fact about the *run* (`prebuild` either ran or the job is
broken); network egress is a fact about the *runner*. They now have separate
variables, because one variable gating both meant every no-network test skipped on
any machine that had simply not run `prebuild` — throwing away the only part of
the suite that always works.

> **Measured 2026-11-03, after two days of blaming the runner.** `Build (linux)`
> had gone red on `a_reachable_member_yields_a_usable_delay` since 2026-11-02, and
> this file's own justification — "the job demonstrably has egress" — was treated as
> settling which side was at fault. It does not settle that, and the failure message
> could not either: mihomo returns `504 Timeout` both when a dial fails and when it
> exceeds `timeout`, so "the runner has no network" and "the probe is wrong" produce
> the same string.
>
> The runner, measured directly:
>
> | probe | result on the runner |
> |---|---|
> | `curl http://cp.cloudflare.com/generate_204` | `204` in 10 ms |
> | `curl https://www.gstatic.com/generate_204` | `204` in 30 ms |
> | the same GET through the engine's mixed port | `204` |
> | the delay route (`/proxies/<name>/delay`) | `504 Timeout` |
>
> So the runner has egress **and the engine can dial from it**; the delay path is
> what fails. The flag stays set — it is what stops the suite passing by not
> running — but a red run is now self-diagnosing rather than open-ended:
>
> - the test prints the engine's proxied-GET control code and its last 40 debug log
>   lines in the failure message;
> - a `Diagnose the runner's own egress` step `curl`s both hosts directly when the
>   suite fails.
>
> **Do not record a cause for this test without one of those two outputs.** The
> previous cause recorded here was a guess, and it cost two days.

> **RESOLVED 2026-11-04 — the delay path failed because the probe sent milliseconds
> as seconds.** The instrumentation above is what localised it; the cause was in the
> request, not the runner and not the engine. `probe.rs` passed
> `EGRESS_ATTEMPT_TIMEOUT.as_secs()` (`5`) to a field mihomo reads as **milliseconds
> in a signed 16-bit integer**, so every delay test ran on a 5 ms budget and
> answered `504 Timeout` in ~7 ms. `Build (linux)` was red because the test was
> **right**. See [`../reference/FIXES.md`](../reference/FIXES.md) and
> [`../reference/EGRESS-READINESS.md`](../reference/EGRESS-READINESS.md) §4b.
>
> Two lessons for this file specifically:
>
> - **"The job demonstrably has egress" settles whether the runner can dial. It says
>   nothing about whether the client asked correctly.** That was the inference that
>   kept this open, and it was made twice.
> - **A red run that is indistinguishable from an environment problem will be
>   treated as one.** The control and the engine log are what made it diagnosable;
>   keep them in the failure message rather than behind a re-run.
>
> The unit is now pinned by an **egress-free** engine test
> (`the_delay_timeout_field_is_milliseconds`), so a no-network runner can no longer
> skip the only assertion that covers this into silence.

**`sing-box` is installed, pinned, and its absence fails the step.** It is the
**hub's** Shadowsocks listener, so it is the only peer whose acceptance of this
client's outbound means anything — see
`client/src-tauri/tests/tunnel_end_to_end.rs` and
[`../reference/EGRESS-READINESS.md`](../reference/EGRESS-READINESS.md). A failed
download fails the step rather than skipping the test: the test is the reason the
job claims to prove the tunnel.

> **The positive-egress case retries a second host before failing.** Setting
> `LOCUS_REQUIRE_EGRESS` immediately produced `504 Timeout` against
> `cp.cloudflare.com` from the gate — because that is a *convenience* endpoint, not
> something a release should depend on. The case now tries `gstatic.com` too and
> names both on failure. One third party's availability was never the assertion.

### The speed optimisation, and the trap it opened (read before touching `frontendDist`)

To avoid rebuilding the byte-identical Vite bundle on all four build runners, the
pipeline builds it **once** in `verify`, uploads it as the `frontend-dist`
artifact, and each build job downloads it into `client/dist`. To make `tauri build`
reuse that bundle instead of rebuilding it, the build step sets an empty
`beforeBuildCommand` through `TAURI_CONFIG`:

```yaml
TAURI_CONFIG: '{"build":{"beforeBuildCommand":""}}'
```

**That override sets `beforeBuildCommand` and NOTHING ELSE. `frontendDist` must
not appear here.** This is the single most expensive mistake made in this file and
it is not obvious from the code, so it is written out in full.

`frontendDist` is not only a build-time input. `tauri::generate_context!()`
(`client/src-tauri/src/lib.rs`) resolves it at **compile time** and **bakes the
resolved path into the binary**, where it becomes the runtime asset root for
`WebviewUrl::App(..)`. So an *absolute* `frontendDist` — which is what a CI runner
needs to point at its own checkout — **ships inside the artifact**:

```jsonc
// the Shape that shipped v3.2.12 broken
{"build":{"beforeBuildCommand":"","frontendDist":"${{ github.workspace }}/client/dist"}}
// On the Windows runner github.workspace is D:\a\Locus\Locus, so the binary
// carries D:/a/Locus/Locus/client/dist as the directory it expects its web
// assets in.
```

On a student's machine that directory does not exist. Tauri resolves the start
page against a missing root, WebView2 renders Chromium's own page — **Edge logo,
`File not found`, `ERR_FILE_NOT_FOUND`** — inside a window still titled Locus. The
window is `.visible(false)` until page-load *Finished*, which never arrives, so the
app never appears at all, and nothing in the message names Locus, Tauri, or CI.
Because the bad value is in the **executable**, a restart changes nothing and a
reinstall changes nothing (it installs the same binary). Reported 2026-09-30 on a
**fresh install pulled from the release page** with a clean `verge.yaml`
(`start_page: /`) and `url=file:///D:/` in `latest.log`.

**The correct path is the relative one already committed.** `tauri.conf.json`
lives in `client/src-tauri/` and carries `"frontendDist": "../dist"`, which
resolves to `client/dist` — exactly where the download step places the bundle, on
every runner, with no path arithmetic and nothing to escape. Do not "fix" it to an
absolute path, and do not reintroduce the shell-time assembly
(`DIST=$GITHUB_WORKSPACE/...; node -e ...`) that once lived in the build step.

> **A separators fix is not a fix.** `17ef21a` normalised `D:\a\...` to `D:/a/...`
> to stop a JSON-escape panic (`invalid escape at line 1 column 54`) after the raw
> path was interpolated into JSON. That turned the build green and left the defect
> fully intact — the absolute runner path was still embedded. If a change makes the
> build pass, ask **why the value was there at all** before assuming it is fixed.

**Guarded** by `check-consistency.sh` §16: no *live* line of the workflow may set
`frontendDist`; the committed config must keep it relative; and the build job must
download the bundle to `client/dist`. The guard was shown to fail against all three
shapes it can be reintroduced in (the `ba01ed2` literal, the `17ef21a`
node-assembled form, and an absolute committed config).

**The guard proves the tree, not the artifact** — and the artifact is what ships. On
2026-09-30 the fix was confirmed by reading the built binary back: the raw
`locus-windows-amd64.exe` from run `36686775049` (`a8b0acf`) carries the relative
`../dist` where the broken build carried `d:/a/Locus/Locus/client/dist`, and
`strings … | grep -c 'a/Locus/Locus'` returns `0`. See `../reference/FIXES.md`,
`../reference/STILL-OPEN.md`, and `CLAIMS.md` §5 **A8**.

> **Nothing in CI performs that read-back.** It was done by hand, so the next
> platform-specific path leak ships the same way. A step that runs the built binary
> and asserts its page-load URL is not `file://` remains the missing mechanism.

---

## What still holds (independent of the removed workflow)

These contracts survived the pipeline and are honoured by the **hub** and the
**publish path**, which are still live:

### The release-asset → consumer contract

Whoever builds a release must produce these, because the update machinery on the
hub expects exactly this shape:

| Asset | Consumer | Notes |
|---|---|---|
| `installer-*` (`Locus_<v>_x64-setup.exe`, `.dmg`, `.deb`, `.rpm`) | **humans** | The supported way to install Locus. Windows NSIS also registers the system service (`StartVergeService`), and the app binary itself declares `requireAdministrator` (see `src-tauri/build.rs`), so it prompts for UAC at launch and does not depend on the service to reach TUN. |
| `installer-Locus_<v>_<arch>.zip` | **humans (macOS)** | The compressed, **ad-hoc-signed** `Locus.app` for each architecture. Exists for the one case the `.dmg` cannot serve: a download that arrives quarantined and refused. A zip does **not** avoid quarantine — a browser fetching it stamps the archive and the extracted `.app` inherits the flag — but it delivers a bundle that survives transport intact, so macOS reports the bypassable "unidentified developer" dialog (System Settings → Privacy & Security → **Open Anyway**) instead of *"damaged and can't be opened"*, which has no such button. **Not an updater payload**: the hub fetches it as a human download, and CI's manifest and the hub's `PLATFORMS` both refuse a `.zip` in an update slot (`check-consistency.sh` §1c). |
| raw `locus-<os>-<arch>[.exe]` | **auto-updater + hub** | Must be raw; the updater cannot unpack a zip. **NOT a human download** — it ships alone, so `sidecar("verge-mihomo")` cannot resolve and validation fails with `os error 2`. |
| `manifest.json` | updater + hub | Version + per-platform filename + SHA256 |

**Never hand a person the raw `locus-*` file.** It is an updater payload that
happens to live on the same release. Running it produces
`failed to run validation core "verge-mihomo" ... (os error 2)` because Tauri
resolves sidecars relative to the executable's own directory and the payload ships
with nothing beside it.

**The updater must never be handed an installer.** It always fetches exactly one
named raw executable, and the hub's `fetch-release.py` resolves assets from an
allowlist of the four raw binaries plus `manifest.json` — so installers and DMGs
are ignored by the update pipeline rather than needing to be excluded from it.
Adding a new packaging artefact therefore requires **no change to the hub or to
`update_config`**.

### Publishing never happens inside the build

A build must not be able to put a binary on students' machines by itself.
Publishing to the live hub is a separate, deliberate step
(`server/scripts/publish-release.sh`), and it holds at rollout 0 until verified
on real hardware. See `RELEASING.md`.

---

## Why it was removed (so it is not rebuilt by accident)

The archived client's CI was the source of the repo's tagging confusion:

- It triggered on `main` pushes and `v*` tags but built only the **retired**
  client — so tagging felt like "releasing the product" when it was not.
- The `Create Release` job was gated on `refs/tags/v*`; a branch-only push
  **skipped** it, and GitHub renders a skipped job as a **green check** — seven
  green checks, no release. Re-running the workflow replayed the same skip.
- The pipeline asserted a version spread across six files plus committed Windows
  `.syso` bytes; stale resources broke the 2.1.0 and 2.2.7 releases, and the
  guard built to catch it was bypassed rather than missing (`FIXES.md` 57).
- Broken tags remain: `v2.2.2` points at a commit that can never pass the (now
  deleted) lint gate, and `v2.2.4` never reached origin. They are reachable in
  git history and are listed here so they are not mistaken for releases.

The full removed pipeline (job graph, Wails build tags, artifact table, the
`--no-verify` trap) is preserved in git history at the commit that deleted
`.github/workflows/build.yml`.

### How the new pipeline avoids these

| Old failure | New safeguard |
|---|---|
| A skipped release job rendered as a green check | The release job is the *last* of three and depends on `build`; a skipped tag job cannot look like a completed release because nothing is uploaded |
| Version asserted across six files plus committed binaries | Version is pinned to `client/package.json`, and the tag drives the release name |
| A build that could publish a partial release | `manifest.json` generation fails on any missing binary **or signature**, and every artifact is checked for its `.sig` before the release is created |
| Signing assumed rather than required | The build fails outright if `LOCUS_UPDATE_KEY` is unset, because an unsigned release is installable by nobody. The secret is presence-checked in `verify` before any platform build, so the failure costs seconds; and because there is no tag condition on `build`, it is caught on the first push after a repository move |
| `--frozen-lockfile` drift (a package removed without regenerating the lockfile) | `verify` runs `pnpm install --frozen-lockfile` first, so the drift fails in seconds rather than at build time |

### The Windows-checkout trap

The Windows job is the one that fails if the repository contains a tracked path
with a character Windows forbids — most easily a colon, which is what Windows
`Zone.Identifier` ("downloaded from the Internet") markers become when a folder is
copied off an NTFS machine. `actions/checkout` then dies with
`error: invalid path '…:Zone.Identifier'`, before anything is built, and **only on
the Windows runner** — so the branch looks green everywhere else.

```sh
git ls-files | grep ':'    # must print nothing
```

Run that after any commit authored on Windows. `.gitignore` now carries
`*:Zone.Identifier` to stop the common case returning, but it cannot cover a
different forbidden character, so the check is the real guard. See `../reference/FIXES.md`
(2026-09-27) — including why the earlier history still cannot be checked out on
Windows and why it was left alone.
