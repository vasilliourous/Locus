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
