/** Normalizes latency values and the core's non-measurement sentinels. */

export const DEFAULT_DELAY_TIMEOUT = 10000

const TESTING = -2

const IMPLAUSIBLE_DELAY = 1e5

export type DelayState =
  | 'testing'
  | 'untested'
  | 'error'
  | 'timeout'
  | 'measured'

/**
 * Classifies a delay value for **display**.
 *
 * Answers "how fast is this node?", which is a different question from the one
 * the Rust readiness probe asks. `timeout` is the correct answer here: a node
 * that hit its budget should be *shown* as a timeout in a latency list.
 *
 * The backend answers "did the tunnel carry a packet at all?"
 * (`core::manager::probe::delay_is_a_measurement`) and deliberately reaches the
 * opposite verdict for the same value — a member that hit its budget still proves
 * the tunnel is carrying traffic. Keeping the two *identical* was a bug, not a
 * consistency win: it made the probe read "this was slow" as "this is dead" and
 * pinned a working connection on "connecting". What the two must agree on is the
 * set of *sentinels* (`0`, `> 1e5`); what they may differ on is what a slow node
 * means for their own callers.
 */
export const classifyDelay = (
  delay: number,
  timeout: number = DEFAULT_DELAY_TIMEOUT,
): DelayState => {
  if (!Number.isFinite(delay)) return 'untested'
  if (delay === TESTING) return 'testing'
  if (delay < 0) return 'untested'
  if (delay > IMPLAUSIBLE_DELAY) return 'error'
  if (delay === 0 || delay >= timeout) return 'timeout'
  return 'measured'
}

/** Rank separately so sentinel magnitudes cannot outrank real measurements. */
const rankOf = (state: DelayState): number => {
  switch (state) {
    case 'measured':
      return 0
    case 'timeout':
      return 1
    case 'error':
      return 2
    case 'testing':
      return 3
    case 'untested':
      return 4
  }
}

export const compareByDelay = (
  a: number,
  b: number,
  timeout: number = DEFAULT_DELAY_TIMEOUT,
): number => {
  const [aState, bState] = [
    classifyDelay(a, timeout),
    classifyDelay(b, timeout),
  ]
  const rankDifference = rankOf(aState) - rankOf(bState)
  if (rankDifference !== 0) return rankDifference

  if (aState !== 'measured') return 0
  return a - b
}
