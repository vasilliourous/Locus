# Agent guidelines — Locus

Read this, then the one guide for the area you are touching. Nothing here is a
procedure; each rule links to the file that owns it.

## Where things are

| Path | What it is | Its guide |
|---|---|---|
| `client/` | The desktop app (Tauri 2 + React). Product logic in `src-tauri/src/locus/`. | [`client/AGENTS.md`](client/AGENTS.md) |
| `server/` | The hub: PocketBase, Caddy, the Shadowsocks/BBR/tc stack, `pb_hooks/`. Also `server/site/` (landing page) and `server/console/` (operator console). | — |
| `docs/` | The documentation. [`docs/README.md`](docs/README.md) is the index. | — |
| `legacy/` | Retired client code. **Reference only — never build it.** | `legacy/wails-client/ARCHIVED.md` |

## Read by what you are doing

| If you are… | Read first |
|---|---|
| **Cutting a release** | [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md) — the order is not optional |
| **Diagnosing a defect** | [`docs/reference/DEBUGGING-METHOD.md`](docs/reference/DEBUGGING-METHOD.md) — and check [`docs/reference/FIXES.md`](docs/reference/FIXES.md) §1, it may already be known-unverified |
| **Writing any fact into a document** | [`docs/operate/CLAIMS.md`](docs/operate/CLAIMS.md) — data vs world claim |
| **Deploying or operating the hub** | [`docs/operate/OPS.md`](docs/operate/OPS.md), [`docs/operate/DEPLOY.md`](docs/operate/DEPLOY.md) |
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

- **The proxy group name must differ from the proxy name** (`Locus Auto` vs
  `Locus`) — mihomo treats a match as a reference loop and refuses the whole
  config, which is valid YAML. Only running the engine catches it.
- **Frozen wire names** (`uot_port`, `download_*`→`update_*`, the
  `macos_intel`/`darwin-*` asymmetry, the `eco`/`free` and `strike`/`Full`
  splits) are contracts with deployed clients and the hub. Do not "tidy" them.
- **A repo fix is inert until the piece that runs it is redeployed.** The hub runs
  its own copy; `server/scripts/hooks-sync.sh` compares them.
- **`main` and the tag are separate pushes**, and CI does not bump versions — a
  tag that disagrees with the version files builds binaries that misreport
  themselves. [`docs/operate/RELEASING.md`](docs/operate/RELEASING.md).
- **Run `check-consistency.sh` with `bash`.** Under `sh`/`dash` it aborts with
  "Bad substitution" and prints false `BAD` lines.
- **Secrets do not travel with a repo copy.** A fresh clone has zero Actions
  secrets and every signed build fails until `LOCUS_UPDATE_KEY` is set.
