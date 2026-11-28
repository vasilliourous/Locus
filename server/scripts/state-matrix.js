#!/usr/bin/env node
// I2 — every operator-visible state, from every other state.
//
// WHY THIS EXISTS
//
// "There is no announced business procedure and there probably never will be"
// was, in the first pass, satisfied by moving two buttons. That is not a property
// anything can check, so the requirement is restated as a matrix: for every pair
// (from, to) of states the console can display, there must be a direct operator
// action (or two, with no ordered prerequisite) that produces `to`, or the cell
// must be IMPOSSIBLE for a stated reason.
//
// A matrix claimed in prose is a matrix nobody ran. So this lifts the REAL
// action handlers out of `admin_console.pb.js` and the REAL `stateLabel` out of
// `model.ts`, drives each cell against a stub dao, and compares the label the
// console would RENDER with the cell's target.
//
// WHAT IT CANNOT SEE
//
// It does not prove the console has a BUTTON for each action. That binding is
// checked in the browser (`verify/requirements.mjs`, `requirements` battery) and
// read in the report's I1/I2 tables. What it proves is that the state machine
// underneath the buttons actually reaches every reachable cell — which is the
// part a UI test would otherwise be asserting about the mock.
//
// Output: one `OK`/`BAD` line per cell. Exit 0 iff every cell agrees.

'use strict'

const fs = require('node:fs')
const path = require('node:path')

const SERVER = path.resolve(__dirname, '..')
const REPO = path.resolve(SERVER, '..')
const HOOK = path.join(SERVER, 'pb_hooks', 'admin_console.pb.js')
const MODEL = path.join(REPO, 'server', 'console', 'src', 'views', 'codes', 'model.ts')

const src = fs.readFileSync(HOOK, 'utf8')

/** Lift `function name(...) { ... }`, braces balanced. */
function grabFunction(body, name) {
  const start = body.indexOf(`function ${name}(`)
  if (start < 0) throw new Error(`no function ${name} in the hook`)
  let depth = 0
  for (let k = body.indexOf('{', start); k < body.length; k++) {
    if (body[k] === '{') depth++
    else if (body[k] === '}') {
      depth--
      if (depth === 0) return body.slice(start, k + 1)
    }
  }
  throw new Error(`unbalanced braces in ${name}`)
}

/**
 * Lift the BLOCK of `if (action === "x") { ... }`.
 *
 * The braces are kept and the `if (action === …)` prefix is NOT: the block is
 * spliced into a function that has no `action` variable, so keeping the condition
 * would throw `action is not defined` at the first statement.
 */
function grabAction(body, action) {
  // `codes.suspend` and `codes.unsuspend` share one branch, written as
  // `if (action === "codes.suspend" || action === "codes.unsuspend")`.
  const marker = action === 'codes.unsuspend' ? 'codes.suspend' : action
  const start = body.indexOf(`if (action === "${marker}"`)
  if (start < 0) throw new Error(`no branch for ${action}`)
  const open = body.indexOf('{', start)
  let depth = 0
  for (let k = open; k < body.length; k++) {
    if (body[k] === '{') depth++
    else if (body[k] === '}') {
      depth--
      if (depth === 0) return body.slice(open, k + 1)
    }
  }
  throw new Error(`unbalanced braces in action ${action}`)
}

// ── The console's own label function, lifted out of model.ts ───────────────
//
// Same technique as `liveness-consistency.js`: the REAL function text is
// extracted and evaluated, so the rule is the console's and not a copy of it.
// Deliberately no build step and no dependency — this runs in CI on a checkout
// with no `node_modules`, and a guard that needs a package install is a guard
// that silently does not run there.
function loadStateLabel() {
  const text = fs.readFileSync(MODEL, 'utf8')
  const start = text.indexOf('export function stateLabel')
  if (start < 0) throw new Error('stateLabel is gone from model.ts — this matrix is blind')
  let depth = 0
  let end = -1
  for (let k = text.indexOf('{', start); k < text.length; k++) {
    if (text[k] === '{') depth++
    else if (text[k] === '}') {
      depth--
      if (depth === 0) {
        end = k + 1
        break
      }
    }
  }
  if (end < 0) throw new Error('unbalanced braces around stateLabel')
  const fn = text
    .slice(start, end)
    .replace('export function stateLabel', 'function stateLabel')
    .replace(/:\s*CodeRow/g, '')
    .replace(/\)\s*:\s*string\s*\{/, ') {')
  // eslint-disable-next-line no-new-func
  return new Function(`${fn}; return stateLabel`)()
}

const stateLabel = loadStateLabel()

// ── Stub PocketBase ────────────────────────────────────────────────────────
const NOW = Date.now()
const DAY = 86400000
const iso = (ms) => new Date(ms).toISOString()

class StubRecord {
  constructor(fields) {
    this.fields = { ...fields }
    this.kind = 'codes'
  }
  get(k) {
    return Object.prototype.hasOwnProperty.call(this.fields, k) ? this.fields[k] : null
  }
  getString(k) {
    const v = this.fields[k]
    return v === null || v === undefined ? '' : String(v)
  }
  getBool(k) {
    return !!this.fields[k]
  }
  set(k, v) {
    this.fields[k] = v
  }
  collection() {
    return { fields: { getByName: () => ({ name: 'stub' }) } }
  }
}

/**
 * A fresh world: one code row, one stub dao, the real helpers bound to them.
 *
 * `handlers` is built ONCE per world because the lifted branches close over the
 * dao; the row object is what each cell mutates.
 */
function world(fields) {
  const row = new StubRecord(fields)
  const writes = []
  const audit = []
  const dao = {
    findFirstRecordByData: (coll) => {
      if (coll === 'codes') return row
      throw new Error('no rows in result set')
    },
    findFirstRecordByFilter: () => {
      throw new Error('no rows in result set')
    },
    findCollectionByNameOrId: (name) => ({ name }),
    findRecordsByExpr: (name) => (name === 'codes' ? [row] : []),
    saveRecord: (rec) => {
      if (rec && rec.kind === 'code_overrides') audit.push(rec.fields)
      else if (rec && rec.kind === 'code_events') {
        /* best-effort mirror */
      } else writes.push(rec.fields)
    },
  }
  const $app = { dao: () => dao }
  const $dbx = { exp: () => ({}) }
  function RecordStub(coll) {
    const r = new StubRecord({})
    r.kind = coll && coll.name === 'code_overrides' ? 'code_overrides' : 'code_events'
    return r
  }
  // eslint-disable-next-line no-eval
  const parsePBDate = eval(`(${grabFunction(src, 'parsePBDate')})`)

  function run(action, body, overrideBody) {
    const branch = grabAction(overrideBody || src, action)
    let out = null
    const ok = (data) => {
      out = { ok: true, ...(data || {}) }
      return out
    }
    const bad = (status, message) => {
      out = { ok: false, status, message }
      return out
    }
    const prelude = [
      grabFunction(src, 'canonical'),
      grabFunction(src, 'setIfPresent'),
      grabFunction(src, 'livenessForConfirmation'),
      grabFunction(src, 'writeOverride'),
      grabFunction(src, 'findOverride'),
      grabFunction(src, 'nowISO'),
      grabFunction(src, 'logEvent'),
    ].join('\n')
    // eslint-disable-next-line no-new-func
    // `action` is a real parameter because a shared branch reads it
    // (`codes.suspend` and `codes.unsuspend` are one block).
    const fn = new Function(
      'body', '$app', '$dbx', 'parsePBDate', 'ok', 'bad', 'Record', 'action',
      `${prelude}\n${branch}`,
    )
    fn(body, $app, $dbx, parsePBDate, ok, bad, RecordStub, action)
    return out
  }

  return { row, run, audit, writes }
}

/** The row as the console would receive it, through the REAL `codes.list`. */
function listRow(w) {
  const res = w.run('codes.list', { query: '' })
  if (!res || !res.ok || !res.codes || !res.codes.length) return null
  return res.codes[0]
}

// ── Fixtures: one row per state the console can display ────────────────────
const BASE = {
  code: 'RQ-ABCD-EFGH-JKMN-T',
  tier: 'strike',
  suspended: false,
  suspended_reason: null,
  activated_at: null,
  released_at: null,
  release_count: 0,
  first_seen_at: null,
  first_seen_fingerprint: null,
  expires_at: null,
  term_days: 70,
  force_state: '',
  evict_requested_at: null,
  evict_reason: '',
  override_id: '',
  last_seen_at: null,
}

const FIXTURES = {
  unused: { ...BASE },
  unclaimed: {
    ...BASE,
    activated_at: iso(NOW - 5 * DAY),
    first_seen_at: iso(NOW - 5 * DAY),
    released_at: iso(NOW - 1 * DAY),
    release_count: 1,
    expires_at: iso(NOW + 20 * DAY),
  },
  'in term': {
    ...BASE,
    activated_at: iso(NOW - 5 * DAY),
    first_seen_at: iso(NOW - 5 * DAY),
    expires_at: iso(NOW + 20 * DAY),
  },
  released: {
    ...BASE,
    first_seen_at: iso(NOW - 10 * DAY),
    force_state: 'released',
    expires_at: iso(NOW + 20 * DAY),
  },
  lapsed: {
    ...BASE,
    activated_at: iso(NOW - 40 * DAY),
    first_seen_at: iso(NOW - 40 * DAY),
    expires_at: iso(NOW - 3 * DAY),
  },
  stopped: {
    ...BASE,
    activated_at: iso(NOW - 5 * DAY),
    first_seen_at: iso(NOW - 5 * DAY),
    expires_at: iso(NOW + 20 * DAY),
    suspended: true,
    suspended_reason: 'chargeback',
  },
}

const STATES = Object.keys(FIXTURES)

// ── The action sequences, exactly what the console's controls send ─────────
//
// A cell names the DIRECT actions an operator takes, in order. Two actions is
// the ceiling: a third would be a workflow, which is what I2 forbids.
const PAST = '2020-01-01T00:00:00.000Z'

function actionsFor(from, to) {
  switch (to) {
    case 'in term':
      return [{ a: 'codes.override', b: { to_state: 'in_term' } }]
    case 'released':
      // A stopped code must be un-suspended first: `released` and `stopped` are
      // both claims about the same row and the badge shows the suspension first.
      if (from === 'stopped') {
        return [{ a: 'codes.unsuspend', b: {} }, { a: 'codes.release', b: {} }]
      }
      // A LAPSED code must be given a future date first: `expired` outranks
      // `available` in the derivation, so releasing it alone leaves it reading
      // "lapsed". Clearing the date is the operator saying this stock has no
      // clock until it is sold (which is what a never-expiring code is).
      if (from === 'lapsed') {
        return [{ a: 'codes.expire', b: { expires_at: '' } }, { a: 'codes.release', b: {} }]
      }
      return [{ a: 'codes.release', b: {} }]
    case 'lapsed':
      return from === 'stopped'
        ? [{ a: 'codes.expire', b: { expires_at: PAST } }, { a: 'codes.unsuspend', b: {} }]
        : [{ a: 'codes.expire', b: { expires_at: PAST } }]
    case 'stopped':
      return [{ a: 'codes.suspend', b: {} }]
    case 'unused':
    case 'unclaimed':
      return []
    default:
      throw new Error(`no recipe for ${to}`)
  }
}

/**
 * The cells that are impossible BY DESIGN, and why.
 *
 * Each is a documented invariant of this codebase, not a gap left for later:
 * `first_seen_at` is the only record a card was sold (clearing it is how a code
 * is double-sold invisibly), and `released_at` is the DEVICE's own report
 * (`docs/reference/OVERRIDES.md`, "Fields an override must never write").
 */
const IMPOSSIBLE_COLUMN = {
  'unused': 'first_seen_at is the only record that a card was ever sold; erasing it would make a second sale invisible',
  'unclaimed': 'released_at is the device\u2019s own statement, which no override may write',
}

/** Cells that are impossible for a reason specific to the PAIR. */
const IMPOSSIBLE_CELL = {
  'unused -> in term':
    'a never-issued code is claimed by the student\u2019s activation; claiming it here would make that activation fail ("already in use")',
  'unused -> released': 'released means issued-then-freed; nothing was ever issued',
}

let failed = 0
const ok = (m) => console.log(`  OK    ${m}`)
const bad = (m) => {
  failed++
  console.log(`  BAD   ${m}`)
}

for (const from of STATES) {
  for (const to of STATES) {
    const start = FIXTURES[from]
    const w = world(start)

    // The fixture must actually READ as its row label, or the row is testing a
    // state the console cannot show.
    const before = listRow(w)
    if (!before || stateLabel(before) !== from) {
      bad(`fixture "${from}" renders as "${before ? stateLabel(before) : 'nothing'}" — the matrix row is wrong`)
      continue
    }

    const steps = actionsFor(from, to)
    const outcomes = steps.map((s) => w.run(s.a, { code: start.code, ...s.b }))

    // A refused step must be the KNOWN impossibility, not a surprise.
    const refused = outcomes.find((o) => o && o.ok === false)
    const after = listRow(w)
    const label = after ? stateLabel(after) : '(missing)'

    if (from === to) {
      // An identity cell must not be a no-op REFUSAL (a dead end); a truthful
      // no-op is fine.
      if (refused && refused.status !== 400) {
        bad(`${from} → ${to} (identity) refused with ${refused.status}: ${refused.message}`)
      } else {
        ok(`${from} → ${to} (already there)`)
      }
      continue
    }

    if (label === to) {
      ok(`${from} → ${to} via ${steps.map((s) => s.a).join(' + ') || 'nothing'}`)
      continue
    }

    // Not reached. Is it one of the cells this state model cannot express?
    const why = IMPOSSIBLE_CELL[`${from} -> ${to}`] || IMPOSSIBLE_COLUMN[to]
    if (why) {
      // Two honest shapes of "impossible": no action was offered at all, or the
      // action offered was refused. Anything else means a path exists that this
      // matrix did not expect.
      if (steps.length === 0) {
        ok(`${from} → ${to} IMPOSSIBLE BY DESIGN — no operator action produces it (${why})`)
      } else if (refused) {
        ok(`${from} → ${to} IMPOSSIBLE BY DESIGN — refused: ${refused.message}`)
      } else {
        bad(`${from} → ${to} produced "${label}" without a refusal, but it is documented as impossible`)
      }
      continue
    }

    bad(
      `${from} → ${to} left the row reading "${label}"` +
        (refused ? ` (the action refused: ${refused.message})` : ' with no refusal'),
    )
  }
}

// ── The audit trail must exist for every state the setter applies ──────────
{
  const w = world(FIXTURES.released)
  w.run('codes.override', { code: FIXTURES.released.code, to_state: 'in_term' })
  if (w.audit.length === 1 && w.audit[0].aspect === 'in_term') {
    ok('codes.override writes one code_overrides row (aspect in_term)')
  } else {
    bad(`codes.override wrote ${w.audit.length} audit row(s); expected 1`)
  }
  if (w.writes.length === 1) ok('codes.override is a single record save')
  else bad(`codes.override performed ${w.writes.length} saves; expected 1`)
}

console.log()
if (failed) {
  console.log(`state-matrix FAILED — ${failed} cell(s)`)
  process.exit(1)
}
console.log(`state-matrix OK — ${STATES.length * STATES.length} cells driven through the real handlers`)
