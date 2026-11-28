// Codes: shared types and helpers.
//
// Split out of the view so the row shape, the status vocabulary and the
// formatting rules have ONE definition. The previous page declared the row
// interface twice (Codes.vue and Devices.vue) with different fields, which is
// how the Devices page came to show a "bound" concept the hub had removed.

export interface CodeRow {
  id: string
  code: string
  tier: string
  status: 'available' | 'redeemed' | 'suspended' | 'expired'
  redeemed: boolean
  suspended: boolean
  expires_at: string
  activated_at: string
  middleman: string
  label: string
  notes: string
  /** How long one purchase lasts, in days. 0/absent = never expires. */
  term_days?: number | null
  term_kind?: string
  /** Whole days until expiry (rounded up), or null when there is no expiry. */
  days_remaining?: number | null
  /** When this code was FIRST redeemed. Survives an operator release. */
  first_seen_at?: string
  first_seen_fingerprint?: string
  /** When a student last signed out of this code (device_release.pb.js). */
  released_at?: string
  /** How many times a student has signed out of it. */
  release_count?: number
  /**
   * A sign-out is waiting to be claimed: `released_at` is later than
   * `activated_at`, so the code is free until the student's next activation.
   * Same comparison as `activation.pb.js`, which is the authority.
   */
  released_pending?: boolean
}

export interface HistoryEvent {
  event: string
  detail: string
  actor?: string
  created: string
}

export interface Facets {
  status: Record<string, number>
  tier: Record<string, number>
}

/**
 * What the operator is told a status MEANS.
 *
 * The badge used to be the raw string, so "available" appeared identically for a
 * freshly minted code and one that had been sold, released, and was now free
 * again — the state that double-sells a card. `first_seen_at` is what
 * distinguishes them, so the label does too.
 */
export function stateLabel(row: CodeRow): string {
  if (row.suspended) return 'stopped'
  if (row.status === 'expired') return 'lapsed'
  // A sign-out waiting to be claimed. It reads as its own state because it is
  // one: the next activation of this code will SUCCEED (a second device would be
  // refused), so calling it "in term" would tell the operator the key is tied up
  // when it is in fact free. See `released_pending` above.
  if (row.redeemed && row.released_pending) return 'signed out'
  if (row.redeemed) return 'in term'
  if (row.first_seen_at) return 'released'
  return 'unused'
}

export function stateClass(row: CodeRow): string {
  if (row.suspended) return 'suspended'
  if (row.status === 'expired') return 'expired'
  if (row.redeemed && row.released_pending) return 'issued'
  if (row.redeemed) return 'bound'
  if (row.first_seen_at) return 'issued'
  return 'available'
}

/** "12 Mar 2027" — short, unambiguous, and sortable in the eye. */
export function shortDate(iso?: string): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso.slice(0, 10)
  return d.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' })
}

export function shortDateTime(iso?: string): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso.slice(0, 16).replace('T', ' ')
  return d.toLocaleString(undefined, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/**
 * Days remaining as the operator reads it.
 *
 * "-3" is unhelpful in a table; "lapsed 3d ago" answers the question the column
 * exists for. Returns null when there is no expiry at all (a code that never
 * expires is not "0 days left").
 */
export function remainingText(row: CodeRow): string | null {
  const d = row.days_remaining
  if (d === null || d === undefined) return null
  if (d === 0) return 'lapsed'
  return `${d}d`
}

export function isUrgent(row: CodeRow): boolean {
  const d = row.days_remaining
  return d !== null && d !== undefined && d <= 7
}
