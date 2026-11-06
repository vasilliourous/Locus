/**
 * The two three-second promises made by the sign-out control.
 *
 * # Why this is a module rather than two `setTimeout`s in the component
 *
 * The student was promised two specific things about *time*, and both are easy to
 * break in a way no type or lint catches:
 *
 *   1. After confirming, there is a window in which the sign-out can still be
 *      **cancelled**. Nothing is destroyed while they can change their mind.
 *   2. After signing out, the result stays on screen long enough to READ before
 *      the app returns to the activation screen.
 *
 * A component that starts a timer in an effect re-creates it on every render, and
 * one whose countdown is derived from `Date.now()` inside a render is a
 * measurement that cannot move (the trap in `client/AGENTS.md`: a diagnostic that
 * cannot change is worse than none). Both failure modes are invisible in a
 * screenshot and only show up as "the number jumped" or "it flashed past".
 *
 * So the rule is here, as a pure function over elapsed time with the clock
 * injected, and the component only renders what it says.
 */

/** How long the "are you sure" window lasts, in milliseconds. */
export const SIGN_OUT_GRACE_MS = 3000

/** How long the result stays on screen before the app returns to activation. */
export const SIGN_OUT_RESULT_MS = 3000

/** What the confirmation control should show at a given moment. */
export type GraceState =
  /** Waiting for the student to press Sign out. */
  | { kind: 'idle' }
  /** Counting down; `remaining` is whole seconds, always >= 1. */
  | { kind: 'counting'; remaining: number }
  /** The window closed and the sign-out is due. The caller must act. */
  | { kind: 'elapsed' }

/**
 * Where the confirmation window is, given how long ago it started.
 *
 * `startedAt` is the moment the student confirmed, in the same units as `now` —
 * or `null` when they have not confirmed yet, which is a different state from
 * "confirmed 0 ms ago" and must not be rendered as "0…". That distinction is the
 * same one `connect-notice.ts` documents for `unknown` vs a settled read: a name
 * that means "absent" and "not yet" at once is how a screen tells a student to
 * do something they already did.
 *
 * `remaining` is `Math.ceil`d so the sequence a student sees is 3, 2, 1 and then
 * the sign-out — never 2, 1, 0, where the final `0` reads as "nothing is
 * happening" for a whole second. The value is clamped at 1 for the same reason: a
 * displayed `0` means the window has closed, and that state is `elapsed`.
 */
export const graceState = (
  startedAt: number | null,
  now: number,
): GraceState => {
  if (startedAt === null) return { kind: 'idle' }

  // A clock that went backwards (NTP correction, a suspended laptop) must not
  // produce a negative countdown or restart the window: treat it as "just
  // started", which under-promises rather than acting early.
  const elapsed = Math.max(0, now - startedAt)
  if (elapsed >= SIGN_OUT_GRACE_MS) return { kind: 'elapsed' }

  return {
    kind: 'counting',
    remaining: Math.max(1, Math.ceil((SIGN_OUT_GRACE_MS - elapsed) / 1000)),
  }
}

/** Whether pressing "Cancel" still prevents the sign-out entirely. */
export const canStillCancel = (state: GraceState): boolean =>
  state.kind === 'counting' || state.kind === 'idle'
