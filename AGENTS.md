# Agent Guidelines — the Locus repository

Instructions for AI coding agents working in this repository. Read this first,
then the guide for the area you are touching.

## Where things are

| Path | What it is | Its own guide |
|---|---|---|
| `client/` | The desktop app (Tauri 2 + React). Product logic in `src-tauri/src/locus/`. | [`client/AGENTS.md`](client/AGENTS.md) |
| `server/` | The hub: PocketBase, Caddy, the Shadowsocks/BBR/tc stack, `pb_hooks/`. Also `server/site/` (the public landing page) and `server/console/` (the operator console). | — |
| `docs/` | Live documentation. `docs/README.md` is the index. | — |
| `legacy/` | Retired clients. **Reference only — read it, never build it.** | — |

`docs/README.md` rule 3 applies everywhere: **the code wins.** Where a document
and the code disagree, the code is right and the document is a bug to fix in the
same change.

## Before you write a fact

Read [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md). A fact that can be
recomputed from this checkout is **data** and belongs in
[`docs/state.toml`](docs/state.toml) with a guard behind it; a fact about the
deployed system is a **world claim** with a date and a re-verify command. Neither
belongs in a sentence. Never write an IP address or a count of live customers.

## Before you diagnose a defect

Read [`docs/reference/DEBUGGING-METHOD.md`](docs/reference/DEBUGGING-METHOD.md).
Four defects have shipped here through fully green pipelines, so "the tests pass"
is not evidence that something works. The short version:

- **Reproduce against the live system** (`curl` the hub), not from the source.
- **A guard firing is not a guard being broken** — find which side is wrong first.
- **Two-sided contracts:** when CI and the hub must agree on a filename or wire
  key, assert the **agreement**, not each side separately.
- **Every new guard must be shown to fail** against the code it catches. A check
  that cannot fail reads exactly like a check that passed.
- **A repo fix is inert until the piece that runs it is redeployed.** The hub has
  its own copy of `fetch-release.py`; `hooks-sync.sh --fetch-service` compares them.
- **Say what you did not verify.** Verified / unverified / falsified / structural
  argument are different claims.

## Cutting a release — read this before you tag anything

**CI does not bump versions.** It labels the GitHub Release and its
`manifest.json` from the git tag name, while the number compiled into the
binaries comes from the version files. If the tag and the files disagree, CI
publishes binaries that misreport their own version, and every client is offered
an "update" that reinstalls the version it already has — forever.

That is not hypothetical: on 2026-09-30 the `v3.2.13` tag was pushed on a commit
whose version files all still said `3.2.12`, and the release shipped 3.2.12
binaries under a 3.2.13 tag.

**The order is not optional:**

```sh
cd client
pnpm release-version X.Y.Z          # writes ALL FIVE version sites
git add -A && git commit -m "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "Locus client vX.Y.Z"
git push origin main
git push origin vX.Y.Z              # the tag is what triggers the release build
```

- **`pnpm release-version` is the only supported way to bump.** It writes
  `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`,
  `Cargo.lock` and `../docs/state.toml`. Do not hand-edit those, and if a new
  version site ever appears, add it to `client/scripts/release-version.mjs` —
  not to your memory — so the next agent cannot forget it.
- **Check before you tag.** `bash server/scripts/check-consistency.sh` must exit
  0. Its §7 fails with `BAD client.version` when `docs/state.toml` disagrees with
  `client/package.json`.
- **Two guards now back this up**, both in
  [`.github/workflows/client.yml`](.github/workflows/client.yml): the `verify`
  job step "Tag must name the version being built" fails any tag whose name does
  not match `client/package.json`, and `release` needs `verify`, so a bad tag
  cannot publish. `cargo fetch --locked` separately catches a stale `Cargo.lock`.
  These are safety nets — they do not replace the sequence above.
- **"All five sites" is data, not prose.** The list lives in `docs/state.toml`
  `[client.version_sites]` and `check-consistency.sh` §9 recomputes it from
  `client/scripts/release-version.mjs`, so a sixth version site fails the build
  instead of silently invalidating any sentence that says "five".
- **`main` and the tag are separate pushes.** CI runs on both; only the tag
  build creates a Release.
- **Full procedure, and the hub-publish step that comes after:** 
  [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md). Note that publishing
  to the hub is a **separate manual step** — GitHub Release alone does not make
  clients update.

## Traps this repository has already paid for

Each of these cost a real incident. The comments in the code explain them at the
point of contact; this is the index.

- **The proxy group name must differ from the proxy name** (`Locus Auto` vs
  `Locus`). mihomo reads a match as a reference loop and refuses the entire
  config — which is valid YAML, so only running the engine catches it. See
  `client/AGENTS.md`.
- **Frozen wire names** (`uot_port`, the `download_*`→`update_*` rename, the
  `macos_intel`/`darwin-*` asymmetry) are contracts with deployed clients and the
  hub. Do not "tidy" them.
- **`docs/state.toml` is a version site.** It is easy to miss because it is not a
  manifest; the release section above covers it.
- **Secrets do not travel with a repo copy.** A fresh clone/migration has zero
  Actions secrets and every signed build fails until `LOCUS_UPDATE_KEY` is set.
- **Run `check-consistency.sh` with `bash`.** Under `sh`/`dash` it aborts with
  "Bad substitution" and prints false `BAD` lines.
- **Verify a test can actually fail before keeping it.** A test that cannot fail
  reads as green and guards nothing.
