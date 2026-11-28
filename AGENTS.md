# Agent guidelines — Chrysalis

Read this, then the one guide for the area you are touching. Nothing here is a
procedure; each rule links to the file that owns it.

## Where things are

| Path | What it is | Its guide |
|---|---|---|
| `client/` | The desktop app (Tauri 2 + React). Product logic in `src-tauri/src/chrysalis/`. | [`client/AGENTS.md`](client/AGENTS.md) |
| `server/` | The hub: PocketBase, Caddy, the Shadowsocks/BBR/tc stack, `pb_hooks/`. Also `server/site/` (landing page) and `server/console/` (operator console). | — |
| `docs/` | The documentation. [`docs/README.md`](docs/README.md) is the index. | — |
| `legacy/` | Retired client code. **Reference only — never build it.** | `legacy/wails-client/ARCHIVED.md` |

## Read by what you are doing

| If you are… | Read first |
|---|---|
| **Starting a working session** | [`docs/operate/SESSION.md`](docs/operate/SESSION.md) — the checkout→secrets→signing we have |
| **Cutting a release** | [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md) — the order is not optional |
| **Diagnosing a defect** | [`docs/reference/DEBUGGING-METHOD.md`](docs/reference/DEBUGGING-METHOD.md) — and check [`docs/reference/FIXES.md`](docs/reference/FIXES.md) §1, it may already be known-unverified |
| **Writing any fact into a document** | [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) — data vs world claim |
| **Deploying or operating the hub** | [`docs/operate/OPS.md`](docs/operate/OPS.md), [`docs/operate/DEPLOY.md`](docs/operate/DEPLOY.md) |
| **A ticket that says "the hub thinks I'm signed out"** | [`docs/operate/RUNBOOK-DESYNC.md`](docs/operate/RUNBOOK-DESYNC.md) — the ordered diagnosis. Start here, not at the code |
| **Touching `last_seen_at`, `stateLabel`, `livenessOf`, the heartbeat or the session probe** | [`docs/reference/AUTH-STATE.md`](docs/reference/AUTH-STATE.md) — entitlement × liveness, the six desync cases and the lease. Then [`docs/reference/adr/`](docs/reference/adr/) for why |
| **Touching the free/maintenance tunnel or the activation allowlist** | [`docs/reference/MAINTENANCE-TUNNEL.md`](docs/reference/MAINTENANCE-TUNNEL.md), [`docs/reference/ACTIVATION-TUNNEL.md`](docs/reference/ACTIVATION-TUNNEL.md) |
| **Changing the hub's data/schema** | [`docs/business/redesign/05-migration-and-live-data.md`](docs/business/redesign/05-migration-and-live-data.md) — the gating file |
| **Touching client Rust logic** | [`client/docs/UPSTREAM-CHANGES.md`](client/docs/UPSTREAM-CHANGES.md) (authoritative on ours-vs-upstream) |
| **Touching the UI** | [`client/docs/FRONTEND.md`](client/docs/FRONTEND.md) |
| **Touching the updater or signing** | [`client/docs/UPDATE-ARCHITECTURE.md`](client/docs/UPDATE-ARCHITECTURE.md), [`client/docs/SIGNING.md`](client/docs/SIGNING.md) |
| Anything not listed | [`docs/README.md`](docs/README.md) — the single index |

## Four rules that apply everywhere

1. **The code wins.** Where a document and the code disagree, the code is right
   and the document is a bug to fix in the same change.
2. **Know which kind of fact you are writing.** Recomputable from this checkout →
   data in [`docs/state.toml`](docs/state.toml), with a guard. About the deployed
   system → a dated world claim in [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md)
   §5. Neither belongs in a sentence. **Never write an IP address or a count of
   live customers.**
3. **A guard must be shown to fail** against the code it catches. A check that
   cannot fail reads exactly like one that passed.
4. **Say what you did not verify.** Verified / structural / unverified are
   different claims.

## Traps this repository has already paid for

One clause each; the file that owns it has the story.

- **The proxy group name must differ from the proxy name** (`Chrysalis Auto` vs
  `Chrysalis`) — mihomo treats a match as a reference loop and refuses the whole
  config, which is valid YAML. Only running the engine catches it.
- **mihomo's delay `timeout` is milliseconds, in a signed 16-bit field** — and
  `Timeout` is the same string a dead tunnel returns, so a 5 ms budget reads as a
  dead tunnel. A `504` that arrives in single-digit milliseconds never timed out.
  This shipped for a month ([`docs/reference/EGRESS-READINESS.md`](docs/reference/EGRESS-READINESS.md) §4b).
- **Frozen wire names** (`uot_port`, `download_*`→`update_*`, the
  `macos_intel`/`darwin-*` asymmetry, the `eco`/`free` and `strike`/`Full`
  splits) are contracts with deployed clients and the hub. Do not "tidy" them.
- **Linux runs three gates, and passing tests says nothing about the other two**
  — `cargo test`, `cargo clippy --all-targets --features clippy -- -D warnings`,
  and `cargo fmt --check`. A change with 685 passing tests went red on `Build
  (linux)` because it was never formatted. Run all three before pushing Rust.
  **There is a fourth, and it is a *cargo* gate** — `scripts/check-rust-deps.sh`,
  which fails on a `[dependency]` nothing uses. `-D warnings` is a rustc flag and
  cannot see `cargo::unused_dependencies`, so clippy prints that warning and exits
  **0**; the class sat unaddressed until the guard existed and immediately found
  one (`rust_iso3166`, unreferenced since the initial commit).
- **Do not stream a gate's output into your context — capture it and print the
  verdict.** `cargo test` on this tree is thousands of lines of `test ... ok`, and
  reading them to learn "everything passed" is the largest avoidable cost in a
  small change. The pattern is the same for all four gates:

  ```sh
  cargo test 2>&1 | tail -5            # the summary, not the 4000 lines above it
  cargo test >/tmp/t.log 2>&1 || tail -40 /tmp/t.log   # failures only when it fails
  ```

  The same applies to `check-consistency.sh`, which is quiet by default for this
  reason — a passing run is one line, and `--verbose` is for when you need the
  transcript. **Never pipe a gate through `head`** to trim it: `head` closes the
  pipe early, a writing command dies of SIGPIPE, and `pipefail` reports a failure
  that did not happen — a real defect in `client.yml`'s runner probe, explained in
  the comment above that step's `gh api` loop. Use `tail`, or redirect to a file.
- **A repo fix is inert until the piece that runs it is redeployed.** The hub runs
  its own copy; `server/scripts/hooks-sync.sh` compares them.
- **`main` and the tag are separate pushes**, and CI does not bump versions — a
  tag that disagrees with the version files builds binaries that misreport
  themselves. [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md).
- **Run `server/scripts/check-consistency.sh` — it is under `server/`, with
  `bash`, from the repo root.** Under `sh`/`dash` it aborts with "Bad
  substitution" and prints false `BAD` lines. `--verbose` prints every section's
  transcript; without it a clean run is one line.
- **A guard that quotes the string it bans must not read itself.** §4, §8 and §53
  each ban a literal and each must name it to explain itself; the exemption used
  to be the filename `check-consistency.sh`, which stopped matching the moment the
  sections were split into `server/scripts/checks/`. All three went red on their
  own evidence. The rule is `server/scripts/lib/guard_self.py`: a *comment* in the
  checks directory may name a banned string, a *code* line may not — so a guard's
  own pattern is built (`"2022-" + "blake3"`) rather than spelled.
- **Secrets do not travel with a repo copy.** A fresh clone has zero Actions
  secrets and every signed build fails until `CHRYSALIS_UPDATE_KEY` (or its
  `LOCUS_UPDATE_KEY` alias) is set.
