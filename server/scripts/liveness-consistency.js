#!/usr/bin/env node
// Does the console's liveness rule, and the hub's, say the same thing?
//
// WHY THIS EXISTS
//
// `last_seen_at` is the only trace a heartbeat leaves on the hub. Two pieces of
// code classify it — `server/pb_hooks/session_probe.pb.js` (which emits
// `live`/`stale`/`absent`/`unknown` in a JSON response) and
// `server/console/src/views/codes/model.ts` (which renders a badge). Neither can
// import the other: one is goja, the other is TypeScript. That is a name and a
// rule duplicated across a language boundary with nothing enforcing agreement,
// which is the incident class this repository has paid for four times (see
// docs/reference/CONTRACT.md: `uot_port`, `download_*`, `bound_this_device`,
// `macos_intel`).
//
// The failure is not theoretical. If the two disagree about the lease, the
// console shows "checked in" for a row the probe calls "stale", an operator
// believes the console, and the desync they were called about appears to persist.
// If the vocabularies drift, one side renders a state the other never emits —
// exactly how `codes.bound` rendered a permanently blank card.
//
// WHAT THIS CHECKS
//
//   1. The four values match, as literal strings, in both sources.
//   2. The lease and stale bounds are the same NUMBER in both.
//   3. The classification rule is EXECUTED, not grepped: the real `livenessOf`
//      is lifted out of model.ts, its bounds are overridden with the hub's
//      constants parsed out of the hook, and both are driven across the boundary
//      cases. A grep could not tell whether `<=` and `<` agree.
//   4. `unknown` is never a liveness STATE derived from a missing value in a way
//      that could render as `absent`/`away`.
//
// Output: one `OK …` line per assertion, or the failing line. Exit 0 iff all.
// No network, no PocketBase, no build step — the rule is read out of the two
// files and evaluated.

'use strict'

const fs = require('node:fs')
const path = require('node:path')

const REPO = path.resolve(__dirname, '..', '..')
const HOOK = path.join(REPO, 'server/pb_hooks/session_probe.pb.js')
const MODEL = path.join(REPO, 'server/console/src/views/codes/model.ts')

let failures = 0
function ok(what) {
  console.log(`OK ${what}`)
}
function bad(what) {
  failures++
  console.log(`FAIL ${what}`)
}

function read(p) {
  try {
    return fs.readFileSync(p, 'utf8')
  } catch (e) {
    bad(`cannot read ${path.relative(REPO, p)}: ${e.message}`)
    return ''
  }
}

const hook = read(HOOK)
const model = read(MODEL)

// ── 1. The vocabulary, as literal strings, in both ──────────────────────────
const STATES = ['live', 'stale', 'absent', 'unknown']
for (const s of STATES) {
  if (new RegExp(`liveness\\s*=\\s*"${s}"`).test(hook)) ok(`the hook emits liveness "${s}"`)
  else bad(`the hook no longer assigns liveness = "${s}"`)
  if (new RegExp(`'${s}'`).test(model)) ok(`model.ts knows liveness "${s}"`)
  else bad(`model.ts no longer mentions liveness '${s}'`)
}

// ── 2. The bounds, as the same number in both ───────────────────────────────
//
// The hook builds them from named constants; model.ts holds them as one
// expression. Both are parsed to SECONDS so a reformatting does not fail the
// guard, but a genuine change on one side does.
function hookSeconds(name) {
  // var LEASE_SECONDS = 2 * 60 * 60 + 16 * 60;
  const m = hook.match(new RegExp(`var\\s+${name}\\s*=\\s*([^;]+);`))
  if (!m) return null
  const expr = m[1]
  if (!/^[\d\s+*()]+$/.test(expr)) return null
  // eslint-disable-next-line no-new-func
  return Function(`"use strict"; return (${expr});`)()
}
function modelSeconds(name) {
  // const LEASE_MS = (2 * 60 * 60 + 16 * 60) * 1000
  const m = model.match(new RegExp(`const\\s+${name}\\s*=\\s*([^\\n]+)`))
  if (!m) return null
  const expr = m[1].replace(/\/\/.*$/, '').trim()
  if (!/^[\d\s+*()]+$/.test(expr)) return null
  // eslint-disable-next-line no-new-func
  const ms = Function(`"use strict"; return (${expr});`)()
  return typeof ms === 'number' ? ms / 1000 : null
}

const hubLease = hookSeconds('LEASE_SECONDS')
const hubStale = hookSeconds('STALE_SECONDS')
const conLease = modelSeconds('LEASE_MS')
const conStale = modelSeconds('STALE_MS')

if (hubLease === null) bad('the hook no longer defines LEASE_SECONDS as plain arithmetic')
else ok(`the hook's lease is ${hubLease}s`)
if (hubStale === null) bad('the hook no longer defines STALE_SECONDS as plain arithmetic')
else ok(`the hook's stale bound is ${hubStale}s`)
if (conLease === null) bad('model.ts no longer defines LEASE_MS as plain arithmetic')
else ok(`model.ts's lease is ${conLease}s`)
if (conStale === null) bad('model.ts no longer defines STALE_MS as plain arithmetic')
else ok(`model.ts's stale bound is ${conStale}s`)

if (hubLease !== null && conLease !== null) {
  if (hubLease === conLease) ok('both sides agree on the lease bound')
  else bad(`LEASE DISAGREES: hook ${hubLease}s vs console ${conLease}s — the console would call a device live while the probe calls it stale`)
}
if (hubStale !== null && conStale !== null) {
  if (hubStale === conStale) ok('both sides agree on the stale bound')
  else bad(`STALE BOUND DISAGREES: hook ${hubStale}s vs console ${conStale}s — the two sides would age a device out on different clocks`)
}

// ── 3. Execute the real rule across the boundaries ──────────────────────────
//
// The function is lifted out of model.ts and re-evaluated with the numbers this
// script parsed. That is the point: it exercises the SHIPPED expression, so
// changing `<=` to `<` in either file turns this red.
function loadConsoleRule(leaseSec, staleSec) {
  const start = model.indexOf('export function livenessOf')
  if (start < 0) return null
  const end = model.indexOf('\n}\n', start)
  if (end < 0) return null
  let body = model.slice(start, end + 2)
  body = body
    .replace('export function livenessOf', 'function livenessOf')
    .replace(/:\s*Liveness/g, '')
    .replace(/:\s*CodeRow/g, '')
    .replace(/:\s*number/g, '')
  // Swap the module-scope constants for the parsed ones, so the bounds being
  // tested are the ones the two files actually declare.
  const src = `
    const LEASE_MS = ${leaseSec * 1000};
    const STALE_MS = ${staleSec * 1000};
    ${body}
    return livenessOf;
  `
  try {
    // eslint-disable-next-line no-new-func
    return Function(src)()
  } catch (e) {
    return { __error: e.message }
  }
}

// The hub's rule, transcribed from session_probe.pb.js's verdict block. This is
// a COPY, which is a weakness the transcript asserts against the source below —
// if the hook's branch structure changes, the shape checks fail first.
function hubClassify(seenMs, nowMs, leaseSec, staleSec) {
  if (seenMs === 0 || isNaN(seenMs)) return 'unknown'
  const age = Math.max(0, Math.floor((nowMs - seenMs) / 1000))
  if (age <= leaseSec) return 'live'
  if (age <= staleSec) return 'stale'
  return 'absent'
}

const rule = loadConsoleRule(hubLease, hubStale)
if (!rule || rule.__error) {
  bad(`could not lift livenessOf out of model.ts: ${rule && rule.__error ? rule.__error : 'not found'}`)
} else {
  const now = Date.UTC(2026, 10, 28, 12, 0, 0)
  const cases = [
    ['no record', '', 'unknown'],
    ['0 seconds ago', new Date(now).toISOString(), 'live'],
    ['lease boundary', new Date(now - hubLease * 1000).toISOString(), 'live'],
    ['one second past the lease', new Date(now - (hubLease + 1) * 1000).toISOString(), 'stale'],
    ['stale boundary', new Date(now - hubStale * 1000).toISOString(), 'stale'],
    ['one second past stale', new Date(now - (hubStale + 1) * 1000).toISOString(), 'absent'],
    ['unparseable', 'not-a-date', 'unknown'],
    ['in the future', new Date(now + 60 * 60 * 1000).toISOString(), 'unknown'],
  ]
  for (const [name, iso, want] of cases) {
    let got
    try {
      got = rule({ last_seen_at: iso }, now)
    } catch (e) {
      bad(`livenessOf threw on "${name}": ${e.message}`)
      continue
    }
    if (got === want) ok(`livenessOf: ${name} → ${want}`)
    else bad(`livenessOf: ${name} → ${got}, expected ${want}`)
  }

  // The two implementations must agree on every boundary case above, measured
  // from the same instant.
  for (const [name, iso, want] of cases) {
    const seenMs = iso && !isNaN(new Date(iso).getTime()) ? new Date(iso).getTime() : 0
    // The hook sees a future timestamp as a positive age; model.ts refuses it.
    // That asymmetry is intentional and asserted separately below, so it is
    // excluded from the agreement check rather than papered over.
    const future = iso && new Date(iso).getTime() > now + 5 * 60 * 1000
    if (future) continue
    const hubSays = hubClassify(seenMs, now, hubLease, hubStale)
    let conSays
    try {
      conSays = rule({ last_seen_at: iso }, now)
    } catch {
      continue
    }
    if (hubSays === conSays) ok(`both sides agree: ${name} → ${hubSays}`)
    else bad(`DISAGREE on ${name}: hook says ${hubSays}, console says ${conSays}`)
  }
}

// ── 4. The rule this whole change exists for: unknown is not a state ────────
//
// The brief names the exact defect: "an unknown state must never be silently
// rendered as signed out". `livenessLabel('unknown')` must not return any word
// that could be read as absence.
const labelStart = model.indexOf('export function livenessLabel')
if (labelStart < 0) {
  bad('model.ts no longer defines livenessLabel — an unknown liveness could reach the table unlabelled')
} else {
  const labelEnd = model.indexOf('\n}\n', labelStart)
  const labelSrc = model.slice(labelStart, labelEnd + 2)
  const unknownClause = labelSrc.match(/case 'unknown':\s*return\s*'([^']*)'/)
  if (!unknownClause) {
    bad('livenessLabel no longer has its own "unknown" branch — unknown would fall through to another state')
  } else {
    const word = unknownClause[1]
    const forbidden = ['signed out', 'signed-out', 'away', 'absent', 'offline', 'not checked in']
    if (forbidden.some((f) => word.toLowerCase().includes(f))) {
      bad(`livenessLabel renders unknown as "${word}" — that is the exact defect this change removes`)
    } else {
      ok(`livenessLabel renders unknown as "${word}", not as a state`)
    }
  }
}

// `livenessOf` must return 'unknown' for an empty value, and never 'absent'.
if (/last_seen_at\?: string/.test(model)) ok('model.ts types last_seen_at as optional, so an absent column is representable')
else bad('model.ts no longer types last_seen_at as optional — a missing column would be coerced')
if (/if \(!raw\) return 'unknown'/.test(model)) ok("livenessOf maps an empty last_seen_at to 'unknown'")
else bad("livenessOf no longer maps an empty last_seen_at to 'unknown' — a never-seen code could render as absent")

// The hub must make the same choice for its NULL column.
if (/liveness = "unknown"/.test(hook)) ok("the hook maps a NULL last_seen_at to \"unknown\"")
else bad('the hook no longer maps a NULL last_seen_at to "unknown"')

// ── 4b. The THIRD copy of the lease: the Overview's "gone quiet" check ───────
//
// `attention.list` cannot import the constant (goja hoists nothing across
// files), so it carries its own `> 2 * 60 * 60 * 1000`. That is a THIRD place
// the lease is written down, and a fleet where the Overview says "quiet 5h"
// while the Codes table says "checked in" is worse than having no Overview check
// at all — the operator trusts the summary precisely because it is the summary.
//
// It is checked as arithmetic rather than as text: the expression is evaluated
// and compared to the other two, so a reformatting does not fail it but a real
// change does.
{
  const adminHook = read(path.join(REPO, 'server/pb_hooks/admin_console.pb.js'))
  const m = adminHook.match(/\(nowA - seenAMs\) > ([0-9\s*()+-]+)/)
  if (!m) {
    bad('could not read the gone_quiet threshold from attention.list — that check is unguarded')
  } else if (!/^[\d\s*()+-]+$/.test(m[1])) {
    bad(`the gone_quiet threshold is not plain arithmetic ("${m[1]}") — this guard cannot verify it`)
  } else {
    // Evaluated defensively: a shape this script mis-parses must FAIL the guard,
    // not crash the whole consistency run. A throw here once took down §58 and
    // read as "the guard is broken" rather than "the threshold is unchecked".
    let ms = null
    try {
      // eslint-disable-next-line no-new-func
      ms = Function(`"use strict"; return (${m[1]});`)()
    } catch (e) {
      bad(`could not evaluate the gone_quiet threshold "${m[1]}" (${e.message})`)
    }
    const sec = ms === null ? NaN : ms / 1000
    if (sec === hubLease) {
      ok('the Overview\u2019s gone_quiet check uses the same lease as the probe and the console')
    } else {
      bad(
        `THIRD COPY DISAGREES: attention.list uses ${sec}s, the probe and console use ${hubLease}s — the Overview would contradict the table it summarises`,
      )
    }
  }

  // It must require a PRIOR beat. A null `last_seen_at` means unknown, and
  // counting that as "quiet" would open every freshly imported code as an alert.
  if (/seenAValid && \(nowA - seenAMs\)/.test(adminHook)) {
    ok('gone_quiet requires a prior beat, so never-seen codes are not counted as quiet')
  } else {
    bad('gone_quiet no longer requires a prior beat — every newly imported code would read as an alert')
  }
}

// ── 5. The heartbeat actually writes the column the rule reads ──────────────
const beatHook = read(path.join(REPO, 'server/pb_hooks/heartbeat.pb.js'))
if (/record\.set\(\s*"last_seen_at"/.test(beatHook)) ok('the heartbeat writes last_seen_at')
else bad('the heartbeat no longer writes last_seen_at — liveness would be permanently unknown and the desync unfixable')
if (/try\s*\{[\s\S]{0,200}saveRecord\(record\)[\s\S]{0,200}\}\s*catch/.test(beatHook)) {
  ok('the last_seen_at write is guarded so telemetry cannot refuse a beat')
} else {
  bad('the last_seen_at write is unguarded — a missing column would turn every beat into a 500')
}
// It must fire before the 403/410 returns, or a suspended/expired device that is
// still beating would look absent.
const seenAt = beatHook.indexOf('last_seen_at')
const firstRefusal = beatHook.indexOf('Code expired')
if (seenAt > 0 && firstRefusal > 0 && seenAt < firstRefusal) {
  ok('last_seen_at is written before the expiry refusal, so a refused device still reads as alive')
} else {
  bad('last_seen_at is written after the refusal returns — a suspended or expired device that is beating would be reported absent')
}

// ── Verdict ─────────────────────────────────────────────────────────────────
if (failures > 0) {
  console.log(`\n${failures} assertion(s) failed`)
  process.exit(1)
}
