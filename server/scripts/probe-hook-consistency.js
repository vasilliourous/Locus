#!/usr/bin/env node
// Does `session_probe.pb.js` actually behave as documented?
//
// WHY THIS EXISTS
//
// The probe is the endpoint an operator is told to trust when the console and the
// client disagree — `docs/operate/RUNBOOK-DESYNC.md` is written entirely against
// its answers. So the one thing that must not happen is the endpoint quietly
// answering something else. Reading it does not establish that: goja is not
// Node, and the branch order, the `entitlement` precedence and the uniform-404
// rule are all behaviour, not text.
//
// This lifts the REAL handler out of the hook, gives it a stub `e` and a stub
// `$app`, and drives it. That is the same technique as
// `console-card-consistency.js`, and it exists for the same reason: a grep cannot
// tell whether two branches agree about precedence.
//
// WHAT IT ASSERTS
//
//   1. The uniform 404 really is uniform — unknown / suspended / expired /
//      wrong-device are byte-identical, and none of them is distinguishable.
//   2. The `entitlement` precedence is the documented one (suspended beats
//      expired beats released-unclaimed beats never-activated).
//   3. The liveness bounds are the documented ones, at the boundary.
//   4. `unknown` is returned for a null `last_seen_at`, never `absent`.
//   5. The write-nothing rule: the handler never calls a save.
//   6. The code never appears in any response body.
//   7. `probe_version` is required and an unknown one is refused, not guessed.
//
// Output: one `OK …` line per assertion, or the failing line. Exit 0 iff all.

'use strict'

const fs = require('node:fs')
const path = require('node:path')

const HOOK = path.join(__dirname, '..', 'pb_hooks', 'session_probe.pb.js')
let src
try {
  src = fs.readFileSync(HOOK, 'utf8')
} catch (e) {
  console.log(`cannot read ${HOOK}: ${e.message}`)
  process.exit(1)
}

let failures = 0
function ok(what) {
  console.log(`OK ${what}`)
}
function bad(what) {
  failures++
  console.log(`FAIL ${what}`)
}

// ── Lift the handler body out of `routerAdd("POST", "/api/session-probe", fn)` ─
//
// Brace-matching rather than a regex, because the body contains braces in strings
// and comments and a regex would stop early — the failure mode that makes a
// guard silently test half a function.
function extractHandler(source) {
  const marker = 'routerAdd("POST", "/api/session-probe", function (e) {'
  const start = source.indexOf(marker)
  if (start < 0) return null
  let i = source.indexOf('{', start)
  let depth = 0
  for (let k = i; k < source.length; k++) {
    const ch = source[k]
    if (ch === '{') depth++
    else if (ch === '}') {
      depth--
      if (depth === 0) return source.slice(i + 1, k)
    }
  }
  return null
}

const body = extractHandler(src)
if (!body) {
  bad('could not lift the /api/session-probe handler out of session_probe.pb.js — this harness is blind')
  process.exit(1)
}

// ── The stub PocketBase ──────────────────────────────────────────────────────
//
// `record` is a plain object with the accessor names the hook uses. Anything the
// hook calls that is missing throws, which is deliberate: a stub that silently
// returns undefined would let a wrong branch pass.

function makeRecord(fields) {
  return {
    __saved: 0,
    get(k) {
      return Object.prototype.hasOwnProperty.call(fields, k) ? fields[k] : null
    },
    getString(k) {
      const v = fields[k]
      return v === null || v === undefined ? '' : String(v)
    },
    getBool(k) {
      return fields[k] === true
    },
    set(k, v) {
      fields[k] = v
    },
    save() {
      this.__saved++
    },
  }
}

/**
 * Runs the lifted handler against a fixture.
 *
 * Returns `{status, body, saved}` — `saved` counts any record save, which the
 * read-only assertion needs.
 */
function runProbe(fixture, requestData) {
  let status = null
  let responseBody = null
  let saved = 0

  const record = fixture ? makeRecord(fixture.fields || {}) : null
  if (record) {
    const originalSave = record.save
    record.save = function () {
      saved++
      return originalSave.call(this)
    }
  }

  const e = {
    json(code, payload) {
      status = code
      responseBody = payload
      return { status: code, body: payload }
    },
  }

  const $app = {
    dao() {
      return {
        findFirstRecordByData(collection, field, value) {
          if (collection !== 'codes') throw new Error(`unexpected collection ${collection}`)
          if (field !== 'code') throw new Error(`unexpected lookup field ${field}`)
          if (!record) {
            // The real PocketBase THROWS for an unknown code; heartbeat.pb.js
            // documents that it surfaces as 500 without the try/catch.
            throw new Error('sql: no rows in result set')
          }
          // HONOUR THE KEY. A stub that returned the record for ANY value would
          // let a fixture whose code does not match the stored one pass as a
          // match — which is exactly what it did, and how the unparseable-code
          // case was briefly "failing" against correct hook code.
          const stored = fixture && fixture.fields && fixture.fields.code
          if (String(value) !== String(stored)) {
            throw new Error('sql: no rows in result set')
          }
          return record
        },
        saveRecord() {
          saved++
        },
      }
    },
  }

  const $apis = {
    requestInfo() {
      return { data: requestData }
    },
  }

  // `var` declarations in the body must not leak between runs, so each run gets a
  // fresh function scope. `routerAdd` is never called — the body is already
  // extracted.
  const fn = new Function('e', '$app', '$apis', 'routerAdd', body)
  try {
    fn(e, $app, $apis, () => {})
  } catch (err) {
    return { status: 'threw', error: err.message, body: null, saved }
  }
  return { status, body: responseBody, saved }
}

// ── Fixtures ─────────────────────────────────────────────────────────────────
//
// Times are fixed so the ages are exact. The hook uses `Date.now()`, so the
// fixtures are built relative to it rather than hardcoded — a hardcoded date
// would silently start asserting a different age as the calendar moved.
const NOW = Date.now()
const iso = (msAgo) => new Date(NOW - msAgo).toISOString()
const MIN = 60 * 1000

// The lease and the stale bound, taken from the DERIVATION rather than restated.
//
// These were literals (`2h16m`), which made this file a FIFTH copy of a number
// that already lived in the hook, the console and the Overview — and it was the
// copy nobody grepped when the lease was corrected to 2h14m on 2026-11-28. §58
// caught it immediately ("at the lease → stale, expected live"), which is the
// guard doing its job, but the durable fix is not to retype it: it is to read the
// hook's own value, so this file cannot disagree with the code it drives.
function hookSeconds(name) {
  const m = new RegExp(`var\\s+${name}\\s*=\\s*([0-9\\s*+]+);`).exec(src)
  if (!m) throw new Error(`cannot read ${name} from session_probe.pb.js`)
  // Plain integer arithmetic, evaluated. Anything else is a shape change this
  // guard must not silently paper over.
  return Function(`"use strict"; return (${m[1]});`)()
}
const LEASE_MS = hookSeconds('LEASE_SECONDS') * 1000
const STALE_MS = hookSeconds('STALE_SECONDS') * 1000

const DEVICE = 'a1b2c3d4e5f6'

function baseFields(over) {
  return {
    code: 'RQ-ABCD-EFGH-JKMN-T',
    tier: 'strike',
    activated_at: iso(5 * 24 * 60 * MIN),
    released_at: null,
    expires_at: new Date(NOW + 30 * 24 * 60 * MIN).toISOString(),
    suspended: false,
    suspended_reason: null,
    last_seen_at: iso(2 * MIN),
    first_seen_fingerprint: DEVICE,
    ...over,
  }
}

function req(over) {
  return { probe_version: 1, code: 'RQ-ABCD-EFGH-JKMN-T', device_id: DEVICE, ...over }
}

// ── 1. probe_version is required and unanswered versions are refused ─────────
{
  const r = runProbe({ fields: baseFields() }, { code: 'RQ-ABCD-EFGH-JKMN-T', device_id: DEVICE })
  if (r.status === 400) ok('a missing probe_version is 400, not silently interpreted')
  else bad(`a missing probe_version answered ${r.status}, expected 400`)

  const r2 = runProbe({ fields: baseFields() }, req({ probe_version: 2 }))
  if (r2.status === 409 && r2.body && r2.body.probe_version === 1) {
    ok('an unknown probe_version is 409 and names the supported one')
  } else {
    bad(`probe_version 2 answered ${r2.status} ${JSON.stringify(r2.body)}, expected 409 carrying ${1}`)
  }
}

// ── 2. The uniform 404, and the gate that makes it safe ────────────────────
//
// THE REAL CONTRACT, which an earlier version of this file got wrong: there is
// ONE gate, the machine check, and it runs BEFORE any state is computed.
//
//   * A caller who cannot present the machine that redeemed the code gets 404 —
//     for an unknown code, a wrong device id, a missing device id, and a
//     pre-migration row alike. Those must be indistinguishable.
//   * A caller who CAN present the machine gets 200 with the detail, INCLUDING
//     for a suspended or expired code. That is not a leak: they hold the machine,
//     and the beat already tells them the same thing in the operator's words.
//
// The second half is the security property that matters, because it is enforced
// only by STATEMENT ORDER. If the machine check were moved below the entitlement
// block, every card-holder would see every code's state with nothing at runtime
// to say so. The ordering assertion below is the guard for that.
{
  const notFoundCases = [
    ['unknown code', null, req()],
    ['wrong device id', { fields: baseFields() }, req({ device_id: 'ffffffffffff' })],
    ['no device id', { fields: baseFields() }, req({ device_id: '' })],
    [
      'pre-migration code with no first_seen_fingerprint',
      { fields: baseFields({ first_seen_fingerprint: null }) },
      req(),
    ],
    // An unparseable code normalises to itself and is looked up like any other —
    // it simply is not in the collection, so the stub throws and the hook answers
    // the uniform 404. The fixture must NOT store the same string, or the case
    // tests nothing (which it briefly did).
    ['unparseable code', { fields: baseFields() }, req({ code: 'not-a-code' })],
  ]

  const bodies = []
  for (const [name, fixture, request] of notFoundCases) {
    const r = runProbe(fixture, request)
    if (r.status !== 404) {
      bad(`${name} answered ${r.status}, expected the uniform 404`)
      continue
    }
    bodies.push([name, JSON.stringify(r.body)])
  }
  if (bodies.length === notFoundCases.length) {
    const first = bodies[0][1]
    if (bodies.every(([, b]) => b === first)) {
      ok(`all ${notFoundCases.length} ungated paths are byte-identical (${first})`)
    } else {
      const differ = bodies.filter(([, b]) => b !== first).map(([n]) => n)
      bad(`the 404s are distinguishable — ${differ.join(', ')} differ from the unknown-code answer`)
    }
  }

  // The machine that owns the code DOES see the detail, even when refused. If
  // this ever 404s, the probe has become useless for the one caller it is for.
  const owned = runProbe({ fields: baseFields({ suspended: true, suspended_reason: 'chargeback' }) }, req())
  if (owned.status === 200 && owned.body.entitlement === 'suspended') {
    ok('the owning machine sees a suspension (the gate is the machine check, not the state)')
  } else {
    bad(`the owning machine got ${owned.status} for a suspended code — the probe cannot diagnose a suspension`)
  }

  // The ordering property, asserted on the source. This is the one thing a
  // behaviour test cannot fully distinguish: a relocated check that still
  // happens to produce the same answer for these fixtures.
  //
  // TESTED AGAINST THE EARLIEST STATE READ, NOT `var entitlement`. `suspended`
  // and `expires_at` are read BEFORE the `entitlement` chain, so comparing the
  // gate to that chain lets a gate placed after the reads pass — which an earlier
  // version of this assertion did, and the mutation that moved the gate below
  // `record.getBool("suspended")` went GREEN. That is a guard that cannot fail.
  const gateAt = src.indexOf('if (!machineMatches) return opaqueNotFound()')
  const firstStateReadAt = Math.min(
    ...['record.getBool("suspended")', 'parsePBDate(record.get("expires_at"))', 'var entitlement'].map((m) => {
      const at = src.indexOf(m)
      return at < 0 ? Number.POSITIVE_INFINITY : at
    }),
  )
  if (gateAt > 0 && Number.isFinite(firstStateReadAt) && gateAt < firstStateReadAt) {
    ok('the machine check precedes every state read (the oracle gate is ordered correctly)')
  } else {
    bad(
      'the machine check no longer precedes the first entitlement read — every card-holder would see every code state',
    )
  }
}

// ── 3. The entitlement precedence ──────────────────────────────────────────
//
// Documented order: suspended > expired > never-activated > released-unclaimed >
// active. Every pair that can co-occur is checked, because precedence is the
// thing a later edit is most likely to reorder without noticing.
{
  const table = [
    ['active', {}, 'active'],
    ['released-unclaimed', { released_at: iso(1 * MIN) }, 'released-unclaimed'],
    ['suspended beats everything', { suspended: true, released_at: iso(1 * MIN) }, 'suspended'],
    [
      'suspended beats expired',
      { suspended: true, expires_at: new Date(NOW - MIN).toISOString() },
      'suspended',
    ],
    [
      'expired beats released-unclaimed',
      { expires_at: new Date(NOW - MIN).toISOString(), released_at: iso(1 * MIN) },
      'expired',
    ],
    ['never-activated', { activated_at: null }, 'never-activated'],
    [
      'never-activated loses to suspended',
      { activated_at: null, suspended: true },
      'suspended',
    ],
  ]
  for (const [name, over, want] of table) {
    const r = runProbe({ fields: baseFields(over) }, req())
    if (r.status !== 200) {
      bad(`${name}: answered ${r.status}, expected 200`)
      continue
    }
    if (r.body.entitlement === want) ok(`entitlement precedence: ${name} → ${want}`)
    else bad(`entitlement precedence: ${name} → ${r.body.entitlement}, expected ${want}`)
  }
}

// ── 4. The liveness bounds, exactly ────────────────────────────────────────
//
// The boundary is where the two implementations could differ by one second and
// nobody would notice until an operator chased a phantom.
{
  const table = [
    ['just now', 0, 'live'],
    ['at the lease', LEASE_MS, 'live'],
    ['one second past the lease', LEASE_MS + 1000, 'stale'],
    ['at the stale bound', STALE_MS, 'stale'],
    ['one second past stale', STALE_MS + 1000, 'absent'],
  ]
  for (const [name, ago, want] of table) {
    const r = runProbe({ fields: baseFields({ last_seen_at: iso(ago) }) }, req())
    if (r.status !== 200) {
      bad(`liveness ${name}: answered ${r.status}`)
      continue
    }
    if (r.body.liveness === want) ok(`liveness: ${name} → ${want}`)
    else bad(`liveness: ${name} → ${r.body.liveness}, expected ${want}`)
  }
}

// ── 5. A null last_seen_at is UNKNOWN, never absent ────────────────────────
//
// The brief for this work names this as the failure that must not happen. It is
// the one assertion worth writing twice.
{
  for (const [name, value] of [
    ['null', null],
    ['missing', undefined],
    ['empty string', ''],
    ['unparseable', 'not-a-date'],
  ]) {
    const fields = baseFields()
    if (name === 'missing') delete fields.last_seen_at
    else fields.last_seen_at = value
    const r = runProbe({ fields }, req())
    if (r.status !== 200) {
      bad(`null-liveness (${name}): answered ${r.status}`)
      continue
    }
    if (r.body.liveness === 'unknown') ok(`a ${name} last_seen_at is 'unknown', not absent`)
    else bad(`a ${name} last_seen_at is '${r.body.liveness}' — must be 'unknown'`)

    // And the age must be null, not 0 or a negative: "never seen" and "seen now"
    // must not render the same.
    if (r.body.hub_view.last_seen_age_s === null && r.body.hub_view.last_seen_at === null) {
      ok(`a ${name} last_seen_at reports age null and last_seen_at null`)
    } else {
      bad(
        `a ${name} last_seen_at reported age=${r.body.hub_view.last_seen_age_s}, at=${r.body.hub_view.last_seen_at}`,
      )
    }
  }
}

// ── 6. The probe writes nothing ────────────────────────────────────────────
//
// STRICTLY READ-ONLY is the contract. A probe that wrote
// `last_seen_at` would be a second source of truth for liveness, and an operator
// diagnosing a device could make it look alive.
{
  const r = runProbe({ fields: baseFields() }, req())
  if (r.saved === 0) ok('the probe saves no record (strictly read-only)')
  else bad(`the probe saved a record ${r.saved} time(s) — it must write nothing`)

  const r2 = runProbe({ fields: baseFields({ suspended: true }) }, req())
  if (r2.saved === 0) ok('the probe saves nothing on the refusal path either')
  else bad(`the probe saved a record ${r2.saved} time(s) on the refusal path`)
}

// ── 7. The code never appears in a response ────────────────────────────────
//
// Logs and transcripts are things people paste. The code is a bearer credential.
{
  const r = runProbe({ fields: baseFields() }, req())
  const text = JSON.stringify(r.body || {})
  if (!text.includes('RQ-ABCD') && !text.includes('EFGH')) {
    ok('no response body carries the activation code')
  } else {
    bad('a response body contains the activation code')
  }
  // The fingerprint is deliberately echoed, but only the REDACTED prefix.
  if (text.includes(DEVICE)) ok('the redacted device id is echoed (the operator needs to confirm it)')
  else bad('the redacted device id is missing from the response')
}

// ── 8. The machine check accepts the shape the two sides ACTUALLY store ─────
//
// This is the bug that made every real probe answer 404. The client sends its
// 12-char redacted fingerprint; `activation.pb.js` stores 16 characters plus a
// literal "****". Equality could therefore never hold, and no test noticed
// because every fixture used the same string on both sides.
//
// So both real shapes are driven here: the short client value against a long
// stored value, and the long stored value (which the console quotes) against
// itself. A different prefix must still be refused — the prefix rule must not
// become "any two non-empty strings match".
{
  const stored = 'a1b2c3d4e5f699aa****'
  const longStored = runProbe({ fields: baseFields({ first_seen_fingerprint: stored }) }, req({ device_id: DEVICE }))
  if (longStored.status === 200) {
    ok('a 12-char client device id matches the 16+"****" value activation stores')
  } else {
    bad(`the client's 12-char device id was refused against a stored "${stored}" (${longStored.status}) — every real probe would answer 404`)
  }

  const consoleRun = runProbe({ fields: baseFields({ first_seen_fingerprint: stored }) }, req({ device_id: stored }))
  if (consoleRun.status === 200) {
    ok('the stored fingerprint the console quotes also passes the machine check')
  } else {
    bad(`the stored fingerprint was refused when presented verbatim (${consoleRun.status})`)
  }

  const nearMiss = runProbe({ fields: baseFields({ first_seen_fingerprint: stored }) }, req({ device_id: 'a1b2c3d4e5f7' }))
  if (nearMiss.status === 404) {
    ok('a device id that differs in one character is still the uniform 404')
  } else {
    bad(`a near-miss device id answered ${nearMiss.status}, expected 404 — the prefix rule is too loose`)
  }
}

// ── 9. An operator's override is REPORTED, not silently absorbed ───────────
//
// R3: the override must set a flag the next probe run detects and alerts on.
// `force_state` / `evict_requested_at` are that flag; this asserts the probe
// actually reads them, and that the ordinary case stays `null` rather than
// rendering as some kind of unset override.
{
  const plain = runProbe({ fields: baseFields() }, req())
  if (plain.status === 200 && plain.body.override === null) {
    ok('a code with no override reports override: null')
  } else {
    bad(`a code with no override reported ${plain.status} ${JSON.stringify(plain.body && plain.body.override)}`)
  }
  if (plain.status === 200 && !(plain.body.divergence || []).includes('operator_override')) {
    ok('a code with no override does not raise operator_override')
  } else {
    bad('a code with no override raised operator_override')
  }

  const overridden = runProbe(
    {
      fields: baseFields({
        force_state: 'released',
        override_id: 'rel-abc',
        evict_requested_at: iso(MIN),
        evict_reason: 'chargeback',
      }),
    },
    req(),
  )
  if (overridden.status === 200 && overridden.body.override && overridden.body.override.force_state === 'released') {
    ok('a released-by-override code reports force_state in the probe')
  } else {
    bad(`a released-by-override code did not report it: ${JSON.stringify(overridden.body && overridden.body.override)}`)
  }
  if (overridden.status === 200 && overridden.body.override && overridden.body.override.evicted === true) {
    ok('the probe reports that a device is being cut off (evicted: true)')
  } else {
    bad('the probe did not report the eviction on an evicted code')
  }
  if (overridden.status === 200 && (overridden.body.divergence || []).includes('operator_override')) {
    ok('an override is raised in the probe divergence list')
  } else {
    bad('an override did not appear in the probe divergence list — nothing would alert')
  }
}

// ── 10. The reason_code vocabulary is closed ───────────────────────────────
//
// `model.ts` and the runbook are keyed on these strings. An undocumented one
// would render as an unexplained value at a student.
{
  const KNOWN = new Set([
    'in_sync',
    'client_beating_hub_quiet',
    'stale_release_claim',
    'hub_refused_client_hasnt_noticed',
    'client_unreachable',
    'client_dropped_session',
    'genuinely_signed_out',
    'clock_skew',
    'unknown',
  ])
  // EXPECTED values, not just membership. A vocabulary check alone let a mutant
  // that routed everything to `in_sync` pass — the worst possible misleading
  // answer, because `in_sync` reads as "both sides agree". The expectations are
  // the operator-facing contract, so they are asserted exactly.
  const scenarios = [
    ['healthy, both observed', {}, { auth_state: 'activated' }, 'client_beating_hub_quiet'],
    ['client says activated, hub released', { released_at: iso(MIN) }, { auth_state: 'activated' }, 'stale_release_claim'],
    ['client says activated, hub suspended', { suspended: true }, { auth_state: 'activated' }, 'hub_refused_client_hasnt_noticed'],
    ['client says activated, hub expired', { expires_at: new Date(NOW - MIN).toISOString() }, { auth_state: 'activated' }, 'hub_refused_client_hasnt_noticed'],
    // THE CASE THE `in_sync` FALLTHROUGH USED TO SWALLOW. A hub that has never
    // seen the device has NO evidence of agreement, so `in_sync` is a lie here.
    ['nothing seen, client activated', { last_seen_at: null }, { auth_state: 'activated' }, 'unknown'],
    ['nothing seen, no client report', { last_seen_at: null }, null, 'unknown'],
    ['gone', { last_seen_at: iso(8 * 24 * 60 * MIN) }, { auth_state: 'activated' }, 'client_unreachable'],
    ['stale', { last_seen_at: iso(3 * 60 * MIN) }, { auth_state: 'activated' }, 'client_unreachable'],
    ['client signed out, hub live', {}, { auth_state: 'signed-out' }, 'client_dropped_session'],
    ['both signed out', { activated_at: null }, { auth_state: 'signed-out' }, 'genuinely_signed_out'],
    ['no client report, hub live', {}, null, 'in_sync'],
    [
      'clock skew is promoted above the others',
      {},
      { auth_state: 'activated', clock: { unix: Math.floor(NOW / 1000) + 3600 } },
      'clock_skew',
    ],
  ]
  for (const [name, over, client, want] of scenarios) {
    const r = runProbe({ fields: baseFields(over) }, req({ client: client === null ? undefined : client }))
    if (r.status !== 200) {
      bad(`reason_code ${name}: answered ${r.status}`)
      continue
    }
    if (!KNOWN.has(r.body.reason_code)) {
      bad(`reason_code ${name} → '${r.body.reason_code}', which is not in the documented vocabulary`)
      continue
    }
    if (r.body.reason_code === want) ok(`reason_code ${name} → ${want}`)
    else bad(`reason_code ${name} → '${r.body.reason_code}', expected '${want}'`)
  }

  // `in_sync` must be UNREACHABLE unless both sides were observed. This is the
  // exhaustive-chain property: a fallthrough to "both agree" is the failure mode
  // that makes a diagnostic actively misleading.
  const inSyncCases = [
    ['unknown liveness', { last_seen_at: null }, { auth_state: 'activated' }],
    ['absent liveness', { last_seen_at: iso(8 * 24 * 60 * MIN) }, { auth_state: 'activated' }],
    ['stale liveness', { last_seen_at: iso(3 * 60 * MIN) }, { auth_state: 'activated' }],
    ['no client report, never seen', { last_seen_at: null }, null],
  ]
  for (const [name, over, client] of inSyncCases) {
    const r = runProbe({ fields: baseFields(over) }, req({ client: client === null ? undefined : client }))
    if (r.status === 200 && r.body.reason_code !== 'in_sync') {
      ok(`${name} is not reported as in_sync`)
    } else {
      bad(`${name} was reported as 'in_sync' — the hub has no evidence of agreement`)
    }
  }
}

// ── 11. The clock-skew verdict is a threshold, not a feeling ───────────────
{
  const near = runProbe(
    { fields: baseFields() },
    req({ client: { auth_state: 'activated', clock: { unix: Math.floor(NOW / 1000) + 200 } } }),
  )
  if (!near.body.divergence.includes('clock_skew')) ok('a 200 s skew is not reported (under the 300 s threshold)')
  else bad('a 200 s skew was reported as clock_skew')

  const far = runProbe(
    { fields: baseFields() },
    req({ client: { auth_state: 'activated', clock: { unix: Math.floor(NOW / 1000) + 400 } } }),
  )
  if (far.body.divergence.includes('clock_skew')) ok('a 400 s skew is reported as clock_skew')
  else bad('a 400 s skew was not reported — the threshold or the sign is wrong')
  // ── The vocabulary, asserted in BOTH directions ────────────────────────────
  //
  // The exact-value cases above prove the hook answers the RIGHT code for the
  // fixtures driven. They cannot prove the runbook and the hook describe the same
  // set — and they did not: a reorder deleted `client_beating_hub_quiet` from the
  // hook while the runbook still opened with it, and every test here passed,
  // because no fixture asserted that code and the vocabulary check only asked
  // whether emitted values were LEGAL.
  //
  // A documented-but-unreachable code is worse than a missing one: the runbook
  // tells an operator to match a value the hub can never produce, so they
  // conclude the probe is broken. So this is checked in both directions against
  // the runbook itself, which is the document the operator actually holds.
  {
    const runbookPath = path.join(__dirname, '..', '..', 'docs', 'operate', 'RUNBOOK-DESYNC.md')
    let runbook = ''
    try {
      runbook = fs.readFileSync(runbookPath, 'utf8')
    } catch (e) {
      bad(`cannot read the runbook: ${e.message}`)
    }
    // Every `reasonCode = "x"` assignment is a code the hook can emit.
    const emitted = new Set([...src.matchAll(/reasonCode = "([a-z_]+)"/g)].map((m) => m[1]))
    // The runbook's §4 table is the operator's contract. Codes appear there as
    // `| \`code\` | ...`.
    const documented = new Set(
      [...runbook.matchAll(/^\| `([a-z_]+)` \|/gm)].map((m) => m[1]).filter((c) => c !== 'reason_code'),
    )

    if (emitted.size === 0) {
      bad('could not read any reason_code assignment from the hook — this check is blind')
    }
    if (documented.size === 0) {
      bad("could not read any reason_code from the runbook's table — this check is blind")
    }

    for (const code of emitted) {
      if (documented.has(code)) ok(`runbook documents "${code}" (which the hook emits)`)
      else bad(`the hook emits "${code}" but the runbook does not document it — an operator reads it and has no action`)
    }
    for (const code of documented) {
      if (emitted.has(code)) continue
      // `unknown`/`in_sync` are documented and reachable; anything else here is a
      // code the operator will never see, which makes the runbook look broken.
      bad(`the runbook documents "${code}" but the hook can never emit it — an operator will hunt for a value that does not exist`)
    }
  }
}
//
// A throw would surface as a bare 500 (the catch returns one), which is honest —
// but it must not happen on any of the shapes above, because a 500 in a runbook
// step reads as "the hub is broken".
{
  const shapes = [
    null,
    { fields: baseFields() },
    { fields: baseFields({ activated_at: null }) },
    { fields: baseFields({ expires_at: 'garbage' }) },
    { fields: baseFields({ last_seen_at: 'garbage' }) },
  ]
  let threw = 0
  for (const s of shapes) {
    const r = runProbe(s, req())
    if (r.status === 'threw') {
      bad(`the handler threw: ${r.error}`)
      threw++
    }
  }
  if (threw === 0) ok('the handler answers every fixture shape without throwing')
}

// ── Verdict ────────────────────────────────────────────────────────────────
if (failures > 0) {
  console.log(`\n${failures} assertion(s) failed`)
  process.exit(1)
}
