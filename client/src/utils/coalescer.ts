/**
 * Trailing-edge coalescing for a stream of live readings.
 *
 * # Why this is its own module
 *
 * It lived inline in `useMihomoWsSubscription`, and it was wrong in a way no test
 * could see: it emitted the first message of each window immediately and re-armed
 * its timer from that message, so a socket pushing faster than the window reopened
 * the window on every frame and the throttle bounded nothing. The value that
 * survived a window was whichever message happened to arrive when the timer was
 * null — not the last one — so the figure on screen could be an arbitrary sample of
 * the burst rather than the newest reading.
 *
 * Extracted so the scheduling rule is a pure function with an injectable clock,
 * which is the only way behaviour like this gets pinned: the bug was invisible to
 * any test that used real time for both the message rate and the window.
 *
 * # The rule: trailing edge only
 *
 * A push that arrives while no window is open *opens* one and is held. When the
 * window closes, the newest value received during it is emitted. Nothing is
 * emitted at the moment of the push itself.
 *
 * # Why not leading-edge (emit the first value immediately)
 *
 * It is tempting — a UI would not wait a full window for its first reading — but
 * combining a leading edge with a trailing one yields **two emissions per window**
 * for any producer whose rate is not a divisor of the window, and the phase is
 * data-dependent. Simulating a 50 Hz producer through a 200 ms window produced
 * emissions at t=0 and t=180, then t=200 and t=380, and so on: a steady 2× the
 * nominal rate, which is exactly the "throttle that does not throttle" failure
 * this module exists to remove. A strict trailing edge is one emission per window
 * by construction, with no arithmetic a future reader has to re-derive.
 *
 * The cost is one window of latency on the very first reading. That is acceptable
 * here because the first reading of a traffic stream is `0 B/s` in practice — the
 * socket connects and then data arrives — so there is nothing meaningful to show
 * sooner than the first real measurement arrives.
 */
export interface Coalescer<T> {
  /** Offer a reading. `sink` is called at most once per window. */
  push: (value: T) => void
  /** Stop any pending emission. Does not emit the held value. */
  cancel: () => void
}

export interface CoalescerClock {
  setTimeout: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>
  clearTimeout: (handle: ReturnType<typeof setTimeout>) => void
}

const defaultClock: CoalescerClock = {
  setTimeout: (fn, ms) => setTimeout(fn, ms),
  clearTimeout: (handle) => clearTimeout(handle),
}

export const createCoalescer = <T>(
  windowMs: number,
  sink: (value: T) => void,
  clock: CoalescerClock = defaultClock,
): Coalescer<T> => {
  let held: T | undefined
  let hasHeld = false
  let timer: ReturnType<typeof setTimeout> | null = null

  const flush = () => {
    timer = null
    if (!hasHeld) return
    const value = held as T
    hasHeld = false
    held = undefined
    sink(value)
  }

  return {
    push: (value: T) => {
      // A non-positive window means "do not coalesce" — pass straight through, so
      // a caller that does not want this behaviour is not silently given a timer.
      if (windowMs <= 0) {
        sink(value)
        return
      }

      // Always overwrite: inside a window the consumer wants the newest reading,
      // not a replay of the frames it missed, and outside one this is the value
      // that opens the next window.
      held = value
      hasHeld = true

      // A window already open will emit what is held. No second timer, which is
      // what makes the rate exactly one per window.
      if (timer === null) {
        timer = clock.setTimeout(flush, windowMs)
      }
    },
    cancel: () => {
      if (timer !== null) {
        clock.clearTimeout(timer)
        timer = null
      }
      hasHeld = false
      held = undefined
    },
  }
}
