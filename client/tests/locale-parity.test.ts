/// <reference types="node" />
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'

import { describe, expect, it } from 'vitest'

/**
 * Every locale carries every English key.
 *
 * # Why this exists
 *
 * `src/locales/en/` is the source of truth and every other locale is a copy of
 * its KEY STRUCTURE with translated values. Nothing enforced that. On
 * 2026-11-23 eleven new UI surfaces (sign-out, the suspension screens, the
 * account panel, the free-data bar, the activation form — 56 keys in
 * `home.json` alone) had landed in English and in **none** of the other twelve
 * bundles, and no gate noticed: `pnpm i18n:check` reports missing counts but
 * exits 0, and it is not a CI step.
 *
 * `docs/brief`-era reasoning and `client/docs/CONTRIBUTING_i18n.md:52` say the
 * structural rule out loud — keep locales aligned with `src/locales/en/` — so
 * this is a guard on a stated rule, not a new policy.
 *
 * # What it deliberately does NOT check
 *
 * The VALUES. A locale may legitimately copy the English string as a stopgap
 * (`CONTRIBUTING_i18n.md:52`: "other locales may temporarily copy English if no
 * translation is available yet") and `zh` is the fallback for a missing key, so
 * an untranslated value is visible-but-honest rather than broken. A missing KEY
 * is different: it falls through to the fallback language, which mixes two
 * languages on one screen without anything failing.
 *
 * # What it does not cover, stated so the gap is visible
 *
 * New keys added to `en/` and the SAME change's absence in a locale is what
 * this catches. It cannot tell whether a translated value is any good, and it
 * says nothing about the Rust bundles (`crates/clash-verge-i18n/locales/*.yml`),
 * which are independently at full parity — measured, not guarded.
 */

const LOCALES = path.resolve(__dirname, '../src/locales')
const SOURCE = 'en'

type Json = string | number | boolean | null | Json[] | { [key: string]: Json }

const flatten = (node: Json, prefix = ''): string[] => {
  if (node === null || typeof node !== 'object' || Array.isArray(node)) {
    return [prefix]
  }
  return Object.entries(node).flatMap(([key, value]) =>
    flatten(value as Json, prefix ? `${prefix}.${key}` : key),
  )
}

const load = (locale: string, file: string): Set<string> =>
  new Set(
    flatten(
      JSON.parse(
        readFileSync(path.join(LOCALES, locale, file), 'utf8'),
      ) as Json,
    ),
  )

const read = (locale: string, file: string): Json =>
  JSON.parse(readFileSync(path.join(LOCALES, locale, file), 'utf8')) as Json

/** `path → value` for every string leaf. The key side of `flatten` discards the
 * value, which is what the parity check needs and this one does not. */
const leaves = (node: Json, prefix = ''): Map<string, string> => {
  const out = new Map<string, string>()
  if (node !== null && typeof node === 'object' && !Array.isArray(node)) {
    for (const [key, value] of Object.entries(node)) {
      for (const [k, v] of leaves(
        value as Json,
        prefix ? `${prefix}.${key}` : key,
      ))
        out.set(k, v)
    }
  } else {
    out.set(prefix, String(node))
  }
  return out
}

const sourceFiles = readdirSync(path.join(LOCALES, SOURCE))
  .filter((f) => f.endsWith('.json'))
  .sort()
const targetLocales = readdirSync(LOCALES)
  .filter((l) => l !== SOURCE && !l.startsWith('.'))
  .sort()

describe('locale key parity', () => {
  it('finds the English bundles to compare against', () => {
    // A guard whose subject list is empty passes silently. Assert the walk found
    // something before asserting anything about it.
    expect(sourceFiles.length).toBeGreaterThan(0)
    expect(targetLocales.length).toBeGreaterThan(0)
  })

  for (const file of sourceFiles) {
    for (const locale of targetLocales) {
      it(`${locale}/${file} carries every en key`, () => {
        const expected = load(SOURCE, file)
        const actual = load(locale, file)
        const missing = [...expected].filter((k) => !actual.has(k)).sort()

        expect(
          missing,
          `${locale}/${file} is missing ${missing.length} key(s) present in en/${file}. ` +
            `A missing key falls through to the fallback language, so the screen ` +
            `renders in two languages and nothing else fails.`,
        ).toEqual([])
      })
    }
  }
})

/**
 * A whole screen must not ship in English in twelve languages.
 *
 * # The defect this catches, which the parity check above cannot
 *
 * On 2026-11-25 the activation gate and the suspension screen landed in all
 * thirteen bundles at once — as **English text in every one of them**. `zh` had
 * `"Enter your code"`; so did `ru`. The parity check above is green over that
 * by design (it compares KEYS), and `pnpm i18n:check` cannot see it either: it
 * reports a key present in `en` and absent elsewhere as `missing`, and a value
 * *identical* to English is indistinguishable from a fallback copy by any
 * tool — the exact gap `docs/operate/PROSE.md` §5 names and does not close.
 *
 * So this is that gap, closed for the one surface where it was total. The
 * subject list is by handout: `TODO-I18N.md` is where a translator takes a
 * screen, and a screen is **finished** only when its row leaves that list with
 * the twelve bundles updated. The earlier version of this guard had parity for
 * the activation screen while the screen itself was 13/13 English.
 *
 * # Why `en` only, and why it is not over-fitted
 *
 * `supportEmail` is identical to English in every bundle on purpose (`§41`:
 * the address exists once, in `en`, and is duplicated by `i18n:format`), so an
 * address-like value is skipped rather than asserted. Nothing else here is a
 * style choice: a translated screen with English left in it is the defect.
 */
const HANDOUT = path.resolve(__dirname, '../TODO-I18N.md')

/** Surfaces the handout says are finished. Each maps to the keys it owns. */
const FINISHED_SURFACES: Record<string, (key: string) => boolean> = {
  activation: (k) => k.startsWith('components.connection.activation.'),
  suspended: (k) => k.startsWith('components.connection.suspended.'),
}

/**
 * Keys that are a FORMAT, not a sentence: every bundle must carry the identical
 * string, and `codePlaceholder` is the activation field's input mask
 * (`RQ-XXXX-XXXX-XXXX-X`, `activation.tsx:327`) — the same letter-shape the
 * `alphabet` line teaches, so translating it would teach a mask the code
 * generator cannot emit. Exempt by key, and named here only where the string is a format
 * nowhere: a new key added here has to be argued for, not inherited.
 */
const NOT_PROSE: ReadonlySet<string> = new Set([
  'components.connection.activation.codePlaceholder',
])

const emailish = (value: string) => /@/.test(value) && !/\s/.test(value)

describe('no finished surface ships in English', () => {
  it('reads a handout with at least one finished surface in it', () => {
    // A guard whose subject list became empty — the file deleted, every row
    // struck out — would pass silently. Assert the subject before asserting
    // anything about it.
    const text = readFileSync(HANDOUT, 'utf8')
    expect(Object.keys(FINISHED_SURFACES).length).toBeGreaterThan(0)
    for (const name of Object.keys(FINISHED_SURFACES)) {
      expect(text, `${name} is not named in TODO-I18N.md`).toContain(name)
    }
  })

  it('every NOT_PROSE key is a format, and a surface still owns prose do', () => {
    // The exemption list above is the one place this guard can be made green
    // dishonestly. Assert its subject: every key named is real English prose
    // text, and the finished surfaces still contribute keys that are NOT
    // exempt — so a bundle cannot pass this file by exempting the whole screen.
    const english = leaves(read(SOURCE, 'home.json'))
    for (const key of NOT_PROSE) {
      expect(english.has(key), `${key} is not an en key`).toBe(true)
    }
    const owned = [...english.keys()].filter((k) =>
      Object.values(FINISHED_SURFACES).some(
        (owns) => owns(k) && !NOT_PROSE.has(k),
      ),
    )
    expect(
      owned.length,
      'no finished surface has any prose left to assert',
    ).toBeGreaterThan(0)
  })

  for (const [surface, owns] of Object.entries(FINISHED_SURFACES)) {
    for (const file of sourceFiles) {
      const english = leaves(read(SOURCE, file))
      const subject = [...english.keys()].filter(owns)
      if (subject.length === 0) continue

      for (const locale of targetLocales) {
        it(`${surface}: ${locale}/${file} is translated, not English`, () => {
          const translated = leaves(read(locale, file))
          const untranslated = subject.filter((key) => {
            const value = english.get(key) as string
            if (emailish(value) || NOT_PROSE.has(key)) return false
            return translated.get(key) === value
          })

          expect(
            untranslated,
            `${locale}/${file} renders ${untranslated.length}/${subject.length} ` +
              `of the \`${surface}\` surface in English (${untranslated
                .slice(0, 4)
                .join(', ')}${untranslated.length > 4 ? ', …' : ''}). ` +
              `\`prose:apply\` writes \`en\` only, so a copy pass leaves this ` +
              `screen English for every student on a non-English device and ` +
              `nothing else fails. Translate it, or take the surface out of ` +
              `\`TODO-I18N.md\` and out of \`FINISHED_SURFACES\` in the same ` +
              `change — never one without the other.`,
          ).toEqual([])
        })
      }
    }
  }
})
