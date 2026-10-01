import type { RecognitionResult } from '@/services/locus'

/**
 * What the activation screen says about the device-recognition check.
 *
 * Extracted as a pure function for the same reason `connect-notice.ts` and
 * `phase.ts` were: this decides whether a returning student is told something
 * true about their own account, and a rule that can only be exercised by mounting
 * React against a Tauri mock is a rule that regresses unnoticed.
 *
 * # What this exists for
 *
 * `locus_recognise()` asks the hub whether it already knows this device, so a
 * student who reinstalled is not asked for a card they may have thrown away. The
 * check runs on first launch, and its result decides what — if anything — the
 * activation screen says *before* the student types.
 *
 * # The three cases are three different sentences, and that is the point
 *
 * The Rust side (`cmd/locus.rs`) is explicit that `RecognitionResult` is not a
 * `bool`, and it is worth restating here because it is the whole reason this
 * function exists rather than a boolean prop:
 *
 *   * `recognised` — the hub knows this device and it holds a live entitlement.
 *     The screen says so, and the student can wait for the app to restore it.
 *   * `unknownDevice` — the hub answered, and does not know this device. This is
 *     a statement about the account, so the prompt stands with no comment.
 *   * `unavailable` — we could not ask. **Not a refusal.** Telling a student
 *     "your device is not recognised" during an outage is a false claim about
 *     their entitlement, and it is the specific mistake the Rust type was shaped
 *     to prevent.
 *
 * # `durable` is a warning, not a detail
 *
 * `recognised.durable === false` means the identity lives only in the app config,
 * so a *future* reinstall will lose it and need the card again. The UI must say
 * so rather than claim the device is remembered: over-promising durability is the
 * support call this feature exists to remove, so a `durable: false` recognition
 * still shows the notice, with the caveat attached.
 *
 * # Returning a notice, not a verdict
 *
 * `null` means "say nothing", which is the correct answer for both
 * `unknownDevice` and a still-in-flight check. This keeps the screen's default
 * state identical to the behaviour before recognition existed: a student holding
 * their card is never worse off, and never told something we do not know.
 */

export type RecognitionNotice = {
  /** Stable id, used for the i18n key and for tests. */
  kind: 'restoring' | 'durableWarning' | 'unavailable'
  /** The i18n key to render. Kept as data so the caller does not re-derive it. */
  messageKey: string
  /**
   * Whether this notice changes what the student should do next.
   *
   * A `restoring` notice is informational — the app is about to move past this
   * screen on its own — so it is rendered quietly. `unavailable` tells the
   * student their card is *required*, so it is rendered as a caution.
   */
  tone: 'info' | 'caution'
  /** Tier name, when the hub named one. Rendered into the message. */
  tier?: string
}

const MESSAGE_KEYS = {
  restoring: 'home.components.connection.activation.recognisedRestoring',
  durableWarning: 'home.components.connection.activation.recognisedNotDurable',
  unavailable: 'home.components.connection.activation.recognitionUnavailable',
} as const

export const recognitionNotice = (
  result: RecognitionResult | null,
): RecognitionNotice | null => {
  // Not checked yet, or the check has not landed. Silence — the prompt below is
  // already the correct thing to show, and the pre-recognition behaviour is
  // exactly "show the prompt".
  if (!result) return null

  switch (result.kind) {
    case 'recognised':
      // The device IS known and the entitlement is being restored. If the
      // identity is not durable, that is the more useful sentence: the student
      // is about to be let in, and the thing they need to know is that the next
      // reinstall will not be this easy.
      //
      // `restoring` is not shown in that case — one message, not two. Two
      // notices would say "we remember you" and "we might not next time" at once,
      // which reads as a contradiction rather than a caveat.
      return result.durable
        ? {
            kind: 'restoring',
            messageKey: MESSAGE_KEYS.restoring,
            tone: 'info',
            tier: result.tier,
          }
        : {
            kind: 'durableWarning',
            messageKey: MESSAGE_KEYS.durableWarning,
            tone: 'caution',
            tier: result.tier,
          }

    case 'unknownDevice':
      // The hub answered and does not know this device. The prompt is the whole
      // message; adding "we don't recognise you" would be a sentence about the
      // student's account that changes nothing they can do.
      return null

    case 'unavailable':
      // We could not ask. Say so, and say what to do — which is to use the card.
      // This is deliberately NOT phrased as a refusal.
      return {
        kind: 'unavailable',
        messageKey: MESSAGE_KEYS.unavailable,
        tone: 'caution',
      }
  }
}
