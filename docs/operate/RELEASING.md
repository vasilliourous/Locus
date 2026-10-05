# Publishing a Release to the Hub

```
audience:    human-operator
status:      live
authoritative-for: cutting a release (tag → CI → hub publish)
verified-against: client/scripts/release-version.mjs, server/scripts/publish-release.sh
```

> **Note:** the old release-cutting machinery is gone. `scripts/release-cut.sh`,
> root `bump.sh`, `bump-version.sh`, `stamp-syso.py`, `smoke-bump.sh`, root
> `VERSION`, the committed `rsrc_windows_*.syso`, and `.github/workflows/build.yml`
> are **deleted** — they versioned the retired Wails client and never the fork.
> What replaced them:
> - CI at `.github/workflows/client.yml` (**repo root** — GitHub ignores a workflow
>   under `client/.github/`). A `v*` tag builds, **signs** and releases all four
>   platforms plus `manifest.json`.
> - The client versions itself (`client/package.json`, `client/src-tauri/Cargo.toml`,
>   `client/src-tauri/tauri.conf.json` must agree, enforced by
>   `client/src-tauri/tests/version_consistency.rs`). There is no root `VERSION`.
>
> **A bump is one command.** `pnpm release-version X` (from `client/`) writes
> **all five** version sites: `package.json`, `src-tauri/Cargo.toml`,
> `src-tauri/tauri.conf.json`, `client/Cargo.lock`, **and `docs/state.toml`'s
> `[client.version]`**. The data file is the one that is easy to miss — it is not
> a manifest, it records the client version as a **derived** fact with
> `provenance = "derived"`, and `check-consistency.sh` §7 checks it against
> `client/package.json`, failing the `verify` job with `BAD client.version`.
> The script handles it so a release cannot forget it; if a sixth site ever
> appears, add it to `client/scripts/release-version.mjs`. `cargo fetch --locked`
> separately catches a stale `Cargo.lock` in each build job.
>
> **The list is data, not prose.** `docs/state.toml` `[client.version_sites]`
> holds exactly these five paths with `provenance = "derived"`, and
> `check-consistency.sh` §9 recomputes it from `release-version.mjs` and fails on
> disagreement — so "all five" cannot drift, and a sixth site fails the build
> instead of silently invalidating this sentence.
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
gh secret set LOCUS_UPDATE_KEY --repo vasilliourous/Locus < .locus-keys/locus_update.key
gh secret list --repo vasilliourous/Locus   # confirms it exists; never prints the value
```

Use the **same** key, not a new one — its public half is compiled into every
installed client, so a replacement key produces updates those clients all refuse.
Full detail, including which operations do and do not preserve secrets, is in
`client/docs/SIGNING.md` §4a.

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
PB_ADMIN_EMAIL=admin@networkingguides.duckdns.org PB_ADMIN_PASS=... \
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
curl -s https://networkingguides.duckdns.org/api/release
curl -s "https://networkingguides.duckdns.org/api/update?version=2.2.0&platform=linux"
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

The following is kept because it explains why certain files disappeared and
which failure modes the descendants of this project already hit. **None of the
commands below work any more** — the scripts they name are deleted.

The old flow was, in order:

1. `./bump.sh <arg> --no-verify` — rewrote six version-bearing files in
   `legacy/wails-client/` and stamped `rsrc_windows_{amd64,arm64}.syso` via
   `stamp-syso.py`. It deliberately did not touch git.
2. A stale-artifact gate read the version back out of the committed `.syso`
   bytes and refused to tag if they did not match root `VERSION`.
3. `git add -A && commit && tag -a v<version>`, then push `main` **and** the tag
   (via `scripts/release-cut.sh`).
4. CI built four platforms on the tag and created the GitHub Release.

Failures worth remembering if a successor reintroduces any of this:

- **A committed Windows resource can be stale while every text check passes.**
  The `go:generate` *directive* said `2.2.8`; the committed `.syso` *artifact*
  said `2.2.6`. Happened at 2.1.0 (stamped `2.0.0`, product `MyVPN`) and again at
  2.2.7. This is why the gate eventually read the version out of the binary.
- **`--no-verify` skipped the artifact update, not just the tests.** Fixed by
  making stamping run before the flag was consulted. (`FIXES.md` 57.)
- **A skipped CI job renders as a green check.** The `Create Release` job was
  gated on `refs/tags/v*`; a branch-only push skipped it and showed seven green
  checks with no release. Re-running the workflow replayed the same skip —
  the fix was to push the tag.
- Root `VERSION` and this tooling drove **only** the archived Wails client. The
  shipping fork was never wired into it. That mismatch is the reason the
  machinery was removed rather than repaired.
