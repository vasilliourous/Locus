/**
 * Which key events may toggle the tunnel.
 *
 * # Why this is a separate, pure function
 *
 * The rule exists to prevent one specific failure — a student typing an
 * activation code presses Space between words, and the app tries to bring a VPN
 * up behind the screen they are typing on. That is not a styling preference, it
 * is a rule about when an action is meaningful, and it is exactly the kind of
 * rule that regresses silently when it lives inline in a `keydown` handler that
 * no test can reach.
 *
 * The same reasoning as `connect-notice.ts` and `phase.ts`: the decision is a
 * function of its inputs, so it is a function, and it is tested directly.
 *
 * # The three ways a key can be wrong
 *
 * 1. **It is not Enter or Space.** Nothing else toggles.
 * 2. **Something with focus owns it.** A text field, a select, another button —
 *    anything where the key has its own meaning. `isContentEditable` is included
 *    even though the app has no rich-text surface today, because a future one
 *    would otherwise silently start a VPN behind the cursor.
 * 3. **It is a repeat.** Holding Enter must not fire a connect per auto-repeat
 *    frame; `locus_connect` is expensive and the cancel path would be racing
 *    itself.
 *
 * A fourth is checked by the caller rather than here, because it is about the
 * event's history rather than its content: `defaultPrevented`. A button that has
 * already handled the press sets it, and acting again would toggle twice.
 */
export interface ToggleKeyEvent {
  key: string
  repeat: boolean
  /** Tag name of the focused element, uppercased, or `''` when there is none. */
  targetTagName: string
  targetIsContentEditable: boolean
}

/** Elements where Enter and Space already mean something else. */
const ELEMENTS_THAT_OWN_THE_KEY = new Set([
  'INPUT',
  'TEXTAREA',
  'SELECT',
  'BUTTON',
  'A',
  // Already a control, and toggling a VPN from a checkbox would be a surprise.
  'OPTION',
])

export const shouldToggleFromKey = (event: ToggleKeyEvent): boolean => {
  if (event.key !== 'Enter' && event.key !== ' ') return false
  if (event.repeat) return false
  if (event.targetIsContentEditable) return false
  if (ELEMENTS_THAT_OWN_THE_KEY.has(event.targetTagName)) return false
  return true
}
