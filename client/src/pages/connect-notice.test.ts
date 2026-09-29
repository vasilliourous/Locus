import { describe, expect, test } from 'vitest'

import type { SubscriptionStatus } from '@/services/locus'

import { activationRefusal, connectNotice } from './connect-notice'

const refused: SubscriptionStatus = {
  state: 'refused',
  reason: 'Account suspended — contact your middleman',
}
const lapsed: SubscriptionStatus = { state: 'lapsed' }
const unknown: SubscriptionStatus = { state: 'unknown' }
const active: SubscriptionStatus = {
  state: 'active',
  daysRemaining: 30,
  urgent: false,
  expiresAt: '2030-01-01 00:00:00.000Z',
}
const activeUrgent: SubscriptionStatus = {
  state: 'active',
  daysRemaining: 2,
  urgent: true,
  expiresAt: '2030-01-01 00:00:00.000Z',
}

describe('what the connection screen says a student cannot connect', () => {
  /**
   * The reported bug: a suspended code produced "This device needs an activation
   * code before it can connect" — an instruction to re-enter a code the student
   * already had and which had been refused. The reason must win.
   */
  test('a refusal is shown instead of the generic card prompt', () => {
    const notice = connectNotice('unknown', false, refused)
    expect(notice.kind).toBe('refused')
    expect(notice).toMatchObject({
      reason: 'Account suspended — contact your middleman',
    })
  })

  /**
   * The refusal must be reported verbatim. It carries the operator's instruction
   * ("contact your middleman"); paraphrasing it would replace an action with a
   * description of one.
   */
  test('the refusal carries the hubs own wording unchanged', () => {
    const notice = connectNotice('unknown', false, refused)
    if (notice.kind !== 'refused')
      throw new Error(`expected refused, got ${notice.kind}`)
    expect(notice.reason).toBe(
      refused.state === 'refused' ? refused.reason : '',
    )
  })

  /**
   * Not gated on phase. A refused device is normally `unknown`, but there is a
   * window where the core is still being torn down and the phase reads
   * `connected`. The student must be told in that window too, not shown a
   * healthy-looking button.
   */
  test('a refusal is reported whatever the connection phase says', () => {
    for (const phase of [
      'unknown',
      'connected',
      'connecting',
      'disconnected',
      'disconnecting',
    ]) {
      expect(connectNotice(phase, false, refused).kind, `phase ${phase}`).toBe(
        'refused',
      )
    }
  })

  /**
   * An error describes this attempt, so it outranks a standing account notice —
   * except a refusal, which is checked first because it explains the error the
   * student is most likely looking at.
   */
  test('a live error suppresses the generic prompt but not a refusal', () => {
    expect(connectNotice('unknown', true, unknown).kind).toBe('none')
    expect(connectNotice('unknown', true, refused).kind).toBe('refused')
  })

  test('a fresh device with no refusal gets the card prompt', () => {
    expect(connectNotice('unknown', false, unknown).kind).toBe('unactivated')
  })

  /**
   * An unactivated device that has never been refused must NOT be told it was
   * refused. This is the reverse error: frightening a new student with a
   * suspension that never happened.
   */
  test('an ordinary unactivated device is never shown a refusal', () => {
    expect(connectNotice('unknown', false, unknown).kind).not.toBe('refused')
  })

  /**
   * A lapsed subscription is a different case with different copy, owned by the
   * lapsed branch. It must not be reported as a refusal, or a student who can
   * simply renew would be told to contact support.
   */
  test('a lapsed subscription is not a refusal', () => {
    expect(connectNotice('connected', false, lapsed).kind).toBe('none')
    expect(connectNotice('connected', false, active).kind).toBe('none')
    expect(connectNotice('connected', false, activeUrgent).kind).toBe('none')
  })
})

describe('the activation screen refusal banner', () => {
  test('shows the hubs sentence for a refused device', () => {
    expect(activationRefusal(refused)).toBe(
      'Account suspended — contact your middleman',
    )
  })

  test('shows nothing when there is no refusal to report', () => {
    for (const status of [unknown, lapsed, active, activeUrgent]) {
      expect(activationRefusal(status), status.state).toBeNull()
    }
  })
})
