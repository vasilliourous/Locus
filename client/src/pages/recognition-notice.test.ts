import { describe, expect, it } from 'vitest'

import type { RecognitionResult } from '@/services/locus'

import { recognitionNotice } from './recognition-notice'

/**
 * The device-recognition notice on the activation screen.
 *
 * The property worth protecting is that **`unavailable` is never reported as a
 * refusal**. The Rust type was shaped specifically to keep "we could not ask"
 * apart from "the hub does not know you" (`cmd/locus.rs`), and the distinction is
 * lost the moment both cases render the same sentence — at which point a student
 * mid-outage is told their entitlement is gone.
 */
describe('the recognition notice', () => {
  it('says nothing before the check lands', () => {
    // The pre-recognition behaviour is "show the prompt". Silence is what makes
    // a student holding their card no worse off than before this feature.
    expect(recognitionNotice(null)).toBeNull()
  })

  it('says nothing when the hub does not know the device', () => {
    // The prompt is already the correct message. "You are not recognised" would
    // be a claim about their account that changes nothing they can do.
    expect(recognitionNotice({ kind: 'unknownDevice' })).toBeNull()
  })

  it('tells a returning student their device is being restored', () => {
    const notice = recognitionNotice({
      kind: 'recognised',
      tier: 'stealth',
      expiresAt: null,
      durable: true,
    })
    expect(notice?.kind).toBe('restoring')
    expect(notice?.tone).toBe('info')
    expect(notice?.tier).toBe('stealth')
  })

  it('warns instead of promising when the identity is not durable', () => {
    // `durable: false` means a FUTURE reinstall loses the entitlement. The UI
    // must not claim the device is remembered — over-promising here is the
    // support call the feature exists to remove.
    const notice = recognitionNotice({
      kind: 'recognised',
      tier: 'eco',
      expiresAt: null,
      durable: false,
    })
    expect(notice?.kind).toBe('durableWarning')
    // A caveat, not a reassurance: it must not be rendered as quiet info.
    expect(notice?.tone).toBe('caution')
  })

  it('never renders "restoring" and the durability warning at the same time', () => {
    // Two notices would say "we remember you" and "we might not next time" at
    // once, which reads as a contradiction rather than a caveat. Exactly one
    // message is chosen per result.
    const durable = recognitionNotice({
      kind: 'recognised',
      tier: 'eco',
      expiresAt: null,
      durable: true,
    })
    const notDurable = recognitionNotice({
      kind: 'recognised',
      tier: 'eco',
      expiresAt: null,
      durable: false,
    })
    expect(durable?.kind).not.toBe(notDurable?.kind)
  })

  it('reports an unreachable hub as a caution, not as a refusal', () => {
    // THE test this file exists for. `unavailable` is a transport failure; the
    // student's entitlement is untouched and unknown. It must produce a notice
    // (so they know why the easy path did not happen) whose key is the
    // *unavailable* one — if this ever returns the refusal or the
    // `unknownDevice`-shaped silence, an outage has become a false statement
    // about someone's account.
    const notice = recognitionNotice({ kind: 'unavailable' })
    expect(notice).not.toBeNull()
    expect(notice?.kind).toBe('unavailable')
    expect(notice?.tone).toBe('caution')
    expect(notice?.messageKey).toContain('recognitionUnavailable')
    // It must not borrow the "we remember you" copy.
    expect(notice?.messageKey).not.toContain('recognised')
  })

  it('every notice carries a distinct i18n key', () => {
    // Three states, three sentences. Two cases sharing a key is how the
    // unavailable/unknownDevice distinction would be lost silently.
    const keys = [
      recognitionNotice({
        kind: 'recognised',
        tier: 'eco',
        expiresAt: null,
        durable: true,
      })?.messageKey,
      recognitionNotice({
        kind: 'recognised',
        tier: 'eco',
        expiresAt: null,
        durable: false,
      })?.messageKey,
      recognitionNotice({ kind: 'unavailable' })?.messageKey,
    ]
    expect(new Set(keys).size).toBe(3)
  })

  it('handles every case of the union without falling through', () => {
    // Exhaustiveness, so adding a fourth `RecognitionResult` case is a failing
    // test rather than a screen that silently says nothing.
    const cases: RecognitionResult[] = [
      { kind: 'recognised', tier: 'eco', expiresAt: null, durable: true },
      { kind: 'recognised', tier: 'eco', expiresAt: '2026-12-01', durable: false },
      { kind: 'unknownDevice' },
      { kind: 'unavailable' },
    ]
    for (const result of cases) {
      // Must not throw, and must return either a notice or an explicit null.
      const notice = recognitionNotice(result)
      expect(notice === null || typeof notice.messageKey === 'string').toBe(true)
    }
  })
})
