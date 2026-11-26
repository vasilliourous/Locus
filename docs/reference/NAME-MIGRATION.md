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
>
> **A `locus` name also survives as an *installed application*, not just as a
> string.** Updating across the move installs the new identity beside the old one,
> so the migration has an installer half as well as a data half; §56 guards that
> pair and [`client/docs/UPDATE-INSTALL-MIGRATION.md`](../../client/docs/UPDATE-INSTALL-MIGRATION.md)
> is where it is designed.

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

### 3.0 When these retire, and who decides

Each row: **what stays**, **why it cannot move in this change**, and **what would
retire it**. The "Where" column names the file that owns the value.

Per-name conditions ("`setup.sh` re-run", "DNS moves", "the operator renames the
repo") are the right shape for a fact about the world, and each row in §3.1–§3.7
carries its own. What none of them can do is tell you **which release** drops an
alias, and that is the question the next reader actually has: every `locus`
string left in this tree will otherwise be re-litigated by whoever greps next.

**Two classes, two different answers.**

| Class | Rows | Who retires it | Gate |
|---|---|---|---|
| **A migration-owned alias** — a fallback this rename *added*, accepted only for the transition | `LOCUS_SECRETS_PASSPHRASE*` (§3.5); the old bootstrap-header spelling, once deployed (§3.4) | Whoever closes the migration, in one change that removes both spellings' readers and the register row together | None can be written from a checkout: the condition is "every deployed host has been re-provisioned", which is a world claim (`CLAIMS.md` §5), not a tree fact |
| **A fleet contract** — a name installed software or DNS depends on | `com.locus.client*` (§3.2); the `locus-*` artefact names (§3.1); `LOCUS_UPDATE_KEY*`; `vasilliourous/Locus` (§3.7) | An **identity release**: versioned, with its own note in `docs/operate/RELEASING.md`, never a tidy-up | `check-consistency.sh` §53 fails the moment one disappears, which is deliberate: the tree cannot tell a correct retirement from the bug. `locusvpn.jadedns.uk` left this class on 2026-11-26 after its retirement condition was met — see §3.6. |

**The rule, in one sentence:** *a class-A alias is deleted in the first release
after its condition is met and that release is recorded here and in `CLAIMS.md`;
a class-B name is never retired by a rename, only by a release that says so.*

**Where the next one is recorded.** When a class-A alias is dropped, add a row
below with the version and the date. §53 fails if this section is removed, so the
record cannot disappear while the aliases are still in the tree:

| Dropped | Release | Date | What made it safe |
|---|---|---|---|
| *(nothing yet — the migration is open)* | | | |

This is deliberately a table with no rows rather than a sentence: a sentence
would say the same thing and could not be updated in place.

### 3.1 Artefact file names — **moved on 2026-11-25, with the alias still served**

The filenames were the one part of the 2026-11-23 rename that was *held*, because
moving them alone breaks publishing silently. On 2026-11-25 they were moved as a
**coordinated artefact migration**: CI now publishes both name sets, the hub
resolves both, and one release carries the change.

| Now current | Where | Why it is a contract | Retires when |
|---|---|---|---|
| `chrysalis-linux-amd64`<br>`chrysalis-windows-amd64.exe`<br>`chrysalis-darwin-amd64`<br>`chrysalis-darwin-arm64` | `client/src-tauri/src/chrysalis/contract.rs` (`artifact_for`); `server/scripts/fetch-release.py` (`NEW_PLATFORMS`); `server/scripts/publish-release.sh` (`PLATFORMS`); `.github/workflows/client.yml` | What CI builds and the hub prefers. The hub resolves these out of the GitHub Release, so a rename on one side alone makes the file "not found" — silently. | Never: this is the current name. It moves only in a future artefact migration of the same shape. |

| Deprecated alias, still served | Where | Why it cannot be deleted yet | Retires when |
|---|---|---|---|
| `locus-linux-amd64`<br>`locus-windows-amd64.exe`<br>`locus-darwin-amd64`<br>`locus-darwin-arm64` | `server/scripts/fetch-release.py` (`LEGACY_PLATFORMS`); `server/scripts/publish-release.sh` (`WINDOWS_INSTALLER_LEGACY`); `.github/workflows/client.yml` (the DEPRECATED ALIAS staging steps) | Every build older than this release resolves these exact names. Deleting them makes a parked fleet unable to fetch the update that would move it, and the failure is `file not found` — no error on the client, no signal on the hub. | No build older than the first `chrysalis-*` release can still be offered an update. Then drop `LEGACY_PLATFORMS`, the legacy installer template, both CI staging steps, and these rows — together. `check-consistency.sh` §1c/§1e and `contract-snapshot.py` (`legacy_artifacts`) refuse a half-removal, which is the state that is actually dangerous. |
| `installer-Locus_<v>_x64-setup.exe`<br>`installer-Locus_<v>_<arch>.zip` | `server/scripts/fetch-release.py` (`LEGACY_INSTALLER_NAME_TEMPLATE`, `LEGACY_MACOS_ZIP_TEMPLATE`); `server/console/src/views/Releases.vue` | The Windows installer and the macOS human download, under the pre-move `productName`. The installer name is derived from `productName`, so these are **filenames** — see below — and they follow it. | With the artefact rows above. The zip is a human download and is not in any update slot; §1c keeps it that way. |

The `macos_*`-key / `darwin-*`-filename asymmetry inside these names is a
**separate** frozen fact, unrelated to the rename. Do not "tidy" it either.

| Now current | Where | Why it is a contract | Retires when |
|---|---|---|---|
| `productName: "Chrysalis"` | `client/src-tauri/tauri.conf.json`, `client/src-tauri/tauri.macos.conf.json` | Tauri derives the bundle filenames from `productName`: `productName` → `<productName>_<v>_x64-setup.exe` → CI prefixes it to `installer-<productName>_<v>_x64-setup.exe`, and it names `<productName>.app` inside the `.dmg` and the `.app.tar.gz`. So this one string decides whether the files the hub fetches exist. It moved on 2026-11-25 **with** every resolver; the v3.5.0 failure it caused is recorded in `FIXES.md`, and is why §1e now asserts *agreement between* `productName` and the template rather than a literal. | Never: it is the product name, and it is *supposed* to be Chrysalis. |

**`productName` changing is why 20 `.app` literals had to change too.** They were
hardcoded `Locus.app` in the macOS packaging steps; each now reads `APP_NAME` from
`tauri.conf.json`, so the next move needs no edit in the workflow. That is the
lesson, not the patch: a value that derives a filename must be *read*, never
restated.

§1e of `check-consistency.sh` pins `productName` against the installer name the
hub resolves, and asserts BOTH installer templates exist, so this cannot be
"tidied" in isolation again.

### 3.2 Update identity — **moved on 2026-11-25, with a data-root adoption**

The identity moved, and it moved **for a reason that made it safe**: this value is
*read*, never compiled into anything that outlives the install. The reason it was
originally held was mistaken in one half and right in the other, and both halves
are recorded here because the next person will ask.

**The `updater identity` half of the old reasoning was wrong.** §3.2 previously said
`com.locus.client` could not move because "a manifest keyed `com.locus.client` is
ignored by a differently-keyed build, with no error". That is true of a **static**
`latest.json` consumed by the plugin's own endpoint resolution. This client does
not use one: `update/install.rs` replaces the endpoint list wholesale
(`.endpoints(vec![url])`) with the URL the **hub** supplies, and the hub's
`/api/update` response contains no identifier field at all (grep: 0 occurrences in
`update.pb.js`). Verification is against the compiled-in **minisign pubkey**, which
the identifier does not name. So the identifier was never the update contract.

**The data-root half was right, and it is the whole reason `LEGACY_APP_IDS` is not
optional.** That reasoning follows the two tables.

**What the identifier *does* decide, and what the 2026-11 update proved the hard
way.** It is not an updater contract, but it is three other things, and the last
one is not recoverable by the user:

1. the app-data root (`%APPDATA%\com.chrysalis.client`, `~/.local/share/...`);
2. the **install directory** on Windows, because `%PROGRAMFILES%\<productName>`
   is `productName`, not the identifier;
3. the **NSIS uninstall registration** on Windows, keyed on `productName`
   (`Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>` — see the
   bundler's own template, quoted in
   [`client/docs/UPDATE-INSTALL-MIGRATION.md`](../../client/docs/UPDATE-INSTALL-MIGRATION.md) §2.1).

Because (2) and (3) moved with `productName`, updating across the move installed a
**second application** rather than a new version — and the older one's data root
held the activation code. That is a fleet contract nobody had written down, and
it is now the row below plus a guard: `check-consistency.sh` §56 compares the
`Locus` identity in `client/src-tauri/src/chrysalis/update/identity_table.rs`
against `packages/windows/migrate-predecessor.nsh`, so the client and the
installer cannot disagree about which identity to migrate.

| Now current | Where | Why it is a contract | Retires when |
|---|---|---|---|
| `com.chrysalis.client`<br>`com.chrysalis.client.dev` | `client/src-tauri/tauri.conf.json` and the three `tauri.*.conf.json` overrides (`identifier`); `client/src-tauri/Cargo.toml` (`[package.metadata.bundle]`); `client/src-tauri/src/utils/dirs.rs` (`APP_ID`) | The **app-data root**: the activation code, device fingerprint and tier live under it, and on Windows the owner SID of that directory is how the privileged Service recognises the app (`core::owner_identity` reads the *resolved root's* owner, not the identifier string). | Never: it is the current identity. |

| Deprecated alias, still adopted | Where | Why it cannot be dropped | Retires when |
|---|---|---|---|
| `com.locus.client`<br>`com.locus.client.dev` | `client/src-tauri/src/utils/dirs.rs` (`LEGACY_APP_IDS`) | Every install before 2026-11-25 keeps its activation code, device fingerprint and tier under this root. `migrate_legacy_app_data_dir` renames it on first launch, so dropping the entry makes those installs start from an **empty** root — they re-activate, and on Windows the Service stops recognising the app until the new root is repaired. Nothing errors; it simply starts empty. | A release where no installed machine still has data under the old root. Not verifiable from the repository — this is a fleet observation, so it retires on evidence, not on a date. |

| Retained | Where | Why it cannot move | Retires when |
|---|---|---|---|
| `locus-backup`, `locus-backup-dev` | `client/src-tauri/src/utils/dirs.rs` (`BACKUP_DIR`) | **A different kind of name from `APP_ID`.** It is the collection name used in **WebDAV paths** (`core::backup` builds `{BACKUP_DIR}/{file}`), so it names a directory on a server this product does not control. Renaming it does not migrate anything: it points at a collection that does not exist, `mkcol` creates an empty one, and every backup a user already uploaded becomes invisible while remaining in their storage — indistinguishable from data loss. It also appears in no user-facing string. | A deliberate backup-location migration that moves every user's remote files. Not a rename. |

**The consequence worth stating plainly:** an install from before 2026-11-25
**keeps its data** — the old root is renamed into the new one on first launch,
before anything else touches it. What it does *not* keep, and never did, is
anything keyed to the identifier string itself; nothing was.

### 3.3 Signing — **the Actions secret stays, the bundle key is either-of**

Two different names, two different lifetimes. They are recorded together because
they are always confused, and confusing them is how a release ships unsigned.

| Now current | Where | Why it is a contract | Retires when |
|---|---|---|---|
| `CHRYSALIS_UPDATE_KEY` | `.github/workflows/client.yml` (secrets reference) | A GitHub Actions secret. Actions secrets are **write-only**: renaming the *reference* to a secret that does not exist yields an empty string, and every signed build then fails with nothing pointing at the name. | Never under a rename — this is the name to converge on. |
| `CHRYSALIS_UPDATE_KEY_B64` | inside `server/secrets.env.age`; `server/scripts/chrysalis-key-extract.sh`; `server/scripts/secrets-encrypt.sh` | A key name **inside the committed ciphertext**. It cannot be changed by editing a file: the bundle must be re-encrypted with `secrets-encrypt.sh` and the passphrase. | When the bundle is next re-encrypted, which is the only thing that can change it. |

| Deprecated alias, still read | Where | Why it cannot be dropped | Retires when |
|---|---|---|---|
| `LOCUS_UPDATE_KEY` | `.github/workflows/client.yml` (both the `env:` mapping and the shell fallback) | The secret itself may still be under this name on any repository that builds — including forks and fresh copies, where it must be re-created by hand. The workflow reads **both** and resolves to one variable, so neither ordering of "create the new secret" vs "land this commit" can produce an empty key. | `gh secret list` shows `CHRYSALIS_UPDATE_KEY` on every repository that builds. Then delete the alias half of the `env:` mapping. |
| `LOCUS_UPDATE_KEY_B64` | inside the committed bundle; both scripts accept it | Every bundle written before 2026-11-25 carries this spelling, and the bundle is the key's only durable copy. | The next deliberate bundle re-encrypt (see below). |

**How the two scripts handle the pair now.** Both *read* both names; neither
*requires* one spelling:

| Script | Order | What it does with the pair |
|---|---|---|
| `server/scripts/chrysalis-key-extract.sh` | `CHRYSALIS_…` first, then `LOCUS_…` | Reports **which** it found. A host still on the alias gets a warning naming the re-encrypt, because otherwise the alias is load-bearing forever and nobody knows. |
| `server/scripts/secrets-encrypt.sh` | either-of, above the `REQUIRED_VARS` loop | Requires exactly **one**, and **refuses both**: the last definition wins in a shell, so a shadowed key would otherwise encrypt cleanly and be wrong. |

`REQUIRED_VARS` no longer lists either spelling — listing one would refuse every
bundle of the other generation, which is the defect this whole section exists to
prevent. §53 asserts the either-of property instead of a literal: both names
readable, ambiguity refused, and neither back in `REQUIRED_VARS`.

**Neither script is covered by a test**, because both need the real passphrase and
the real ciphertext. What §53 can do is refuse a rename that would make them
wrong, which is the class the migration actually produced.

The rename **broke both once already**: an over-eager `CHRYSALIS_UPDATE_KEY_B64`
substitution left the extractor failing with *"the bundle has no
CHRYSALIS_UPDATE_KEY_B64"*, and the encrypt-side required-var list would have
refused every re-encrypt. Both were found by running the scripts against the real
bundle, not by reading them — the extractor's failure is a silent one that only
appears at signing time.

**What is NOT verified here:** that the committed bundle still decrypts, and that
its key ID still matches the pubkey compiled into the client, are facts about the
ciphertext. They live in [`CLAIMS.md`](../operate/CLAIMS.md) as dated world claims
(A25), never as a sentence on this page.

### 3.4 The bootstrap header (a matched pair, **asymmetric during the window**)

| Now current | Where | Why it is a contract | Retires when |
|---|---|---|---|
| `X-Chrysalis-Bootstrap` / `x-chrysalis-bootstrap` | `server/modules/09-activation-tunnel.sh` (`IDENTITY_HEADER`, sets it); `server/pb_hooks/activation.pb.js`, `code_lookup.pb.js` (read it) | The path filter stamps a per-client digest so tunnelled clients are rate-limited per device rather than per loopback address. If the writer and a reader disagree, the header is simply ABSENT and **every tunnelled client shares one bucket** — no error, just a silent collapse of the rate limit. | Never: this is the current spelling. |

| Deprecated alias, **still accepted by both readers** | Where | Why it cannot be dropped yet | Retires when |
|---|---|---|---|
| `x-locus-bootstrap`<br>`X-Locus-Bootstrap`<br>`x-chrysalis-bootstrap`<br>`X-Chrysalis-Bootstrap` | `server/pb_hooks/activation.pb.js`, `code_lookup.pb.js` (all four are matched, in that call order, current first) | The hub runs its **own** copy of the hooks from `/root/server/` (`hooks-sync.sh`), and clients in the field were built before 2026-11-23. A deployed hub and a deployed client can be on opposite sides of the rename for a release cycle. Header lookup is case-sensitive in the map goja hands the hook, so both cases of both spellings are listed rather than relying on a normalisation. | Both halves are deployed **and** no client in the field sends the old spelling. Then remove the two `x-locus`/`X-Locus` arms and their §53 rows together. |

**The pair is not symmetric, and treating it as if it were was a bug.** The path
filter only ever *writes* the header, so it carries the current spelling and
nothing else. The hooks only ever *read* it, so during a deploy window they accept
**both**. A guard that demanded the old spelling be absent from the hooks would
have forced exactly the deployment window that collapses every student into one
rate-limit bucket — which is why §53 now asserts the two sides differently: the
writer sends the current name, the readers still contain the old one.

**Status: renamed in the repository, dual-accept in the hooks.** Nothing here
proves the deployed hub has been redeployed — a repo fix is inert until the piece
that runs it is redeployed, and `hooks-sync.sh` is what compares them.

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
| ~~`locusvpn.jadedns.uk`~~ | **RETIRED 2026-11-26.** Was served **alongside** `chrysalisvpn.jadedns.uk` on one Caddy block (`server/modules/05-caddy.sh` `LANDING_DOMAIN_LEGACY`). The alias existed because DNS and TLS are a deployment, not a commit: `chrysalisvpn` had no working certificate, so dropping the old name would have 404ed the page. | The condition this row set for itself — *`chrysalisvpn.jadedns.uk` answers on the box* — is now met. On 2026-11-26 the hub completed ACME for `chrysalisvpn.jadedns.uk` (`certificate obtained successfully`), answered **200** on it locally, and stopped serving `locusvpn` at all. `LANDING_DOMAIN_LEGACY` now defaults to **empty**, `state.toml [site.domain_legacy]` is empty, and the template names one hostname. | Retired. An operator whose DNS has not moved may still pass `LANDING_DOMAIN_LEGACY=locusvpn.jadedns.uk` explicitly — the variable is kept for that, the default is not. |
| `/etc/locus/…`, `locus-fetch.service` | `server/modules/09-activation-tunnel.sh`, `05-caddy.sh`, `08-firewall.sh` | **Renamed in the repository** to `/etc/chrysalis/` and `chrysalis-fetch.service`. On a live box the old unit and key path exist until `setup.sh` re-runs; the repo copy is inert until then. §53 pins both retired spellings in `09-activation-tunnel.sh` and `05-caddy.sh`, because a `locus` reappearing in either is the silent half of this row: the box keeps the old path and the repo stops reading it. | `setup.sh` re-run on the hub (class B — see §3.0). |
| `locus-hub` (ssh alias) | the probe in `server/scripts/deploy.sh`, `publish-release.sh`, `verify-release.sh` | An alias in the **operator's** `~/.ssh/config`, not in this repository. The scripts **probe `chrysalis-hub` first** and fall back to the bare host; the alias itself lives on the operator's machine, so a probe for a name that is not configured there is a no-op rather than an error — which is why the fallback exists and why nothing in CI would notice its absence. | The operator adds `chrysalis-hub` to their ssh config (class B — see §3.0). When no operator machine needs the fallback, the probe's else-branch goes. |

The service `05-caddy.sh` **disables** is `chrysalis-upload.service`, not a
`locus` name: that unit was already carried the new spelling before this
migration, and the function retires it because leaving it running holds port 8091
against `chrysalis-fetch`. An earlier revision of this row called it
`locus-upload.service`, which no file in the tree ever contained — the kind of
plausible-looking alias this page exists to keep out.

### 3.7 The GitHub repository

| Retained | Where | Why | Retires when |
|---|---|---|---|
| `vasilliourous/Locus` | `server/scripts/fetch-release.py` (`GITHUB_REPO`), `publish-release.sh`, `verify-release.sh`, `deploy-site.sh`, `server/templates/chrysalis-fetch.service`, `server/site/index.html` | The remote's name is a hosting decision, and the hub fetches releases from it **unauthenticated** — a rename that races a deploy breaks publishing. `GITHUB_REPO` is an env override, so the hub can be pointed at a new name without a code change. | The operator renames the repo; then the defaults change in one commit. |

### 3.8 A `locus` that survived in PROSE, not in code

The rows above are names something *reads*. This one was a sentence: when
`HUB_ALIAS_DOMAIN` is set, `05-caddy.sh` logs how to point a client at the alias,
and until this row existed it said **`locus_hub_override`** — a `verge.yaml` field
the rename had already called `chrysalis_hub_override`. Nothing reads the log, so
nothing failed; an operator would have followed it and set a key the client
ignores.

| Retained (wrong) | Where | Why it is here | Retires when |
|---|---|---|---|
| ~~`locus_hub_override`~~ → `chrysalis_hub_override` | `server/modules/05-caddy.sh` (log line); `client/src-tauri/src/config/verge.rs` (the field); `server/pb_hooks/update.pb.js` (comment) | The field is **local** config, like the other `locus_*` fields §4 lists, so the rename moved it and the operator-facing prose did not follow. §53 now fails if the old spelling reappears in either file. | Never — it is already the new name. The row is the record of a prose defect, not an alias. |

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
- **`locus.yaml` → `chrysalis.yaml`** as the tier profile file
  (`TIER_PROFILE_FILE` in `chrysalis/apply.rs`, which is LOCAL: it is written into
  `app_profiles_dir()` on this machine).
- ~~`locus-backup` → `chrysalis-backup`~~ **NOT moved, and §3.2 says why.** This
  line claimed the rename moved `BACKUP_DIR`; it did not, because that constant is
  the collection name in **WebDAV paths** — a directory on a server this product
  does not control. Renaming it would point at an empty collection and make every
  uploaded backup invisible. The earlier claim here was the kind of error a
  "what the rename touched" list invites: it recorded an *intention* as a fact.
- **~500 user-facing strings** across 13 locales, applied through
  `PROSE.md`/`apply-prose.mjs` rather than by hand.
- **The landing page**, copy and hostname.

## 5. How this stays true

- **§53 of `check-consistency.sh`** asserts the retained set: it fails if a
  documented alias vanishes (someone "completed" the rename and broke the fleet)
  or if the documented list is empty (someone deleted the register).
- **§56** keeps the two halves of the *cross-grade* migration honest: it reads
  `client/src-tauri/src/chrysalis/update/identity_table.rs` (by running the
  `predecessors` binary, so it reads what the compiler reads) and compares it,
  row by row and in order, against
  `client/src-tauri/packages/windows/migrate-predecessor.nsh`. The client uses
  the first to refuse a hand-over while a predecessor is running; the installer
  uses the second to move that predecessor's data root aside and remove the old
  application. A row in one and not the other is a fleet that either races a live
  application or keeps the duplicate installed — silently, because on Windows the
  app has already exited by the time the installer runs.
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
| ~~The cat logo~~ **Now replaced** | It carried no text, so this register never governed it. It was replaced by the FlClash glyph on 2026-11-23 for brand reasons, not naming ones — see `client/src-tauri/icons/README.md`. The row is kept, struck through, because a reader grepping for the mark should find where it went. |

## 7. Related

- [`docs/operate/DEPLOY.md`](../operate/DEPLOY.md) — the deployment these names land on
- [`docs/operate/SECRETS-MANAGEMENT.md`](../operate/SECRETS-MANAGEMENT.md) — the bundle and its passphrase
- [`client/docs/IDENTITY-MIGRATION.md`](../../client/docs/IDENTITY-MIGRATION.md) — the app identity, and why §3.2 is separate from this page
- [`docs/history/LAYOUT.md`](../history/LAYOUT.md) — the path-level history this rename adds a row to
