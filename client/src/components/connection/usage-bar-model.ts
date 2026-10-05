import type { AllowanceStatus } from '@/services/locus'

/**
 * Formatting for the free tier's usage bar, as pure functions.
 *
 * Split out of the component for the same reason `connect-notice.ts` is split
 * out of `connection.tsx`: the rules a student actually reads — when the warning
 * shows, what the throttle banner says — are decisions worth testing directly,
 * without a renderer. Everything here is a pure function of the backend's
 * already-classified `AllowanceStatus`; the frontend does no arithmetic of its
 * own, so the warning line and the throttle line cannot drift from the Rust.
 */

/** Bytes in a mebibyte, for the human strings beside the bar. */
const MIB = 1024 * 1024

/**
 * A compact, human byte string: "1.2 GB", "480 MB".
 *
 * Binary units with decimal-looking labels, which is what students expect from
 * a data allowance and what the hub's mebibyte figure actually means. One
 * decimal place below 10 so "1.4 GB" does not read as "1 GB", and none above,
 * so "480 MB" does not read as a false precision.
 */
export const formatBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB'
  const gb = bytes / (MIB * 1024)
  if (gb >= 1) {
    return gb >= 10 ? `${Math.round(gb)} GB` : `${gb.toFixed(1)} GB`
  }
  const mb = bytes / MIB
  return mb >= 10 ? `${Math.round(mb)} MB` : `${mb.toFixed(1)} MB`
}

/** What the usage bar should render, or why it should render nothing. */
export type UsageDisplay =
  | { kind: 'none' }
  | {
      kind: 'bar'
      percent: number
      usedLabel: string
      allowanceLabel: string
      /** Past the 80% line but not spent. */
      warning: boolean
      /** Spent; the throttle banner shows for the rest of the window. */
      throttled: boolean
    }

/**
 * Decides what the usage bar shows.
 *
 * Returns `none` for `unlimited`, which is every paying tier and every hub that
 * predates the free tier. That is the whole reason `AllowanceStatus` has an
 * `unlimited` variant rather than a zero allowance: a paying student must never
 * see a free-tier bar, and a bar reading "0 of 0 MB" is exactly that bug.
 */
export const usageDisplay = (allowance: AllowanceStatus): UsageDisplay => {
  if (allowance.state !== 'metered') return { kind: 'none' }
  return {
    kind: 'bar',
    percent: Math.max(0, Math.min(100, allowance.percent)),
    usedLabel: formatBytes(allowance.usedBytes),
    allowanceLabel: formatBytes(allowance.allowanceBytes),
    warning: allowance.warning,
    // `throttled` is the backend's verdict; never re-derived here from the
    // percentage, or a change to the threshold would need two edits.
    throttled: allowance.throttled,
  }
}

/**
 * The translated strings the summary needs, injected rather than imported.
 *
 * The model stays pure and locale-free so its tests do not need an i18n
 * provider: the caller passes the already-translated templates in, and this
 * module decides *which* one applies. That keeps the decision — the part that
 * can be wrong in a way a student notices — testable without a renderer.
 */
export interface UsageCopy {
  /** `{{used}} of {{total}} used` */
  used: string
  /** `{{used}} of {{total}} used — nearly out of free data` */
  nearlyOut: string
  /** `{{used}} of {{total}} used — slowed until your allowance resets` */
  throttled: string
}

/** Substitute the two placeholders in a copy template. */
const fill = (template: string, used: string, total: string): string =>
  template.replace('{{used}}', used).replace('{{total}}', total)

/**
 * The one-line summary beside the bar.
 *
 * Deliberately separate from {@link usageDisplay} so the component's text and
 * the bar's colour come from the same classification — the failure mode this
 * avoids is a green bar under a message saying "throttled".
 *
 * `copy` is required rather than defaulted: a missing translation must be a
 * compile error or a visible placeholder, never a silently English string in a
 * student's own language.
 */
export const usageSummary = (display: UsageDisplay, copy: UsageCopy): string => {
  if (display.kind === 'none') return ''
  const used = display.usedLabel
  const total = display.allowanceLabel
  if (display.throttled) return fill(copy.throttled, used, total)
  if (display.warning) return fill(copy.nearlyOut, used, total)
  return fill(copy.used, used, total)
}

/**
 * Whether the one-time 80% warning should be shown as a distinct notice.
 *
 * The bar itself always shows the warning colour; this governs the *notice*,
 * which is the thing that must fire once and not repeat on every render. The
 * caller pairs it with the persisted `warned` flag in the usage window.
 */
export const shouldAnnounceWarning = (allowance: AllowanceStatus): boolean =>
  allowance.state === 'metered' && allowance.warning
