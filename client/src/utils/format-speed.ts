import parseTraffic from './parse-traffic'

/**
 * A speed, formatted to be read by a person rather than a network engineer.
 *
 * `parseTraffic` answers "how many of which unit" — it is the right tool for
 * totals and for the graph, and it is shared and tested, so it is not changed
 * here. This wraps it for the one place a *human* reads a live speed, because
 * the raw answer reads badly to the audience this product has:
 *
 *  - **Nothing looks broken.** A student idling on a page legitimately sees a few
 *    hundred bytes a second, and `parseTraffic` renders that as `847 B/s`. To
 *    someone who has never heard of a byte, "B" is a mystery and "under 1" is
 *    indistinguishable from "not working". Sub-kilobyte speeds are therefore
 *    shown in KB/s with one decimal (`0.8 KB/s`), which reads as "a small,
 *    real number" instead of a suspicious one.
 *  - **Zero reads as idle, not as a fault.** A speed of exactly zero is shown as
 *    the idle word, not `0 B/s`. "0 B/s" is what a broken tunnel shows *and* what
 *    a quiet connection shows; the graph and the totals are the honest evidence
 *    of which it is, and a bare zero here needlessly alarms.
 *
 * Unit preference is preserved: KB and above keep `parseTraffic`'s own scaling,
 * so the two are only inconsistent in the sub-KB band, where the friendlier form
 * is the point.
 */
const IDLE_THRESHOLD_BYTES_PER_SEC = 1

export interface FormattedSpeed {
  /** The number, as text. */
  value: string
  /** The unit, **without** a `/s` suffix — the caller adds that. */
  unit: string
  /** True when the speed is effectively zero, so the UI can say "idle". */
  idle: boolean
}

export const formatSpeed = (
  bytesPerSecond: number | undefined,
): FormattedSpeed => {
  const bytes = bytesPerSecond ?? 0

  if (!Number.isFinite(bytes) || bytes < IDLE_THRESHOLD_BYTES_PER_SEC) {
    return { value: '0', unit: 'B', idle: true }
  }

  // Sub-kilobyte: promote to KB/s so a small live speed shows a normal-looking
  // number. `Math.max` guards the rounding-to-zero case (e.g. 3 B/s -> 0.0 KB/s
  // would read as idle), which is exactly the impression this avoids.
  if (bytes < 1024) {
    const kb = Math.max(0.1, bytes / 1024)
    return { value: kb.toFixed(1), unit: 'KB', idle: false }
  }

  const [value, unit] = parseTraffic(bytes)
  return { value: String(value), unit, idle: false }
}
