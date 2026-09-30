import { describe, expect, test, vi } from 'vitest'

import { createCoalescer, type CoalescerClock } from './coalescer'

/**
 * A clock the test drives, so the message rate and the window are independent.
 *
 * This matters more than it looks: the bug this module was extracted from was
 * invisible under real time, because a test could not make messages arrive faster
 * than the window without also making the window elapse. With a manual clock,
 * "five messages inside one window" is expressible.
 */
const makeClock = () => {
  let now = 0
  let nextId = 1
  const pending = new Map<number, { at: number; fn: () => void }>()

  const clock: CoalescerClock = {
    setTimeout: (fn, ms) => {
      const id = nextId++
      pending.set(id, { at: now + ms, fn })
      return id as unknown as ReturnType<typeof setTimeout>
    },
    clearTimeout: (handle) => {
      pending.delete(handle as unknown as number)
    },
  }

  /** Advance time, firing everything due, in order. */
  const advance = (ms: number) => {
    const target = now + ms
    for (;;) {
      const due = [...pending.entries()]
        .filter(([, t]) => t.at <= target)
        .sort((a, b) => a[1].at - b[1].at)
      if (!due.length) break
      const [id, t] = due[0]
      pending.delete(id)
      now = t.at
      t.fn()
    }
    now = target
  }

  const pendingCount = () => pending.size

  return { clock, advance, pendingCount }
}

describe('trailing-edge coalescer', () => {
  /**
   * The reported bug, as a test.
   *
   * A burst inside one window must collapse to exactly ONE emission carrying the
   * newest value. Under the old implementation every message re-opened the
   * window, so a socket faster than the window produced an emission per message
   * and the throttle bounded nothing.
   */
  test('a burst inside one window collapses to the newest value', () => {
    const sink = vi.fn()
    const { clock, advance } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    coalescer.push(1)
    coalescer.push(2)
    coalescer.push(3)
    coalescer.push(4)
    coalescer.push(5)

    // Nothing yet: the window is still open. This is the whole point — the old
    // implementation emitted `1` here and again at the boundary.
    expect(sink).not.toHaveBeenCalled()

    advance(200)

    expect(sink).toHaveBeenCalledTimes(1)
    // The MOST RECENT value, not an arbitrary one from the middle of the burst.
    expect(sink).toHaveBeenCalledWith(5)
  })

  /**
   * The value that survives must be the newest, which is the property the old
   * implementation did not have: there the survivor was whichever message landed
   * when the timer happened to be null.
   */
  test('the surviving value is the latest, whatever the burst shape', () => {
    const sink = vi.fn()
    const { clock, advance } = makeClock()
    const coalescer = createCoalescer<string>(200, sink, clock)

    for (const value of ['a', 'b', 'c', 'd', 'e', 'f']) coalescer.push(value)
    advance(200)

    expect(sink).toHaveBeenCalledTimes(1)
    expect(sink).toHaveBeenCalledWith('f')
  })

  /**
   * The rate is bounded, and this is the assertion that caught the leading-edge
   * design.
   *
   * A leading edge PLUS a trailing one emits twice per window whenever the
   * producer's rate is not a divisor of the window — measured at 20 emissions for
   * 2 s of 50 Hz input through a 200 ms window, against an ideal of ~11. That is
   * a 2× overload hiding inside a function named "throttle", and it is
   * data-phase-dependent, so it would have shown up as an intermittent doubling
   * on some connections and not others.
   *
   * Strict trailing edge: one emission per window, no exceptions.
   */
  test('exactly one emission per window, whatever the input rate', () => {
    const sink = vi.fn()
    const { clock, advance } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    // A socket at 50 Hz for two seconds: 100 readings, 10 windows.
    for (let i = 0; i < 100; i++) {
      coalescer.push(i)
      advance(20)
    }
    advance(200)

    // 2000 ms / 200 ms = 10 windows, plus the final closing flush.
    expect(sink.mock.calls.length).toBeLessThanOrEqual(11)
    expect(sink.mock.calls.length).toBeGreaterThanOrEqual(10)
    // And the newest value must be the last thing emitted, not a stale one.
    expect(sink).toHaveBeenLastCalledWith(99)
  })

  /**
   * One emission per window means the count is a function of TIME, not of the
   * number of messages. Doubling the message rate must not change the output
   * count at all — which is the property a "throttle" is for.
   */
  test('doubling the message rate does not double the emissions', () => {
    const run = (stepMs: number) => {
      const sink = vi.fn()
      const { clock, advance } = makeClock()
      const coalescer = createCoalescer<number>(200, sink, clock)
      for (let elapsed = 0; elapsed < 2000; elapsed += stepMs) {
        coalescer.push(elapsed)
        advance(stepMs)
      }
      advance(200)
      return sink.mock.calls.length
    }

    const at50Hz = run(20)
    const at500Hz = run(2)

    // Both cover the same 2 seconds, so both must produce the same number of
    // emissions. The old implementation's count scaled with the input rate.
    expect(at500Hz).toBe(at50Hz)
  })

  /**
   * A quiet window neither emits nor leaves a timer behind: the next push opens a
   * fresh window rather than being swallowed by a stale one.
   */
  test('a quiet window emits nothing and leaves no timer', () => {
    const sink = vi.fn()
    const { clock, advance, pendingCount } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    coalescer.push(1)
    advance(200)
    expect(sink).toHaveBeenCalledTimes(1)
    expect(pendingCount()).toBe(0)

    advance(200)
    advance(200)
    expect(sink).toHaveBeenCalledTimes(1)
  })

  /**
   * A slow producer passes through at its own rate: each reading opens its own
   * window and is emitted when that window closes, with no batching of unrelated
   * readings and no accumulation of delay.
   */
  test('a producer slower than the window passes every reading through', () => {
    const sink = vi.fn()
    const { clock, advance } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    coalescer.push(1)
    advance(200)
    coalescer.push(2)
    advance(200)
    coalescer.push(3)
    advance(200)

    expect(sink).toHaveBeenCalledTimes(3)
    expect(sink.mock.calls.map((c) => c[0])).toEqual([1, 2, 3])
  })

  /**
   * A zero window means "do not coalesce". A caller that did not ask for
   * throttling must not silently get a timer.
   */
  test('a non-positive window passes everything through', () => {
    const sink = vi.fn()
    const { pendingCount } = makeClock()
    const coalescer = createCoalescer<number>(0, sink)

    coalescer.push(1)
    coalescer.push(2)
    coalescer.push(3)

    expect(sink).toHaveBeenCalledTimes(3)
    expect(pendingCount()).toBe(0)
  })

  /**
   * `cancel` must drop the held value, not emit it: it is called from an effect
   * cleanup, and emitting during teardown would write to a cache for a component
   * that is going away.
   */
  test('cancelling drops the held value without emitting it', () => {
    const sink = vi.fn()
    const { clock, advance, pendingCount } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    coalescer.push(1)
    coalescer.push(2)
    coalescer.cancel()
    advance(200)

    expect(sink).not.toHaveBeenCalled()
    expect(pendingCount()).toBe(0)
  })

  /**
   * After `cancel` the coalescer is usable again — a remount must not leave it
   * permanently inert.
   */
  test('a coalescer is reusable after cancel', () => {
    const sink = vi.fn()
    const { clock, advance } = makeClock()
    const coalescer = createCoalescer<number>(200, sink, clock)

    coalescer.push(1)
    coalescer.cancel()

    coalescer.push(2)
    advance(200)

    expect(sink).toHaveBeenCalledTimes(1)
    expect(sink).toHaveBeenCalledWith(2)
  })
})
