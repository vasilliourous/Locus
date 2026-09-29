import { describe, expect, test } from 'vitest'

import { formatSpeed } from './format-speed'

describe('human-readable speed formatting', () => {
  /**
   * The complaint this exists for: a small live speed rendered as "847 B/s"
   * reads as broken to someone who does not know what a byte is. It must become
   * a normal-looking KB/s number instead.
   */
  test('sub-kilobyte speeds are promoted to KB/s, not left as bare bytes', () => {
    const { value, unit, idle } = formatSpeed(847)
    expect(unit).toBe('KB')
    expect(value).toBe('0.8')
    expect(idle).toBe(false)
  })

  /** A rounding-to-zero must not reappear as an apparent zero. */
  test('a tiny nonzero speed never rounds to zero', () => {
    const { value, idle } = formatSpeed(3)
    expect(idle).toBe(false)
    expect(Number(value)).toBeGreaterThan(0)
  })

  /** Zero is idle, and is flagged so the UI can say so rather than "0 B/s". */
  test('zero is reported as idle', () => {
    expect(formatSpeed(0).idle).toBe(true)
    expect(formatSpeed(undefined).idle).toBe(true)
  })

  /** At and above a kilobyte, the shared parser's scaling is preserved. */
  test('kilobyte and larger speeds keep the shared units', () => {
    expect(formatSpeed(1024).unit).toBe('KB')
    expect(formatSpeed(1024 * 1024).unit).toBe('MB')
    expect(formatSpeed(1024 * 1024).idle).toBe(false)
  })

  /** A non-finite reading is treated as idle, never rendered as "NaN". */
  test('non-finite input is treated as idle rather than rendered', () => {
    expect(formatSpeed(Number.NaN).idle).toBe(true)
    expect(formatSpeed(Number.POSITIVE_INFINITY).value).toBe('0')
  })
})
