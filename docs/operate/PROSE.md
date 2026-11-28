# Writing — how a prose change travels from `PROSE.md` into a shipped build

```
audience:    whoever edits the app's wording
status:      live
authoritative-for: the procedure for applying a prose change, and what it does not reach
verified-against: client/scripts/prose-manifest.mjs, client/scripts/apply-prose.mjs, client/scripts/prose-locate.mjs, server/scripts/release-gate.sh, .github/workflows/client.yml
```

> **Refreshed, not rewritten.** The last full revision of this document was
> before the Locus→Chrysalis rename and before the manifest grew the
> "Deliberately not in the manifest" and "Not English-only" sections. Every
> count and path below was re-derived from the tree and checked against
> `prose-manifest.mjs --check`; where a number disagreed with the tree the tree
> won. §8 says which claims are still read rather than run.

> **The one-line version.** Edit the Text column of `client/PROSE.md`, run
> `pnpm prose:apply`, regenerate with `pnpm prose`, run the gates, commit. Then
> read §5, because "applied" does not mean "the app now says it" — unless you are
> on English.

This document is the procedure. What each string *is* belongs in
[`PROSE.md`](../../client/PROSE.md); what the app *looks like* belongs in
[`/COLOURS.md`](../../COLOURS.md).

---

## 1. What you are editing

`client/PROSE.md` is **generated from the tree**, not maintained by hand. It holds
every user-facing string in the app — **330** of them — each with three facts:

That 330 is not "every key in `en.json`". It is the keys a student can reach:
262 frontend leaves and 70 backend (`en.yml`) leaves, minus the strings listed
under **Deliberately not in the manifest**. The generator prints the exact split
and its own count on every run (`PROSE.md written: N strings, M out of scope`),
which is why the count is not repeated anywhere as a hard-coded literal — see
[`docs/state.toml`](../state.toml).

| Column | What it is | Why you need it |
|---|---|---|
| **Key** | `home.components.connection.connect` | the identity. The applier matches on this, not on the text. |
| **Text** | `Connect` | **the only cell you change.** |
| **Code** | `src/pages/connection.tsx:154` | where the string is *consumed*. Not where it is stored. |

The third column is the one people misread. A string's storage is always
`src/locales/en/home.json` (one of five bundles: `home`, `layout`, `profiles`,
`settings`, `shared`), but its
**consumer** can be far away — the service-blocker sentences are rendered from
`pages/connect-notice.ts`, and the sign-out outcome strings from
`components/shared/sign-out-card.tsx`. That column is how you find the screen when
a rewrite needs the surrounding code to make sense.

**Every row has one.** A `—` in that cell is a bug, not a string without a home:
it means the scan could not find the call site, and the two ways that happens
have both already happened in this repo — i18next's selector form
`i18n.t(($) => $.a.b.c)`, where the key is a bare property path and not a quoted
string, and a Rust file list resolved against the wrong root. Both are now
`prose-locate.mjs` (note: `client/scripts/`, not `client/scripts/lib/` — it was
moved out of the manifest so the test could import it without running the
generator). It fails when any row's Code cell is empty.

### The headings are the screen map

Strings are grouped in the order a student meets them, and there are twelve
screen headings: Activation gate, Suspension screen, Connection tab, Support
block, Sign-out card, Account tab, Traffic graph and figures, Sidebar and
navigation, Dialogs, Toasts and notifications, Error screen, and **Native
strings** (the tray, OS notifications and the macOS launch guard — those live in
Rust and a YAML file, not in the client bundles).

Two more sections are not screens and are the ones people miss:

| Section | What it is |
|---|---|
| **Deliberately not in the manifest** | Bundle keys a student cannot reach — `profiles.*` (the removed profiles UI), the unrendered `components/base/` control set (`shared.actions`, `shared.placeholders`, `shared.validation`, `shared.editorModes`, `shared.statuses`), and `shared.labels.retryAttempts`. Each carries the reason it is still shipped. A copy pass should read this *first*, so it does not "fix" a string no screen renders. Read the table as it is generated: an entry disappears when its last key does, so it is not a fixed list. |
| **Not English-only: what a rewrite does and does not reach** | The per-locale reality. The long form of §5 below; the generated copy sits next to the strings so a copy editor who never opens this document still meets it. |

"Network readouts" is no longer a heading of its own — those strings now sit
under **Error screen** and **Traffic graph and figures**.

If you want to change what a *screen* says, work down one heading. If you want to
change one *concept* — a word used in six places — use the Key column: a stable
word is one spelling everywhere, and the manifest is the only place all spellings
are visible at once.

---

## 2. The procedure

```
cd client

# 1. edit the Text column in PROSE.md — nothing else
$EDITOR PROSE.md

# 2. see exactly what would change, before anything is written
pnpm prose:apply --dry-run

# 3. write it
pnpm prose:apply

# 4. regenerate the manifest from the new state
pnpm prose

# 5. the gates (§4)
pnpm typecheck && pnpm test && pnpm lint && pnpm prose:check
```

`pnpm prose:apply` prints every row it changed as a before/after pair. **Read that
output.** It is the last point at which a mistake is cheap.

### This is a step in cutting a release, and the gate enforces it

A prose change is a **content change**, so in [`RELEASING.md`](RELEASING.md) the
order is:

```
edit PROSE.md → prose:apply → prose → the gates → THEN the version bump
```

The bump is the last content change, so a reworded string and the version that
carries it land in the same commit and the same tag.

**`server/scripts/release-gate.sh` refuses to pass with prose out of order.** Step
6 of that gate runs, before any tag exists:

| It asserts | Against | Why it is a release concern |
|---|---|---|
| `prose:apply --dry-run` would write **nothing** | `client/` | PROSE.md was edited and never applied: the document changed and no student sees it. **This is the report "the PROSE.md changes are not in the build."** |
| `prose-manifest.mjs --check` passes | `client/` | A bundle moved and PROSE.md was not regenerated — every `file:line` in it is now a lie. |
| `tests/locale-parity.test.ts` passes | `client/` | A finished surface still reads as English in a non-English bundle. `prose:apply` writes `en` only (§5), and 24 bundles' worth of English is what shipped on 2026-11-25 with every other gate green. |

It **skips** the third when `client/node_modules` is absent, and says so — a gate
that fails on a missing toolchain is a gate people learn to ignore.

What the gate cannot see, stated rather than implied: whether a translation is any
good (§5), whether the row you applied was the one you meant, and whether the
`prose:apply` you ran was committed — that last is the working-tree check in step
2 of the same script, which refuses a dirty tree at tag time.

### Why step 4 is not optional

`PROSE.md` is regenerated from the bundles, so after step 3 the file's Text column
already matches reality — but the **Code** column may not, if your edit changed a
line number. `pnpm prose:check` in step 5 catches that, and CI runs the same check
(§4).

---

## 3. What the applier refuses, and why that is the point

Five refusals. Each is a real mistake, not a hypothetical:

**Placeholders.** The app has **two** placeholder syntaxes, substituted by two
different engines, and a rewrite must keep whichever one the row is in:

| Where | Syntax | Example |
|---|---|---|
| `src/locales/en/*.json` | i18next | `{{used}} of {{total}}` |
| `clash-verge-i18n/locales/en.yml` | rust_i18n | `Switched to {mode}.` |

A rewrite that drops one renders `expires in  days`, or an OS notification
saying "Mode changed." forever — no type checker sees it, no test fails, and it
only shows up on a screen with a real value in it. The applier compares the
placeholder set of the original against the replacement and rejects the row:

```
1 row(s) NOT applied:
  home.components.connection.subscriptionExpiresIn — placeholders changed: {{count}} ->
  backend:notifications.clashModeChanged.body — placeholders changed: {mode} ->
Nothing was written.
```

The single-brace form was **not** guarded until it was found by testing: there is
no `{{` anywhere in `en.yml`, so the guard matched nothing in that file and every
native placeholder was unchecked. Both forms are matched now.

**A key that is not in the bundle.** The manifest has drifted from the code —
someone renamed a key, or deleted the string. Regenerate, do not force.

**A row whose key does not parse.** A hand-edited row in some other shape is
reported as unparsed rather than guessed at. The manifest's row shape is the
contract.

**A backend leaf it cannot locate.** The YAML is edited as text surgery rather
than parse-and-dump, so the file keeps its comments (and a one-line copy edit does
not become a 200-line reformat). If the leaf cannot be found, nothing is written.

**When any row fails, no file is touched.** The applier is all-or-nothing on the
write path, so a half-applied copy pass is not a state you can reach.

---

## 4. The gates

| Gate | Command | What it catches |
|---|---|---|
| Types | `pnpm typecheck` | a `t()` key that no longer exists (`TranslationKey` is generated) |
| Tests | `pnpm test` | the palette guards **and `tests/prose-locate.test.ts`**; a copy change should never touch the palette guards |
| Lint | `pnpm lint` | unused imports left behind when a label moved |
| Manifest | `pnpm prose:check` | `PROSE.md` disagreeing with the tree |
| Parity | `pnpm i18n:parity` | a finished surface still reading as English in a non-English bundle. `pnpm test` runs the same file; this alias runs **only** that file, which is what the release gate uses. |
| Cross-repo | `bash server/scripts/check-consistency.sh` | **§41**: the support address must exist exactly once. See the trap below. |

`prose:check` and `i18n:parity` are both wired into the `verify` job of
`.github/workflows/client.yml` (steps *"User-facing prose manifest is current"*
and *"Locale keys are in parity with English"*), so a stale manifest fails CI
rather than being noticed by whoever next opens the file. `prose:check` is a
string comparison — it cannot be satisfied by anything but the real generated
output.

`pnpm lint` runs `eslint … src` only, so it will not lint a stray edit under
`scripts/` or `tests/`. If you changed the generator or the applier rather than a
string, the type check and the tests are your gates; lint is silent about it.

**What `prose:check` cannot see, and the test that covers it.** `--check` compares
the generated text against the committed file. If the generated Code column was
*wrong when it was generated*, comparing it to itself is clean — a stale manifest
and a differently-stale manifest agree. That is how four live strings and all 68
native rows sat behind a `—` with every gate green. `tests/prose-locate.test.ts`
runs inside `pnpm test`, calls the same `scripts/prose-locate.mjs` the
generator does, and fails on any row whose consumption site does not resolve.

### The `i18n:format` trap

`pnpm i18n:format` (`cleanup-unused-i18n.mjs --align --apply`) is the tool for
adding a *new* key or removing a dead one. If you run it:

- **it copies `supportEmail` into every locale bundle**, and §41 then fails the
  build for having 13 copies of an address that must exist once. Strip it from the
  twelve non-English bundles afterwards.
- it rewrites every bundle's formatting, so run it as its **own commit** and never
  mixed with a copy pass. A diff that is 300 lines of reordering and one real
  change is a diff nobody reviews.

Neither of those is a defect in this procedure; they are properties of that tool,
and the procedure is to know them.

### A dead entry in `NOT_USER_FACING`

`NOT_USER_FACING` in `prose-manifest.mjs` is a list of prefixes, and the generated
table is built by iterating **only the prefixes that matched a live key**. So an
entry whose keys are all gone silently disappears from `PROSE.md` — which is the
right behaviour, but it also means the list can carry a prefix that matches
nothing. `monaco.*` is one: the editor chrome left the bundles, and the entry
remains as a reason attached to zero keys. It is harmless today and it is the
thing to check when a key you expected to be listed is not: if no generated row
names its prefix, no key in `en.json` starts with it.

### Adding or removing a string

`prose:apply` cannot create a key, only change one. A **new** string is a code
change: add it to `src/locales/en/<ns>.json`, use it from the component,
`pnpm i18n:types`, then `pnpm prose`. A **removed** string is the reverse — and
first confirm nothing still references it, because `i18n:check` reporting a key as
unused means no *component* names it, not that no *code* does. `notice-service.ts`
maps fifteen `profiles.*` keys from Rust error codes that the backend still emits;
deleting them would break a live toast that no screen mentions.

---

## 5. What a rewrite does NOT reach

This is the section that decides whether a job is finished.

`src/locales/en/` is canonical, but the app ships in **13 languages**, and each of
the other 12 owns the same key set. `prose:apply` writes English only. A row you
rewrite therefore has **four** possible outcomes per locale, and only the third
is a finished job:

| That locale's value | What a student sees after you apply | Finished? |
|---|---|---|
| a real translation | the **old** meaning, in their language | **no** |
| a copy of the old English | the **old** English | **no** |
| the key is absent | the new English, by fallback | yes |
| a value the app cannot read | nothing — the build fails | **no** |

The fourth row is the one that is not merely stale. A **native** string
(`Native strings`, `en.yml`) is read by rust_i18n at **compile time**, and a
lint-free YAML file can still fail it: a malformed `clash-verge-i18n/locales/
*.yml` stops `cargo build` with `proc macro panicked: Parse file … failed`, on
whichever platform builds it. `prose:apply` writes English only, so a rewrite
leaves the other twelve bundles untouched and valid — but if you hand-edit one,
run `cargo build -p clash-verge-i18n` before you believe it.

So a rewrite of a string that someone translated is **not live for that student**
until the translated value is updated. That is not a bug to fix by copying English
over the top — that would replace a real translation with English, which is worse
than a stale one. It is a per-language translation task, and it is reviewed per
language.

`pnpm i18n:check` reports the drift as `missing` per locale. A value *identical*
to English in a locale that normally translates cannot be distinguished from a
fallback copy by any tool, so the count is a floor, not a total.

`i18n:check` is also a **parity** check, not a translation-quality check: it reads
each locale against the English baseline and reports `unused` / `missing` /
`extra` / `missingSource`. A locale that still holds the old English under a key
that exists in both is *clean* to it — which is exactly the case §5's first two
rows describe, and exactly why `tests/locale-parity.test.ts` (not this script) is
the guard for a finished surface. On the tree this document was refreshed against,
every locale reports `unused=0, missing=0, extra=0` for both `src/locales` (262
keys) and the backend `en.yml` (70 keys); that is a clean state, not a permanent
one.

**Count-sensitive strings need the plural forms re-checked in every language.**
`{{count}} days` has one plural form in Japanese and three in Russian. i18next
will not warn if your rewrite changes which case a number falls into.

### Native strings travel differently

The 68 strings under **Native strings** live in
`client/crates/clash-verge-i18n/locales/en.yml`, not in a client bundle, and they
are read by Rust at runtime. `prose:apply` handles them (the tray menu, the OS
notifications, the macOS first-run guard), but:

- they are **cached in the built binary**, so they only change in a new build;
- the tray is a native menu rendered by the OS, so text length matters — a long
  label is clipped with no ellipsis;
- several are strings this fork inherited from Clash Verge (`Clash Verge is about
  to exit.`, `Profile Reactivated.`), so a copy pass here is also a de-branding
  pass.

---

## 6. Shipping it

A prose change is **ordinary code**. It is not a release, and it does not need a
version bump:

1. **Commit** `PROSE.md` and the locale bundles **in the same commit**. A commit
   that changes a bundle without the manifest fails `prose:check` in CI, and one
   that changes the manifest without the bundle is a document describing text the
   app does not show.
2. **Push to `main`.** `client/**` is in the workflow's `paths` filter (alongside
   `server/**`, `scripts/**`, `docs/**` and the root docs), so the run starts;
   `verify` runs the gates in §4 — including `prose:check` and `i18n:parity` —
   plus `check-consistency.sh`. A prose-only push also refreshes the **preview
   installers**, because `preview` is `if: github.event_name == 'push' && github.ref == 'refs/heads/main'`
   — so a copy change can be installed and read on a real screen without cutting a
   release. **Read the words on that screen.** No gate in this repository can tell
   you whether a sentence is clear.
3. **Releasing** is a separate, deliberate act (`docs/operate/RELEASING.md`). The
   version is not bumped by CI, so a tag that disagrees with the version files
   builds a binary that misreports itself. If the copy change should reach
   students as an update, bump with `pnpm release-version X` and follow that
   document; a prose edit alone does not justify it.

---

## 7. The failure modes this repository has already paid for

Each of these is a way a copy pass goes wrong **silently** — the app builds, the
screen renders, and the text is wrong or missing:

- **A string stated twice.** Copy that exists in two places disagrees eventually.
  `PROSE.md` is generated for exactly this reason; do not paste a string into a
  component.
- **A deleted key that something still uses.** `i18n:check`'s "unused" means "no
  component renders it". `notice-service.ts` and `notification-handlers.ts` both
  reference keys dynamically, from Rust error codes.
- **The support address.** It exists exactly once, in `src/locales/en/home.json`,
  and §41 fails the build otherwise. `i18n:format` duplicates it (§4).
- **A placeholder dropped in translation.** Renders an empty gap; no test fails.
- **A manifest that is not regenerated.** Every `file:line` in it becomes a lie,
  and nothing else in the repo notices.
- **A copy change mixed with a format change.** The real edit is invisible in the
  diff, which is how it gets reverted by the next person tidying.

The rule that covers all six: **one concern per commit, and read the applier's
before/after output before you trust it.**

---

## 8. Not verified

**Verified in this refresh** (run against the tree, output read):

- `node scripts/prose-manifest.mjs --check` → `PROSE.md is up to date`.
- `node scripts/prose-manifest.mjs` → `330 strings, 25 out of scope, 0 unattributed`.
- `node scripts/cleanup-unused-i18n.mjs` → `unused=0, missing=0, extra=0` for all
  13 locales, both `src/locales` (262 keys) and the backend `en.yml` (70 keys).
- The row counts per heading, the five `en` bundles, the 13 locales, the 12
  siblings, the 68 native rows and the 25 out-of-scope keys — all counted from
  the tree, not copied.

**Read, not run:**

- The gate list in §4 is read from `client.yml`'s step names and
  `release-gate.sh`. That both are green on a runner was not watched.
- The `preview` claim in §6 is read from `client.yml`'s `paths` filter and the
  job's `if:`. It was not watched running.
- `cargo build -p clash-verge-i18n` failing on malformed YAML (§5) is the
  generator's stated behaviour and the manifest's own text, not reproduced here.
- The per-locale coverage in §5 is a count from `i18n:check`, not a review of any
  translation. Nothing in this repository can tell you whether a sentence is
  clear, and no paragraph below changes that.
