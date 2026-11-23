/// <reference types="node" />
import { describe, expect, it } from 'vitest'

import {
  backendCode,
  locate,
  nativeLeafKeys,
} from '../scripts/prose-locate.mjs'

/**
 * Every row in `PROSE.md` points at a real consumption site.
 *
 * # Why this exists
 *
 * `PROSE.md`'s Code column is the only answer to "where does this string come
 * from" — the row's storage is always a locale bundle, so without it a copy
 * editor has to grep. `prose-manifest.mjs --check` compares generated text
 * against the committed file, so it **cannot** see a Code cell that was already
 * wrong when it was generated: a stale manifest and a differently-stale manifest
 * compare clean against themselves.
 *
 * That is not hypothetical, and it happened twice in the same file:
 *
 *   · `locate()` knew four literal forms of a key and missed i18next's selector
 *     form `i18n.t(($) => $.a.b.c)`, where the key is a **bare property path**
 *     with no quotes. Four live strings rendered a Code cell of `—`, which reads
 *     as "a student cannot reach this". Two of them are thrown at the student
 *     (`services/api.ts`) and one is a port validation message
 *     (`hooks/use-clash.ts`).
 *   · `backendCode()` searched a file list whose entries carried a `client/`
 *     prefix that `ROOT` already supplied, so every file missed and **all 68**
 *     native rows read `—`.
 *
 * So a dash is a failure here. It is either a string nothing consumes or a
 * broken scan, and the two cases above were both the second.
 *
 * # What it does not check
 *
 * That the line number is still accurate. Line drift is `--check`'s job and is
 * enforced in CI. A cell that is empty is a different, harder failure: it is the
 * scan finding nothing at all.
 */

describe('PROSE.md Code column', () => {
  it('resolves the two selector-form keys that used to read as unreachable', () => {
    // Named explicitly, so a regression reports the string rather than a count.
    //
    // THESE TWO USED TO BE FOUR. The ipInfo pair was the other half of the
    // selector-form blind spot, and it is resolved by the SAME fix — but its
    // only consumer, `services/api.ts`, was itself reachable from nothing, so
    // the strings were never rendered to a student. The file is gone
    // (2026-11-23); the two that remain are genuinely reachable. Asserting the
    // removed pair would re-add a dependency on dead code to reach a pass.
    const previouslyBlank = [
      'settings.modals.clashPort.messages.portTooHigh',
      'settings.modals.clashPort.messages.portTooLow',
    ]
    for (const key of previouslyBlank) {
      expect(
        locate(key),
        `${key} is rendered to a student but locate() found no call site. ` +
          'It is written in the selector form i18n.t(($) => $.a.b.c) — check ' +
          'that form is still matched, not just the quoted forms.',
      ).not.toHaveLength(0)
    }
  })

  it('resolves every native key, whose leaves are not unique', () => {
    // `title`, `body`, `on` and `off` appear in every notification, so searching
    // the leaf — which is what this used to do — can never be right. The whole
    // dotted path is what Rust names.
    const keys = nativeLeafKeys().map((entry) => entry.key)
    expect(keys.length).toBeGreaterThan(60)
    const unresolved = keys.filter((key) => backendCode(key).length === 0)
    expect(
      unresolved,
      'Native strings whose Rust call site could not be found. A `—` in the ' +
        'Code column of a native row reads as "unreachable", but these are all ' +
        'passed to t! — check the file list in backendCode() still resolves ' +
        'against ROOT.',
    ).toEqual([])
  })
})
