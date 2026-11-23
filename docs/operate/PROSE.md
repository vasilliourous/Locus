# Writing — how a prose change travels from `PROSE.md` into a shipped build

```
audience:    whoever edits the app's wording
status:      live
authoritative-for: the procedure for applying a prose change, and what it does not reach
verified-against: client/scripts/prose-manifest.mjs, client/scripts/apply-prose.mjs, .github/workflows/client.yml
```

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
every user-facing string in the app — 333 of them — each with three facts:

| Column | What it is | Why you need it |
|---|---|---|
| **Key** | `home.components.connection.connect` | the identity. The applier matches on this, not on the text. |
| **Text** | `Connect` | **the only cell you change.** |
| **Code** | `src/pages/connection.tsx:154` | where the string is *consumed*. Not where it is stored. |

The third column is the one people misread. A string's storage is always
`src/locales/en/home.json` (or one of the four sibling bundles), but its
**consumer** can be far away — the service-blocker sentences are rendered from
`pages/connect-notice.ts`, and the sign-out outcome strings from
`components/shared/sign-out-card.tsx`. That column is how you find the screen when
a rewrite needs the surrounding code to make sense.

**Every row has one.** A `—` in that cell is a bug, not a string without a home:
it means the scan could not find the call site, and the two ways that happens
have both already happened in this repo — i18next's selector form
`i18n.t(($) => $.a.b.c)`, where the key is a bare property path and not a quoted
string, and a Rust file list resolved against the wrong root. Both are now
guarded by `tests/prose-locate.test.ts`, which fails when any row's Code cell is
empty.

### The headings are the screen map

Strings are grouped in the order a student meets them: Activation gate, Suspension
screen, Connection, Support block, Sign-out, Account, Traffic, Sidebar, Dialogs,
Toasts and notifications, Network readouts, Error screen, and Native strings (the
tray, OS notifications and the macOS launch guard — those live in Rust and a YAML
file, not in the client bundles).

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
| Cross-repo | `bash server/scripts/check-consistency.sh` | **§41**: the support address must exist exactly once. See the trap below. |

`prose:check` is also wired into the `verify` job of
`.github/workflows/client.yml`, so a stale manifest fails CI rather than being
noticed by whoever next opens the file. It is a string comparison — it cannot be
satisfied by anything but the real generated output.

**What `prose:check` cannot see, and the test that covers it.** `--check` compares
the generated text against the committed file. If the generated Code column was
*wrong when it was generated*, comparing it to itself is clean — a stale manifest
and a differently-stale manifest agree. That is how four live strings and all 68
native rows sat behind a `—` with every gate green. `tests/prose-locate.test.ts`
runs inside `pnpm test`, calls the same `scripts/lib/prose-locate.mjs` the
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
rewrite therefore has three possible outcomes per locale:

| That locale's value | What a student sees after you apply | Finished? |
|---|---|---|
| a real translation | the **old** meaning, in their language | **no** |
| a copy of the old English | the **old** English | **no** |
| the key is absent | the new English, by fallback | yes |

So a rewrite of a string that someone translated is **not live for that student**
until the translated value is updated. That is not a bug to fix by copying English
over the top — that would replace a real translation with English, which is worse
than a stale one. It is a per-language translation task, and it is reviewed per
language.

`pnpm i18n:check` reports the drift as `missing` per locale. A value *identical*
to English in a locale that normally translates cannot be distinguished from a
fallback copy by any tool, so the count is a floor, not a total.

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
2. **Push to `main`.** `client/**` is in the workflow's `paths` filter, so the run
   starts; `verify` runs the four gates and `check-consistency.sh`. A prose-only
   push also refreshes the **preview installers**, because `preview` runs for every
   push to `main` — so a copy change can be installed and read on a real screen
   without cutting a release. **Read the words on that screen.** No gate in this
   repository can tell you whether a sentence is clear.
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

- The gate list in §4 was run by hand on this tree, not observed in CI. The
  commands themselves ran green locally; whether `verify` runs them in the same
  order on a runner is read from `client.yml`, not watched.
- The claim that `preview` publishes on a prose-only push is read from
  `client.yml`'s `paths` filter and `preview`'s `if:`. It was not watched running.
- The per-locale coverage in §5 is a count from `i18n:check`, not a review of any
  translation.
