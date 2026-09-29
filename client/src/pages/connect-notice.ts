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

  // `unknown` is "not activated" — a fresh device, which is what the card prompt
  // is for. Any other phase has its own affordances and needs no notice.
  if (phase === 'unknown') return { kind: 'unactivated' }

  return { kind: 'none' }
}

/** The activation screen's message, when the hub has refused this device. */
export const activationRefusal = (
  subscription: SubscriptionStatus,
): string | null =>
  subscription.state === 'refused' ? subscription.reason : null
