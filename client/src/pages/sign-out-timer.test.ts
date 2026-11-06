import { describe, expect, test } from 'vitest'

import {
  canStillCancel,
  graceState,
  SIGN_OUT_GRACE_MS,
  SIGN_OUT_RESULT_MS,
} from './sign-out-timer'

describe('the sign-out confirmation window', () => {
  /**
   * The promise the dialog makes: three whole visible seconds in which the
   * student can change their mind. The sequence is 3, 2, 1 — never 2, 1, 0,
   * because a displayed `0` for a whole second reads as "nothing is happening"
   * at the exact moment something is about to.
   */
  test('counts 3, 2, 1 and then acts', () => {
    const t0 = 1_000_000
    expect(graceState(t0, t0)).toEqual({ kind: 'counting', remaining: 3 })
    expect(graceState(t0, t0 + 1)).toEqual({ kind: 'counting', remaining: 3 })
    expect(graceState(t0, t0 + 1000)).toEqual({ kind: 'counting', remaining: 2 })
    expect(graceState(t0, t0 + 2000)).toEqual({ kind: 'counting', remaining: 1 })
    expect(graceState(t0, t0 + 2999)).toEqual({ kind: 'counting', remaining: 1 })
    expect(graceState(t0, t0 + SIGN_OUT_GRACE_MS)).toEqual({ kind: 'elapsed' })
  })

  /**
   * Nothing is destroyed until the window closes, and this is the assertion that
   * pins it: there is no moment before `elapsed` at which the caller is told to
   * act. If someone "simplifies" the rule to act on confirm, this fails.
   */
  test('never reports elapsed before the grace period is over', () => {
    const t0 = 1_000_000
    for (let ms = 0; ms < SIGN_OUT_GRACE_MS; ms += 50) {
      expect(graceState(t0, t0 + ms).kind, `at +${ms}ms`).toBe('counting')
    }
  })

  /**
   * "Not confirmed yet" is not "confirmed zero milliseconds ago". Rendering the
   * second when it is the first is how a dialog shows a countdown before the
   * student has pressed anything.
   */
  test('has not started, and cannot be elapsed, before the student confirms', () => {
    expect(graceState(null, Date.now())).toEqual({ kind: 'idle' })
    expect(canStillCancel(graceState(null, Date.now()))).toBe(true)
  })

  /**
   * Cancel stays available for the WHOLE window. A control that disappears at
   * second 1 while the countdown still runs is a trap: the dialog says it can be
   * stopped and then stops offering the way to stop it.
   */
  test('cancel is available for every moment of the window, and only then', () => {
    const t0 = 1_000_000
    expect(canStillCancel(graceState(t0, t0))).toBe(true)
    expect(canStillCancel(graceState(t0, t0 + 1500))).toBe(true)
    expect(canStillCancel(graceState(t0, t0 + SIGN_OUT_GRACE_MS - 1))).toBe(true)
    expect(canStillCancel(graceState(t0, t0 + SIGN_OUT_GRACE_MS))).toBe(false)
  })

  /**
   * A clock that jumps backwards (an NTP correction, a laptop waking from sleep)
   * must not restart the window or produce a negative countdown. It reports "just
   * started", which under-promises: the student waits a moment longer rather than
   * the sign-out happening before they expect it.
   */
  test('a backwards clock does not restart or invert the countdown', () => {
    const t0 = 1_000_000
    expect(graceState(t0, t0 - 5000)).toEqual({
      kind: 'counting',
      remaining: 3,
    })
  })

  /**
   * The second promise: the result is readable before the screen changes. Three
   * seconds is short enough not to feel stuck and long enough to read one
   * sentence — and it is asserted so a future "make it snappier" does not
   * silently turn it into a flash.
   */
  test('the result is held long enough to read', () => {
    expect(SIGN_OUT_RESULT_MS).toBeGreaterThanOrEqual(3000)
    expect(SIGN_OUT_RESULT_MS).toBeLessThanOrEqual(5000)
  })
})
