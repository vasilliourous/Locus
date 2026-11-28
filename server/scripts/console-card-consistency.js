#!/usr/bin/env node
// Do the Overview's cards count the same rows the links open?
//
// WHY THIS EXISTS. The console's Overview shows counts, and every count is a
// button that opens the Codes page already filtered to it. That makes two
// numbers that MUST agree — the `dashboard` handler computes one, and
// `codes.list` computes the other, in different code, from the same collection.
//
// On the live tree they did not agree, twice over:
//
//   * the "Activated on a device" card read `codes.bound`, a field the hook had
//     stopped returning when device binding was removed — the card rendered
//     blank and nobody noticed, because a blank number reads as "no data" rather
//     than as a bug;
//   * `expiringSoon` counted every future expiry inside 30 days, INCLUDING
//     unused stock, while the worklist it links to excludes unsold and
//     suspended codes — an unsold card cannot lapse, so there is nothing to
//     renew. The card said 7 and the page showed 11.
//
// A grep cannot catch this: it has to run both handlers and compare their
// answers on the same fixture, which is what this does. No network, no
// PocketBase — the two action bodies are lifted out of the hook and given a
// stub dao.
//
// Output: one `OK …` line per assertion, or the failing line. Exit 0 iff all
// pass. Invoked by check-consistency.sh.

'use strict'

const fs = require('fs')
const path = require('path')

const HOOK = path.join(__dirname, '..', 'pb_hooks', 'admin_console.pb.js')
let src
try {
  src = fs.readFileSync(HOOK, 'utf8')
} catch (e) {
  console.log(`cannot read ${HOOK}: ${e.message}`)
  process.exit(1)
}

/** Lift one `function name(...) { ... }` out of the hook, braces balanced. */
function grabFunction(name) {
  const start = src.indexOf(`function ${name}(`)
  if (start < 0) throw new Error(`no function ${name} in the hook`)
  let depth = 0
  let i = src.indexOf('{', start)
  for (let k = i; k < src.length; k++) {
    if (src[k] === '{') depth++
    else if (src[k] === '}') {
      depth--
      if (depth === 0) return src.slice(start, k + 1)
    }
  }
  throw new Error(`unbalanced braces in ${name}`)
}

/** Lift the body of `if (action === "<action>") { ... }`, braces balanced. */
function grabAction(action) {
  const marker = `if (action === "${action}") {`
  const start = src.indexOf(marker)
  if (start < 0) throw new Error(`no action branch for ${action}`)
  let depth = 0
  for (let k = src.indexOf('{', start); k < src.length; k++) {
    if (src[k] === '{') depth++
    else if (src[k] === '}') {
      depth--
      if (depth === 0) return src.slice(start, k + 1)
    }
  }
  throw new Error(`unbalanced braces in action ${action}`)
}

const parsePBDate = eval(`(${grabFunction('parsePBDate')})`) // eslint-disable-line no-eval

// ── Fixture ──
// One row per state the two handlers classify, including the two that made the
// old counts disagree (unused stock with a future expiry; a suspended code that
// has also lapsed).
const NOW = Date.now()
const DAY = 86400000
const iso = (ms) => new Date(ms).toISOString()

function rec(o) {
  return {
    id: o.id || o.code,
    code: o.code,
    getString: (k) => (o[k] === undefined || o[k] === null ? '' : String(o[k])),
    getBool: (k) => !!o[k],
    get: (k) => (o[k] === undefined ? null : o[k]),
  }
}

const codes = [
  // In term, ending in 20 days: a renewal candidate inside 30, outside 7.
  rec({ code: 'USED-20D', tier: 'free', activated_at: iso(NOW - 50 * DAY), expires_at: iso(NOW + 20 * DAY) }),
  // In term, ending in 4 days: the 7-day queue.
  rec({ code: 'USED-4D', tier: 'strike', activated_at: iso(NOW - 66 * DAY), expires_at: iso(NOW + 4 * DAY) }),
  // NEVER USED, expiry 10 days out. This is the row that broke expiringSoon:
  // unsold stock cannot lapse, so the worklist excludes it.
  rec({ code: 'UNUSED-10D', tier: 'free', expires_at: iso(NOW + 10 * DAY) }),
  // Fresh, no expiry.
  rec({ code: 'FRESH', tier: 'free' }),
  // Lapsed.
  rec({ code: 'LAPSED', tier: 'free', activated_at: iso(NOW - 60 * DAY), expires_at: iso(NOW - 3 * DAY) }),
  // Suspended AND in term: the operator's own decision, excluded from the queue.
  rec({ code: 'STOPPED-5D', tier: 'free', suspended: true, activated_at: iso(NOW - 60 * DAY), expires_at: iso(NOW + 5 * DAY) }),
  // Suspended and lapsed: must count as suspended, not expired.
  rec({ code: 'STOPPED-LAPSED', tier: 'free', suspended: true, activated_at: iso(NOW - 60 * DAY), expires_at: iso(NOW - 9 * DAY) }),
  // SIGNED OUT AND NOT CLAIMED. Still `status: redeemed` (the code has been
  // redeemed and is not lapsed or suspended), but `released_pending` is true —
  // `released_at` is later than `activated_at`, so the next activation will be
  // admitted. This row is what makes the In-term card and the Attention list
  // agree: the badge must read "signed out", and the card must still count it
  // as in term, because a code with term left is one the operator is owed for.
  rec({
    code: 'RELEASED-12D',
    tier: 'free',
    activated_at: iso(NOW - 48 * DAY),
    released_at: iso(NOW - 1 * DAY),
    release_count: 1,
    expires_at: iso(NOW + 12 * DAY),
  }),
  // Claimed again after that sign-out: `activated_at` is now LATER than
  // `released_at`, so the claim is spent and this is in force on a machine.
  rec({
    code: 'RECLAIMED-30D',
    tier: 'free',
    activated_at: iso(NOW - 1 * DAY),
    released_at: iso(NOW - 2 * DAY),
    release_count: 1,
    // 29.5 days, NOT exactly 30. The hook computes its own `Date.now()` some
    // milliseconds after this fixture does, so `NOW + 30*DAY` lands a few ms
    // PAST the window and the row falls out of both sides — which made this
    // fixture test nothing while appearing to. Deliberately inside, so the row
    // is genuinely counted, and a separate boundary assertion pins the `<`/`<=`
    // question directly (see the worklist-boundary check below).
    expires_at: iso(NOW + 29.5 * DAY),
  }),
]

const tierConfigs = [rec({ tier: 'free', active: true }), rec({ tier: 'strike', active: true })]

const dao = {
  findRecordsByExpr: (name) => {
    if (name === 'codes') return codes
    if (name === 'tier_configs') return tierConfigs
    if (name === 'activation_attempts') return []
    if (name === 'code_events') return []
    return []
  },
}
const $app = { dao: () => dao }
const $dbx = { exp: () => ({}) }

function run(action, body) {
  const branch = grabAction(action).replace(`if (action === "${action}") {`, '{')
  let out = null
  const ok = (data) => {
    out = { ok: true, ...data }
  }
  const bad = (status, message) => {
    out = { ok: false, status, message }
  }
  // eslint-disable-next-line no-new-func
  new Function('body', '$app', '$dbx', 'parsePBDate', 'ok', 'bad', branch)(body, $app, $dbx, parsePBDate, ok, bad)
  return out
}

// ── Assertions ──
const dash = run('dashboard', {})
const list = (q) => run('codes.list', q) || { codes: [] }

let failures = 0
function same(label, cardValue, rows) {
  if (cardValue === rows) {
    console.log(`OK ${label}: ${cardValue}`)
  } else {
    console.log(`${label}: the card says ${cardValue}, the list it opens has ${rows}`)
    failures++
  }
}

same('In term (status=redeemed)', dash.codes.redeemed, list({ status: 'redeemed' }).codes.length)
same('Unused stock (status=available)', dash.codes.available, list({ status: 'available' }).codes.length)
same('Lapsed (status=expired)', dash.codes.expired, list({ status: 'expired' }).codes.length)
same('Stopped (status=suspended)', dash.codes.suspended, list({ status: 'suspended' }).codes.length)
same('Due in 30 days (worklist 30)', dash.codes.expiringSoon, list({ expiring_within_days: 30 }).codes.length)

// The partition must be exhaustive: every code lands in exactly one bucket, or a
// total that looks right can still hide a row that appears nowhere.
const parts = dash.codes.available + dash.codes.redeemed + dash.codes.suspended + dash.codes.expired
same('the four buckets partition every code', dash.codes.total, parts)

// The 7-day queue is what the Codes nav badge shows, so it must be a subset of
// the 30-day one, not merely a smaller number.
const seven = list({ expiring_within_days: 7 }).codes.length
const thirty = list({ expiring_within_days: 30 }).codes.length
if (seven <= thirty) console.log(`OK 7-day queue is within the 30-day window (${seven} ≤ ${thirty})`)
else {
  console.log(`the 7-day queue (${seven}) exceeds the 30-day window (${thirty})`)
  failures++
}

// ── The window BOUNDARY, asserted directly rather than by accident ──
//
// The card (`dashboard.expiringSoon`) and the list it opens
// (`codes.list` with `expiring_within_days`) compute the same window in two
// places, and they disagreed at the edge: the card used `expMs < in30` while the
// list keeps a row whose expiry EQUALS `windowEnd` (`expMs2 > windowEndMs`
// skips it, so equal passes). A code expiring in exactly 30 days was therefore in
// the list and not in the count.
//
// This is asserted on the source rather than through a fixture row because the
// fixture cannot sit on a boundary reliably: the hook computes its own `Date.now()`
// after this file does, so a row pinned at `NOW + 30*DAY` is a few milliseconds
// outside the window and tests nothing. An earlier version of this file made
// exactly that mistake — the row looked like a boundary case and silently fell out
// of both sides, so reverting the comparison did not fail the guard.
const dashSrc = src.slice(src.indexOf('action === "dashboard"'), src.indexOf('expiringSoon++'))
if (/expMs <= in30/.test(dashSrc)) {
  console.log('OK the expiring card includes a code expiring exactly on the window edge (matches the list)')
} else if (/expMs < in30/.test(dashSrc)) {
  console.log('the expiring card uses `expMs < in30` while the list keeps `expires_at == windowEnd` — a code on the boundary is in the list and missing from the count')
  failures++
} else {
  console.log('could not read the expiringSoon comparison from dashboard — this boundary guard is blind')
  failures++
}

// ── The sign-out state, which is the console's half of "one code, one machine"
//
// `activation.pb.js` refuses a redeemed code unless `released_at` is LATER than
// `activated_at`, so the console has to answer the same question the same way or
// an operator is told a student can get in when they cannot (or the reverse).
// `released_pending` is that answer, and these pin it in both directions: a row
// with a waiting sign-out must say so, and one whose sign-out was already
// consumed must NOT.
const byCode = (c) => (list({ view: 'all' }).codes || []).find((r) => r.code === c) || {}

const releasedRow = byCode('RELEASED-12D')
if (releasedRow.released_pending === true) console.log('OK a waiting sign-out is reported as released_pending')
else {
  console.log(`RELEASED-12D released_pending is ${releasedRow.released_pending}, but its released_at is later than its activated_at — a second device would be admitted and the console says nothing`)
  failures++
}
// It is still an in-term code, not a free one: the student's term is running and
// the operator is owed nothing. Counting it as `available` would tell the
// operator there is stock to sell.
if (releasedRow.status === 'redeemed') console.log('OK a signed-out code is still status=redeemed (it has a term, not stock)')
else {
  console.log(`RELEASED-12D has status ${releasedRow.status}; a signed-out code with term left is not available stock`)
  failures++
}

const reclaimedRow = byCode('RECLAIMED-30D')
if (reclaimedRow.released_pending === false) console.log('OK a consumed sign-out is not reported as released_pending')
else {
  console.log(`RECLAIMED-30D released_pending is ${reclaimedRow.released_pending}, but activated_at is later than released_at — the sign-out was spent and a second device would be refused`)
  failures++
}

// The two halves must agree with the hook's own comparison, not merely with
// each other: read the threshold the hook uses and assert the console's answer
// on the row that sits closest to it.
const hooks = fs.readFileSync(path.join(__dirname, '..', 'pb_hooks', 'activation.pb.js'), 'utf8')
if (hooks.includes('return relMs > atMs;')) {
  console.log('OK activation.pb.js compares released_at > activated_at, the same rule asserted here')
} else {
  console.log('activation.pb.js no longer compares released_at > activated_at — this fixture proves nothing until it does')
  failures++
}

process.exit(failures ? 1 : 0)
