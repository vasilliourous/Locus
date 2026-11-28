# TODO — screens that are finished, and the twelve languages that are not

```
audience:    whoever translates the app, and whoever applies a prose change
status:      live
authoritative-for: which surfaces are asserted to be translated, and which are not yet
verified-against: client/tests/locale-parity.test.ts, client/scripts/apply-prose.mjs, client/src/locales/
```

> **Why this file exists.** `pnpm prose:apply` writes **English only**. Nothing in
> this repository could tell a translated screen from an English one — a value
> *identical* to English is indistinguishable from a fallback copy by any tool
> ([`docs/operate/PROSE.md`](../docs/operate/PROSE.md) §5). The first pass
> measured the damage: on 2026-11-25 the activation gate and the suspension
> screen shipped as **English text in all thirteen bundles** — `zh` had
> `"Enter your code"`, and so did `ru` — while every gate was green.
>
> So a surface is listed below **only when it is translated**, and a test fails
> the build for any listed surface that is still English in any locale.

> **This is a release step, not a suggestion.** `server/scripts/release-gate.sh`
> (step 6) runs the guard below before a tag may be cut, alongside the check that
> no wording is left waiting to be applied. The order — apply, regenerate, then
> bump the version — is [`docs/operate/RELEASING.md`](../docs/operate/RELEASING.md)
> step 1; skipping it is the defect this document exists to stop.

## The list is the guard

`client/tests/locale-parity.test.ts` → *"no finished surface ships in English"*
holds a `FINISHED_SURFACES` map. Each name there must appear in this file, and
each listed surface must be translated in all twelve non-English bundles.

**A surface is finished when, and only when, both are true:**

1. every key it owns reads as that language in every bundle, and
2. its row has left the table below and joined the finished list above it.

Do not add a surface to `FINISHED_SURFACES` to make a red build green. That is
the whole failure this file was written to stop: the guard is worth exactly as
much as its list is honest.

## Finished

| Surface | Keys | Since |
|---|---|---|
| **Activation gate** | `components.connection.activation.*` — 7 keys, `src/pages/activation.tsx` (`codePlaceholder` is an input mask, exempt and named in `NOT_PROSE`) | v3.5.2 |
| **Suspension screen** | `components.connection.suspended.*` — 6 keys, `src/pages/suspended.tsx` | v3.5.2 |

## Not finished

Each row is a screen a student meets, and the count is *values identical to
English*, measured across the twelve bundles. A floor, not a total: a real
translation that happens to match English (a product name, an address) counts.

| Surface | Where it is | Measure | What closes it |
|---|---|---|---|
| Support block, account panel, sign-out, traffic, dialogs, toasts | the rest of `src/locales/*/` | ~50% of all values are identical to English | One surface per pass — translate a screen, add it to `FINISHED_SURFACES`, keep it green. Do not bulk-copy English over a real translation. |
| **The session probe card** | `components.connection.account.probe.*` — 9 keys, `src/components/shared/probe-card.tsx` | **9 of 9 English in all twelve bundles** (added 2026-11-28) | Translate the nine. It is the smallest surface in this table and the one an operator is most likely to ask a student to read, so it is a good first pass. |

## How a pass is run

```sh
cd client

# 1. which keys of a surface are still English, per locale
pnpm exec vitest run tests/locale-parity.test.ts

# 2. translate them in src/locales/<locale>/<ns>.json — by hand, or by a
#    translator. Never `i18n:format`: it copies `supportEmail` into every
#    bundle and §41 of `check-consistency.sh` then fails the build.

# 3. the gates that decide whether the job is done
pnpm test && pnpm typecheck && pnpm lint && pnpm prose:check
```

Adding a **new** key still goes through `en` first (`pnpm i18n:types`, then
`pnpm prose`); this file governs the step after that, which had no owner.

## What this does not cover

- **Whether a translation is any good.** The guard proves it is not the English
  string. It says nothing about accuracy, register or a placeholder that moved.
- **Plural forms.** `{{count}} days` has one form in Japanese and three in
  Russian; a rewrite that changes which case a number falls into fails here
  only if it also leaves English in place.
- **The native strings.** `crates/clash-verge-i18n/locales/*.yml` holds the tray,
  the notifications and the macOS guard, compiled by `rust_i18n` at build time.
  They are at parity with `en`, and nothing guards their *values* either.
