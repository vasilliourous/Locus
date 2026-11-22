# Name migration — Chrysalis, and the Locus names that stay

```
audience:    all
status:      live
authoritative-for: which names are Chrysalis, which are still Locus, and what retires each one
verified-against: server/scripts/check-consistency.sh §53; docs/state.toml
```

> **The product is Chrysalis.** On 2026-11-23 it was renamed from Locus. Every
> user-facing string, every document, and every identifier that is *ours to move*
> moved with it.
>
> **`locus` is not gone, and that is deliberate.** A set of low-level names could
> not move without breaking something that is already deployed: an installed
> client's update identity, a published artefact a hub resolves by name, the key
> that signs those artefacts, or the committed ciphertext that holds that key.
>
> This page is the complete list. **If you are about to "fix" a `locus` you found
> by grepping, read the row that owns it first.** §53 of
> [`server/scripts/check-consistency.sh`](../../server/scripts/check-consistency.sh)
> asserts this list against the tree, so a name that is neither documented here nor
> renamed fails the build.

---

## 1. Why a product can be renamed and its identifiers not

Three properties make a name into a **contract** rather than a label:

1. **It is compiled into software you can no longer change.** A released client
   carries its bundle identifier and its trusted signing key in its own binary.
2. **Something outside the repository resolves it by name.** The hub fetches
   `locus-linux-amd64` out of a GitHub Release; CI signs a file whose name it
   asserted by literal.
3. **It is the durable handle for state that must not be orphaned.** An
   activation code lives in a directory named after the identity; a private key
   lives in a committed encrypted bundle under a key name.

A rename that ignores those three produces a tree that builds green and a fleet
that stops updating. This migration renames everything in the *first* category
and holds the rest, with the holding recorded rather than forgotten.

## 2. The mapping that was applied

| Form | Became | Where it applies |
|---|---|---|
| `Locus` | `Chrysalis` | prose, `productName`, installer and bundle names |
| `locus` | `chrysalis` | crate, npm package, module path, fn prefix, field prefix |
| `LOCUS` | `CHRYSALIS` | env vars, `CHRYSALIS_COLORS`, error codes |
| `locus-` | `chrysalis-` | systemd units, tmp prefixes, service names |
| `locus_` | `chrysalis_` | env vars, `verge.yaml` fields, test names |

Complications the mapping had to survive, recorded so the next rename knows they
exist: a generated TypeScript mirror that fails `--check` if hand-edited; a
matched-pair HTTP header (`X-Locus-Bootstrap`) whose two halves live in different
languages; a consistency script that asserts ~11 old literals by regex; and
`git mv` on a directory whose module path is part of 221 `crate::` references.

## 3. The retained names — the complete list

Each row: **what stays**, **why it cannot move in this change**, and **what would
retire it**. The "Where" column names the file that owns the value.

### 3.1 Artefact file names

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `locus-linux-amd64`<br>`locus-windows-amd64.exe`<br>`locus-darwin-amd64`<br>`locus-darwin-arm64` | `client/src-tauri/src/chrysalis/contract.rs` (`artifact_for`); `server/scripts/fetch-release.py`; `server/scripts/publish-release.sh`; `.github/workflows/client.yml` | The hub resolves these exact names out of the GitHub Release, and three scripts assert the literals. Renaming one side alone breaks publishing for that platform — and the break is silent, because the file simply is not found. | A release publishes **both** name sets, the hub resolves both, and no build older than that release can still be offered an update. Then drop the `locus-*` rows. |

The `macos_*`-key / `darwin-*`-filename asymmetry inside these names is a
**separate** frozen fact, unrelated to the rename. Do not "tidy" it either.

### 3.2 Update identity

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `com.locus.client`<br>`com.locus.client.dev` | `client/src-tauri/tauri.conf.json` (`identifier`); `client/src-tauri/Cargo.toml`; `client/src-tauri/src/utils/dirs.rs` (`APP_ID`) | One value, two jobs. It is the **updater identity** — a manifest keyed `com.locus.client` is ignored by a differently-keyed build, with no error. And it is the **app-data root**: the activation code, device fingerprint and tier live under it, and on Windows the owner SID of that directory is how the privileged service recognises the app. | A deliberate identity release: `LEGACY_APP_IDS` (the existing adoption path) proves it can be done, and the updater-key mechanism proves how a fleet is moved. It is not part of a rename. |
| `locus-backup`, `locus-backup-dev` | `client/src-tauri/src/utils/dirs.rs` (`BACKUP_DIR`) | Backup directory names inside that same data root. Same lifetime as the root. | With the data root. |

**The consequence worth stating plainly:** because the identity did not move, an
installed Chrysalis **keeps its data and keeps receiving updates** from the
release it was on. That is why this rename needed no data migration and no key
rotation.

### 3.3 Signing

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `LOCUS_UPDATE_KEY` | `.github/workflows/client.yml` (×23) | A GitHub Actions secret. Actions secrets are write-only: renaming the *reference* yields an empty key and every signed build fails, which is a documented trap in `AGENTS.md`. | The operator re-creates it under the new name from the bundle (§3.4), then renames the reference in one commit. |
| `LOCUS_UPDATE_KEY_B64` | inside `server/secrets.env.age` (ciphertext); `server/scripts/chrysalis-key-extract.sh`; `.chrysalis-keys/README.md` | A **key name inside the committed encrypted bundle**. Changing it means re-encrypting the bundle, which re-keys the project — not needed to rename a spelling. | With a deliberate bundle re-encrypt. |

**Neither is a sign of a stale migration.** They are the aliases the fleet's
update path depends on.

### 3.4 The bootstrap header (a matched pair)

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `X-Locus-Bootstrap` / `x-locus-bootstrap` | `server/modules/09-activation-tunnel.sh` (sets it); `server/pb_hooks/activation.pb.js`, `code_lookup.pb.js` (read it) | The path filter stamps a per-client digest so tunnelled clients are rate-limited per device rather than per loopback address. If one side changes and the other does not, the header is simply absent and **every tunnelled client shares one bucket** — no error, just a silent collapse of the rate limit. Note the hub runs its *own* copy from `/root/server/` (`hooks-sync.sh`), so a repo change is inert until redeployed. | Both halves deployed together, accepting both header spellings for one release, then the old one dropped. |

**Status: renamed in the repository.** All three sites read `X-Chrysalis-Bootstrap`
now, which is safe *in this tree*; the deployed hub must be redeployed before a
client that sends the new header is in the field, or activation through the
tunnel degrades as described above.

### 3.5 Secrets environment variables

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `LOCUS_SECRETS_PASSPHRASE`<br>`LOCUS_SECRETS_PASSPHRASE_FILE` | accepted as a **fallback** by `server/setup.sh`, `server/restore.sh`, `server/scripts/secrets-{en,de}crypt.sh` | The primary names are `CHRYSALIS_SECRETS_*`. The old spellings are still *accepted* because a host that has not been re-provisioned passes them, and a deploy that fails on a variable name mid-upgrade is worse than one that reads a second name. | The next release after every deployed host's invocation has been updated. Then delete the fallback branches. |

There is also `.chrysalis-keys/` → this directory *was* `.locus-keys/`; the
rename moved the `.gitignore` carve, §5b/§5c and every doc link in one edit,
because a half-moved carve publishes or hides the signing key.

### 3.6 Live infrastructure names

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `locusvpn.jadedns.uk` | served **alongside** `chrysalisvpn.jadedns.uk` on one Caddy block: `server/templates/Caddyfile`, `server/modules/05-caddy.sh` (`LANDING_DOMAIN_LEGACY`), recorded in `docs/state.toml` `[site.domain_legacy]` | DNS and TLS are a deployment, not a commit. The old name resolves today; the new one does not yet. Serving both means the page never 404s between the rename and the DNS change. | `chrysalisvpn.jadedns.uk` answers on the box → set `LANDING_DOMAIN_LEGACY=""` and drop the row from `state.toml`. |
| `/etc/locus/…`, `locus-fetch.service`, `locus-upload.service` | `server/modules/09-activation-tunnel.sh`, `05-caddy.sh`, `08-firewall.sh` | **Renamed in the repository** to `/etc/chrysalis/` and `chrysalis-fetch.service`. On a live box the old unit and key path exist until `setup.sh` re-runs; the repo copy is inert until then. `locus-upload.service` survives only as a *retirement* target in `05-caddy.sh` — it is the name the script disables. | `setup.sh` re-run on the hub. The `locus-upload` mention is a historical reference and should stay until no box can still have it. |
| `locus-hub` (ssh alias) | `server/scripts/deploy.sh`, `publish-release.sh`, `verify-release.sh` | An alias in the **operator's** `~/.ssh/config`, not in this repository. Renaming the literal in the scripts breaks deploy on the machine that has the old alias, and nothing in CI would notice. | The operator adds `chrysalis-hub` to their ssh config; then the literal changes. |

### 3.7 The GitHub repository

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `vasilliourous/Locus` | `server/scripts/fetch-release.py` (`GITHUB_REPO`), `publish-release.sh`, `verify-release.sh`, `deploy-site.sh`, `server/templates/chrysalis-fetch.service`, `server/site/index.html` | The remote's name is a hosting decision, and the hub fetches releases from it **unauthenticated** — a rename that races a deploy breaks publishing. `GITHUB_REPO` is an env override, so the hub can be pointed at a new name without a code change. | The operator renames the repo; then the defaults change in one commit. |

## 4. What was renamed, for the record

Not exhaustive — the point is the *classes*, because each had its own hazard:

- **Module path** `src-tauri/src/locus/` → `src/chrysalis/` plus `cmd/locus.rs` →
  `cmd/chrysalis.rs`. The compiler is the oracle here: a missed reference does not
  compile, which is why this went first.
- **15 IPC commands** (`locus_status` → `chrysalis_status`, …) and their
  **generated** TypeScript mirror. These are Tauri IPC names, not wire contract —
  nothing outside this build calls them.
- **`pub struct LocusStatus`** and its siblings, then
  `gen-ts-contract.py --write`. Hand-editing the generated file fails `--check`.
- **18 `locus_*` fields** in `verge.yaml`, which are **local** config: the same
  reasoning that froze `uot_port` (crosses the wire) does not apply to them.
- **`LOCUS_COLORS`/`locusTheme`/`LOCUS_LIGHT`**, the mihomo proxy/group/UoT names
  (`Chrysalis`, `Chrysalis Auto`, `Chrysalis-UoT` — which must still differ from
  each other), the `chrysalis://` event names, the tray id, the UA string.
- **`locus.yaml` → `chrysalis.yaml`** as the tier profile file, and
  `locus-backup`/`chrysalis-backup` as the *directory the app creates*.
- **~500 user-facing strings** across 13 locales, applied through
  `PROSE.md`/`apply-prose.mjs` rather than by hand.
- **The landing page**, copy and hostname.

## 5. How this stays true

- **§53 of `check-consistency.sh`** asserts the retained set: it fails if a
  documented alias vanishes (someone "completed" the rename and broke the fleet)
  or if the documented list is empty (someone deleted the register).
- **`docs/state.toml`** carries `[site.domain_legacy]` as a **derived** fact, so
  §9 recomputes it against `05-caddy.sh` and fails when the two disagree.
- **`§5b`/`§5c`** keep `.chrysalis-keys/` carrying nothing but its README, and
  keep the extraction script reachable from it.
- **`§1c`/`§1e`/`§1g`/`§1h`** pin the artefact and installer names that §3.1
  retains, so they cannot drift silently.

## 6. Out of scope, on purpose

| Left alone | Why |
|---|---|
| `vendor/` (9 binaries, 1 data file) | `locus` there is a **geosite category tag** and byte sequences inside pinned upstream binaries. Editing them fails the hash gate (§39) and corrupts a vendored file. |
| `legacy/` (64 files) | The retired Wails client. Never built, and its old names are the only correspondence its commits have. |
| `docs/archive/`, `docs/history/` | Retired and reference material. The folder carries the status, and a historical document naming a dead path is history, not rot. |
| Historical quotations in `FIXES.md`, `CLAIMS.md` §5, and CI comments | A defect log that says a message read something else is a false record. Each such mention is marked `(now …)` where it matters; the `D:\a\Locus\Locus` CI-workspace path in `CLAIMS.md` A8 is marked **historical string, not a live identifier**. |
| Genuine English "locus" | None found outside `vendor/`. If one appears, it is not a defect. |
| The cat logo | Contains no text; verified by grep over every `.svg`. Not touched. |

## 7. Related

- [`docs/operate/DEPLOY.md`](../operate/DEPLOY.md) — the deployment these names land on
- [`docs/operate/SECRETS-MANAGEMENT.md`](../operate/SECRETS-MANAGEMENT.md) — the bundle and its passphrase
- [`client/docs/IDENTITY-MIGRATION.md`](../../client/docs/IDENTITY-MIGRATION.md) — the app identity, and why §3.2 is separate from this page
- [`docs/history/LAYOUT.md`](../history/LAYOUT.md) — the path-level history this rename adds a row to
