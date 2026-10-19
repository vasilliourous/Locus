# Publishing a Release to the Hub

```
audience:    human-operator
status:      live
authoritative-for: cutting a release (tag → CI → hub publish)
verified-against: client/scripts/release-version.mjs, server/scripts/publish-release.sh
```

> **Note:** the old release-cutting machinery is **deleted**, not repointed — it
> versioned the retired client and never the fork. What replaced it:
> - CI at `.github/workflows/client.yml` (**repo root** — GitHub ignores a workflow
>   under `client/.github/`). A `v*` tag builds, **signs** and releases all four
>   platforms plus `manifest.json`.
> - The client versions itself (`client/package.json`, `client/src-tauri/Cargo.toml`,
>   `client/src-tauri/tauri.conf.json` must agree, enforced by
>   `client/src-tauri/tests/version_consistency.rs`). There is no root `VERSION`.
>
> **A bump is one command.** `pnpm release-version X` (from `client/`) writes
> every site listed in `[client.version_sites]` — including `docs/state.toml`,
> the one that is easy to miss because it is a data file, not a manifest.
> `check-consistency.sh` §7 checks it against `client/package.json` and fails the
> `verify` job with `BAD client.version`. The script handles it so a release
> cannot forget it; if a further site ever
> appears, add it to `client/scripts/release-version.mjs`. `cargo fetch --locked`
> separately catches a stale `Cargo.lock` in each build job.
>
> **The list is data, not prose.** `docs/state.toml` `[client.version_sites]`
> holds those paths with `provenance = "derived"`, and `check-consistency.sh` §9
> recomputes it from `release-version.mjs` and fails on disagreement — a new site
> fails the build instead of silently invalidating this paragraph.
>
> **Read `UPDATE-SYSTEM.md` for the full pipeline**, and `client/docs/SIGNING.md`
> for key custody. The short version:

```
1. Bump every version site, from client/:
       pnpm release-version X.Y.Z
2. Commit, then tag, then push both:
       git add -A && git commit -m "chore(release): X.Y.Z"
       git tag -a vX.Y.Z -m "Locus client vX.Y.Z"
       git push origin main && git push origin vX.Y.Z
     → the TAG triggers CI to build + sign all four platforms and create the
       GitHub Release. Pushing main alone does not.
3. Fetch:  console Releases → "Fetch & publish",  or
           POST /api/admin/fetch-release {"version":"X.Y.Z"}
     → the hub pulls from GitHub, verifies format + SHA-256 + signature
4. Publish: writes update_config. The console does 3 and 4 in one button.
5. RE-RENDER THE LANDING PAGE — this is a real step, not a nicety:
       server/scripts/deploy.sh --site
     → re-renders the four download buttons from the new manifest and uploads it
```

> **Step 5 is the one people skip, and skipping it is silent.** The public page's
> four download URLs are *rendered from the release manifest* by
> `deploy-site.sh` — they are not hand-written, precisely so a version cannot be
> baked into `index.html`. But that means the page only moves when the renderer
> RUNS, and nothing in steps 1–4 does that: CI builds artifacts, the hub serves
> them, and the landing page goes on advertising the previous version's files.
>
> Nothing fails. The page is not broken — it is offering last release's binaries,
> which still exist on the hub, so every button still downloads a real file. The
> only symptom is that new visitors get the old version, and that is invisible
> from inside the repo.
>
> **Verify it, do not assume it.** `verify-release.sh` re-reads the SERVED page
> and fails when its buttons do not name the version just published — see
> "Checking the page is current" below.

### Checking the page is current

The page is a **Class A world claim**: what is deployed is not derivable from the
tree. Two ways to check it, both read-only:

```bash
# 1. What the page actually offers, right now.
curl -s https://locusvpn.jadedns.uk/ \
  | grep -oE 'updates/[0-9]+\.[0-9]+\.[0-9]+/[^"]*'

# 2. Does it match the version the hub is serving?
server/scripts/verify-release.sh <version>
```

Everything under `/updates/<version>/` must name `<version>`. A page naming an
older version is not an error state the repo can see — it is a page nobody
re-rendered, and the fix is step 5.

> **CI does not bump versions — it labels the release from the tag name.**
> `github.ref_name` supplies the release title and the manifest's `version`
> field, while the number compiled into the binaries comes from the version
> files. So if the tag and the files disagree, CI publishes binaries that
> misreport their own version under the tag's name, and the hub offers an
> "update" that reinstalls the same version forever.
>
> This happened on 2026-09-30: the `v3.2.13` tag was pushed on a commit whose
> version files still said `3.2.12`. The `verify` job now fails in seconds on
> any tag whose name disagrees with `client/package.json` (step
> "Tag must name the version being built"), so the mistake costs a red check
> rather than a published release. **Bump, commit, then tag.**

> **Fetch and publish are two steps.** Forgetting step 4 leaves the artifacts
> served while `/api/release` still advertises the old version — nothing looks
> broken, updates simply do not happen.
>
> **There is no rollout percentage.** Publishing IS offering; `active` is the only
> off switch, and there is no server-driven downgrade.
>
> **Windows clients cannot update at all right now?** That is a different
> situation from a normal release — see
> [`RECOVER-WINDOWS-UPDATE.md`](RECOVER-WINDOWS-UPDATE.md).
>
> **macOS clients offered a bare binary?** *"the update to <version> is not an
> installer (<N> bytes)"* on macOS means the hub served a bare Mach-O instead of
> the `.app.tar.gz` — see [`RECOVER-MACOS-UPDATE.md`](RECOVER-MACOS-UPDATE.md).

### Before the *first* tag in a repository: set the signing secret

CI signs every build, so `LOCUS_UPDATE_KEY` must exist as an Actions secret or
step 2 fails (and so does every branch build — signing is not tag-gated).
**Actions secrets are stored per-repository and are not carried by a copy or a
clone**, so a migrated repository starts without it:

```sh
# If .locus-keys/ is empty, recover the key from the bundle first:
# see RELEASE-RECOVERY.md §2.
gh secret set LOCUS_UPDATE_KEY --repo vasilliourous/Locus < .locus-keys/locus_update.key
gh secret list --repo vasilliourous/Locus   # confirms it exists; never prints the value
```

Use the **same** key, not a new one — its public half is compiled into every
installed client, so a replacement key produces updates those clients all refuse.
Full detail, including which operations do and do not preserve secrets, is in
`client/docs/SIGNING.md` §4a.

> **The key is in three places since the 2026-10-16 rotation** — the Actions
> secret, an offline copy, and `LOCUS_UPDATE_KEY_B64` inside
> `server/secrets.env.age`. Before that it existed only in the Actions store,
> which is write-only, so when it was needed it could not be retrieved and no
> release could be signed. 3.3.0 replaced it rather than recovering it.
>
> **If any of the three is missing**, do not generate a replacement key as a
> reflex: a new key is a breaking change for every installed client. Follow
> [`RELEASE-RECOVERY.md`](RELEASE-RECOVERY.md), which covers a missing key file, a
> missing Actions secret, and a fleet still on the old key — in the order that
> avoids the second failure.

### Check the account can run CI at all, before tagging

**Added 2026-10-17, after a tag that produced nothing.** A release needs Actions
minutes, and a **private repository on a Free plan has none**. The failure looks
like storage, not billing:

```
##[error]Failed to CreateArtifact: Artifact storage quota has been hit.
```

Deleting artifacts does not help (the repository read 0 bytes and still failed).
Full diagnosis and the cascade that follows — `verify` uploads `frontend-dist`,
the builds download it, `release` downloads it too, so there is no release — is in
[`CI-CD.md`](CI-CD.md). One line to check first:

```sh
gh api repos/:owner/:repo -q .private   # true on a Free plan means CI cannot run
```

### 3.3.1 — a worked example of the whole path (2026-10-17)

Recorded because it exercised every gate at once, and because the ordering is the
lesson.

1. `pnpm release-version 3.3.1` wrote every site in `[client.version_sites]`;
   `check-consistency.sh` green.
2. The **committed** tree failed §8 — two documents (`RELEASE-RECOVERY.md`,
   `DOC-AUDIT.md`) existed only in the working tree while nine documents linked to
   them. A local run passed and CI failed, because the difference between the two
   was exactly the uncommitted files. **A local green run is evidence about your
   working tree, not about what you are about to tag.**
3. The tag was replaced (`git tag -d` + re-tag + `--force` push) after that fix.
   Safe only because no release had been created from it — check with
   `gh release view v<version>` first.
4. The first tag run produced **no release** and a `verify` failure that had
   nothing to do with the code: account billing (above).
5. After the account was unblocked, `verify` → four builds → `release` all passed.
   21 assets published, `manifest.json` reading `3.3.1` with all four platforms
   signed.
6. Artifacts cleared afterwards (~800 MB) — the release had already copied what it
   needed, and deleting Actions artifacts does not touch a published release.

**Two things this release proved green that had never been run:**
`verify` succeeded with the new guards §30–§34 in place, and **macOS Intel built
and signed** — the platform behind the original defect report.

---

## Publishing a build to the hub

Two routes, both starting from a GitHub Release that CI created:

**The console (normal).** Releases → enter the version → **Fetch & publish**. The
hub pulls the four binaries and their signatures straight from GitHub, verifies
each one's format, SHA-256 and signature, and writes `update_config`. Nothing
large travels from a browser.

**The CLI**, for a hand-built or hotfixed binary that is not on a GitHub Release:

```bash
# Validate first — fetches from GitHub, touches nothing
DRY_RUN=1 server/scripts/publish-release.sh 3.1.0 --from-github

# Publish. There is no rollout to set; publishing is offering.
PB_ADMIN_EMAIL=admin@networkingguides.jadedns.uk PB_ADMIN_PASS=... \
  server/scripts/publish-release.sh 3.1.0 --from-github
```

Omitting `--from-github` uses files from `RELEASE_DIR` (default
`./release-artifacts`). The script **refuses to publish** when an artifact is under
1MB (a truncated download or Git LFS pointer), when a hash disagrees with
`manifest.json`, or when a signature is missing.

### Why signatures are not optional

The Tauri updater verifies a minisign signature over every download and offers no
bypass. A release published without one is **installable by nobody**, while looking
perfectly healthy from the hub's side. `fetch-release.py`, `publish-release.sh` and
`releases.set` all refuse it — the checks exist so this fails at publish time rather
than as "updates silently never arrive". See `client/docs/SIGNING.md`.

### Confirm the hub is serving it

```bash
curl -s https://networkingguides.jadedns.uk/api/release
curl -s "https://networkingguides.jadedns.uk/api/update?version=2.2.0&platform=linux"
server/scripts/verify-release.sh 3.1.0      # read-only; hashes the SERVED bytes
```

`verify-release.sh` is the right tool: it checks the `update_config` row **and**
re-downloads each artifact to confirm the served bytes match the recorded hash.

### The macOS download, which is not an update

CI also produces `installer-Locus_<v>_amd64.zip` and `..._arm64.zip` — the
compressed, **ad-hoc-signed** `Locus.app`. The hub fetches them, and the console's
Releases page links them with the verified size, format and SHA-256.

They are **not** update payloads and never appear in `update_config`, CI's
`manifest.json` platform map, or the hub's `PLATFORMS`. Both sides actively refuse
a `.zip` in an update slot (`check-consistency.sh` §1c), because a compressed
bundle handed to the updater is the 2026-10-01 Windows mistake in a new costume —
a legitimate release asset that is wrong in the slot.

**A missing macOS zip does not block a publish.** It is staged when present and
reported when absent, so a broken macOS packaging step can never stop a Windows or
Linux hotfix shipping. The console shows it either way.

**Why it exists:** ad-hoc signing moves the Gatekeeper failure from
*"damaged and can't be opened"* (no bypass, Terminal only) to *"unidentified
developer"* (System Settings → Privacy & Security → **Open Anyway**). The `.dmg`
is still the normal install; the zip is what to hand a student whose download was
refused. See [`OPS.md`](OPS.md) → "Notes & limitations".

### ⚠️ There is no rollout, and no automatic downgrade

Publishing **is** offering. `active` (boolean) is the only off switch, and
`releases.set` refuses to activate a release whose artifacts are missing, unsigned,
or point at a different version.

**Stopping only stops OFFERING** — clients that already updated stay updated. A bad
build can only be fixed by publishing a higher version, which the possibly-broken
client must then successfully fetch. So **test on real hardware before publishing**,
not before widening a rollout, because there is no rollout to widen.

---

## Related

- `../reference/API.md` — the `/api/release` contract the hub serves.
- `SECRETS-MANAGEMENT.md` — how server credentials are stored and rotated.
- `server/scripts/publish-release.sh` — pushing a built release to the hub.
- `server/scripts/fetch-release.py` — the GitHub-fetch path used by the console
  publish hook.
- `server/scripts/verify-release.sh` — confirm `/updates/<version>/` and the
  `update_config` row agree.

---

## History — the removed release-cutting path (for context only)

**Do not reintroduce any of it.** A root `VERSION`, `bump.sh`, the syso stamping
and `scripts/release-cut.sh` drove **only** the retired Wails client; the shipping
fork was never wired into them, which is why they were removed rather than
repaired. Three failure modes are worth remembering:

- **A committed binary resource can be stale while every text check passes.** The
  generated directive and the committed artifact disagreed at 2.1.0 and again at
  2.2.7. Never ship a generated artifact that nothing regenerates.
- **`--no-verify` skipped the artifact update, not just the tests.**
- **A skipped CI job renders as a green check.** Seven green checks, no release.

The full old pipeline is in git history at the commit that deleted
`.github/workflows/build.yml`.
