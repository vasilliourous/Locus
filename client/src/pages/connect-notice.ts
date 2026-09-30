import type { SubscriptionStatus } from '@/services/locus'

/**
 * What the connection screen should say when the student cannot connect.
 *
 * Extracted as a pure function for the same reason `phaseFromStatus` was: this
 * rule decides whether a student is told something true or something misleading,
 * and a rule that can only be exercised by mounting React against a Tauri mock is
 * a rule that regresses unnoticed.
 *
 * # The bug this exists for
 *
 * A device whose code the hub has refused is *unactivated* — a refusal withdraws
 * the entitlement — and the screen rendered "This device needs an activation code
 * before it can connect" at it. The student had a code. It had been suspended, and
 * the app responded by asking them for it again, with no mention that anything had
 * happened. The only way to discover the truth was to close and reopen the app, and
 * even then the reason was not shown.
 *
 * The fix is priority, not new copy: when we know why access ended, that is the
 * whole message, and the generic prompt stands down.
 *
 * # The second bug this file carries (2026-09-30)
 *
 * The card prompt then appeared when it should not have, for a different reason.
 * The rule keyed on `phase === 'unknown'`, and `unknown` meant two different
 * things: "we read the status and this device is not activated" AND "we have not
 * read the status yet". The connection hook starts in the second state and moves
 * to a settled one when its read lands, so on EVERY mount the screen rendered the
 * card prompt for one render — "milliseconds, sometimes over a second", which is
 * exactly how long `locus_status` takes while it proves the tunnel with a real
 * request through it. Navigating Home -> Settings remounts the page, so it
 * happened on every navigation.
 *
 * The fix is the same shape as the first one: stop conflating two facts. The
 * phase is now `checking` until a read settles and `unactivated` only once the
 * backend has said so, and this rule keys on the latter. A student is told to find
 * their card only when the backend has actually established they need to.
 */
export type ConnectNotice =
  /** The hub refused this device; show its own sentence, verbatim. */
  | { kind: 'refused'; reason: string }
  /** Nothing has been set up yet. The generic "find your card" prompt. */
  | { kind: 'unactivated' }
  /** No notice. A working device, or a state another element already explains. */
  | { kind: 'none' }

/**
 * Decides what to show beneath the connect control.
 *
 * `phase` is the connection phase and `hasError` is whether the backend already
 * produced a sentence of its own; an error always wins, because it describes
 * something that just happened rather than a standing account state.
 *
 * The refusal is checked FIRST, and deliberately not gated on `phase`. A refused
 * device normally reads as `unknown` (unactivated), but it can also read as
 * `connected` for the moment before the refusal tears the core down — and in that
 * window the student must still be told, not shown a healthy-looking button.
 */
export const connectNotice = (
  phase: string,
  hasError: boolean,
  subscription: SubscriptionStatus,
): ConnectNotice => {
  if (subscription.state === 'refused') {
    return { kind: 'refused', reason: subscription.reason }
  }

  // An error is the most specific thing we have: it is about this attempt.
  if (hasError) return { kind: 'none' }

  // `unactivated` is "we read the status, and this device holds no entitlement".
  // It is deliberately NOT `unknown`/`checking`, which means "no read has settled
  // yet" — see the note below.
  //
  // Any other phase has its own affordances and needs no notice.
  if (phase === 'unactivated') return { kind: 'unactivated' }

  return { kind: 'none' }
}

/** The activation screen's message, when the hub has refused this device. */
export const activationRefusal = (
  subscription: SubscriptionStatus,
): string | null =>
  subscription.state === 'refused' ? subscription.reason : null
