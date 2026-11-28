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
  /**
   * The last time this code checked in with the hub, or `''` when it never has.
   *
   * This is the LIVENESS axis, and it is a different fact from `status`, which is
   * the ENTITLEMENT axis. They were conflated, and that is the defect this field
   * exists to fix: `status: 'signed out'` is derived from `released_at` — "the
   * student pressed Sign out since the last activation" — which says nothing
   * about whether the device is in use. A client beating every five minutes was
   * displayed as signed out because a stale `released_at` happened to be present.
   *
   * READ THIS BEFORE RENDERING IT. `''` means unknown, and unknown MUST NOT be
   * rendered as any liveness state — see `livenessOf` below.
   */
  last_seen_at?: string
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
  //
  // IT DOES **NOT** SAY "SIGNED OUT", and that change is the whole point of the
  // liveness column beside it. `released_at` means "a Sign out was recorded since
  // the last activation" — an ENTITLEMENT fact about the CODE. It says nothing
  // about whether a device is using it, and for a beating device it is usually
  // stale (a reinstall that re-activated, a claim nobody spent, a sign-out from a
  // machine the student has since replaced).
  //
  // Rendered as "signed out" beside "checked in 2m ago" those two columns
  // contradict each other, and the operator believes the word over the datum —
  // which is how the original report ("the hub says I'm signed out") happened.
  // The label now names its own axis: the CODE is released, not the DEVICE
  // signed out. The runbook's step 0 quotes this wording.
  if (row.redeemed && row.released_pending) return 'sign-out recorded'
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

// ── The liveness axis ─────────────────────────────────────────────────────────
//
// WHY THIS IS SEPARATE FROM `stateLabel`, AND MUST STAY SEPARATE
//
// The hub has two independent questions about a code and they have different
// owners:
//
//   entitlement — "may this code be used?"   → the HUB's to answer (it refuses)
//   liveness    — "is this device alive?"    → the CLIENT's to answer (it beats)
//
// Collapsing them into one badge is the bug this change removes. The console used
// to answer the liveness question from `released_at`, an entitlement fact, so a
// client that was heartbeating every five minutes rendered as "signed out". No
// amount of fixing the comparison would have helped: the input was the wrong
// fact. So the two axes are now two fields, and `stateLabel` keeps its existing
// entitlement meaning.
//
// The vocabulary below is a CONTRACT with `server/pb_hooks/session_probe.pb.js`,
// which emits the same five strings. `check-consistency.sh` §43 asserts they
// agree, because a string duplicated across languages with nothing enforcing
// agreement is the incident class this repo has paid for four times
// (docs/reference/CONTRACT.md).
export type Liveness = 'live' | 'stale' | 'absent' | 'unknown'

/**
 * How long a client may be silent before it is not "live".
 *
 * Derived from the CLIENT's heartbeat constants — `MAX_INTERVAL` is 2 h, ±10 %
 * jitter, 15 s timeout, one maintenance-tunnel retry — not chosen by feel. A
 * shorter lease would declare a healthy device absent the moment it backed off
 * to its maximum interval after an outage, which is the failure this number
 * exists to prevent. `server/pb_hooks/session_probe.pb.js` carries the same
 * number for the same reason.
 *
 * The arithmetic: 2 h × 1.1 + 15 s timeout + 60 s slack = 2 h 13 min 15 s,
 * rounded up to 2 h 14 min. `docs/state.toml` [liveness] `lease_s` is the
 * canonical value, and `check-consistency.sh` §59 recomputes it from
 * `heartbeat.rs` and fails if anything here disagrees.
 *
 * `STALE_MS` mirrors the client's own `GRACE_PERIOD` (7 days) so both sides age
 * a device out on the same clock; two windows would recreate the desync.
 */
const LEASE_MS = (2 * 60 * 60 + 14 * 60) * 1000
const STALE_MS = 7 * 24 * 60 * 60 * 1000

/**
 * Classifies the liveness of a code from `last_seen_at`.
 *
 * # `unknown` is a real answer, and the most important one
 *
 * An empty or unparseable `last_seen_at` returns `'unknown'`, NEVER `'absent'`.
 * A code that has never beaten has no evidence either way, and rendering that as
 * "away" — still less as "signed out" — would accuse a device on no evidence.
 * The brief for this work names this as the failure that must not happen: an
 * unknown state silently rendered as a definite one. `livenessLabel` renders it
 * as "—" for exactly that reason.
 *
 * `now` is injectable so the boundaries are testable without freezing the clock,
 * which is how the lease constants are shown to be the ones actually applied.
 */
export function livenessOf(row: CodeRow, now: number = Date.now()): Liveness {
  const raw = row.last_seen_at
  if (!raw) return 'unknown'
  const seen = new Date(raw).getTime()
  if (Number.isNaN(seen)) return 'unknown'
  // A future timestamp means the two clocks disagree or the column was written
  // wrongly. Either way the age is not knowable, and it must not be reported as
  // "live forever" — the negative age would otherwise pass every bound below.
  if (seen > now + 5 * 60 * 1000) return 'unknown'
  const age = now - seen
  if (age <= LEASE_MS) return 'live'
  if (age <= STALE_MS) return 'stale'
  return 'absent'
}

/**
 * The operator's word for a liveness state.
 *
 * Deliberately says "checked in" rather than "online": `last_seen_at` proves the
 * device REACHED THE HUB, not that the student is at it. A laptop in a tray,
 * connected to nothing, beats every five minutes and is genuinely `live`. Calling
 * that "online" would be a claim about the student that the data does not
 * support; "checked in 2m ago" is precisely what was observed.
 */
export function livenessLabel(state: Liveness): string {
  switch (state) {
    case 'live':
      return 'checked in'
    case 'stale':
      return 'not checked in'
    case 'absent':
      return 'away'
    // NOT "unknown" and NOT "signed out": a dash says "no data" without saying
    // anything false. This branch is the whole point of the type.
    case 'unknown':
      return '—'
  }
}

/**
 * The `livenessLabel` result with the age appended, for a table cell.
 *
 * Kept apart from the label so a caller that only wants the word does not have to
 * strip the number back off. `now` is injectable for the same reason as above.
 */
export function livenessText(row: CodeRow, now: number = Date.now()): string {
  const state = livenessOf(row, now)
  const label = livenessLabel(state)
  const raw = row.last_seen_at
  if (state === 'unknown' || !raw) return label
  const seen = new Date(raw).getTime()
  const ageMin = Math.max(0, Math.round((now - seen) / 60000))
  if (ageMin < 60) return `${label} ${ageMin}m ago`
  const ageH = Math.round(ageMin / 60)
  if (ageH < 48) return `${label} ${ageH}h ago`
  return `${label} ${Math.round(ageH / 24)}d ago`
}
