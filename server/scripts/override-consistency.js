#!/usr/bin/env node
// Does the override/eviction code say what the plan says it says?
//
// WHY THIS EXISTS, AND WHY IT IS NOT A UNIT TEST
//
// The two defects this feature can ship are structural, not arithmetic:
//
//   1. An eviction written as `codes.suspended = true`. That column is read by
//      activation.pb.js, so "evict" expressed as a suspension REFUSES THE NEXT
//      HOLDER and breaks the resale that releasing exists for. This was a real
//      bug in the first draft of the design, and it is invisible to any test that
//      only exercises the heartbeat.
//   2. An eviction predicate the heartbeat does not read, or reads in the wrong
//      order. Then a live client keeps serving a released code — the original
//      silent bug, with extra fields.
//
// Neither can be caught by the console's typecheck (different language), and
// neither can be caught by a hub unit test without a running PocketBase. So, like
// `liveness-consistency.js`, this reads the RULES OUT OF THE SOURCE and asserts
// their shape. It cannot prove the handler behaves; it can prove the two
// invariants that must hold for it to be correct at all.
//
// Output: one OK line per assertion, or the failing line. Exit 0 iff all pass.

'use strict'

const fs = require('node:fs')
const path = require('node:path')

// scripts/ → server/ → repo root
const SERVER = path.resolve(__dirname, '..')
const REPO = path.resolve(SERVER, '..')
const HOOKS = path.join(SERVER, 'pb_hooks')

let failed = 0
const ok = (m) => console.log(`  OK    ${m}`)
const bad = (m) => {
  failed++
  console.log(`  BAD   ${m}`)
}



// Extract a brace-balanced block starting at `from` (which points at a `{`).
// A fixed slice is what made the writeOverride check read the NEXT function's
// catch clause and report a failure that did not exist — the same
// "the guard reads beyond its evidence" class the repo keeps hitting.
const balanced = (s, from) => {
  const open = s.indexOf('{', from)
  if (open < 0) return ''
  let depth = 0
  for (let i = open; i < s.length; i++) {
    if (s[i] === '{') depth++
    else if (s[i] === '}') {
      depth--
      if (depth === 0) return s.slice(open, i + 1)
    }
  }
  return s.slice(open)
}

// Strip `//` comment lines. An assertion that forbids a CONSTRUCT must not read
// the comment that explains why the construct is forbidden — that is the
// guard_self trap the repo has already paid for (server/scripts/lib/guard_self.py).
const codeOnly = (s) => s.replace(/^\s*\/\/.*$/gm, '')

const read = (f) => {
  try {
    return fs.readFileSync(path.join(HOOKS, f), 'utf8')
  } catch (e) {
    bad(`cannot read ${f}: ${e.message}`)
    return ''
  }
}

// ── 1. Eviction is NOT expressed as the `suspended` column ──────────────────
//
// The release handler must not set `suspended` at all: an eviction is
// `evict_requested_at`, and `suspended` belongs to codes.suspend/kick.
{
  const admin = read('admin_console.pb.js')
  // The release block runs from the action guard to the reactivate guard.
  const start = admin.indexOf('action === "codes.release"')
  const end = admin.indexOf('action === "codes.reactivate"')
  if (start < 0 || end < 0 || end < start) {
    bad('could not locate the codes.release block')
  } else {
    const block = admin.slice(start, end)
    if (/\.set\(\s*["']suspended["']/.test(block)) {
      bad('codes.release writes the `suspended` column — this would refuse the NEXT holder and break the resale')
    } else {
      ok('codes.release does not write the `suspended` column')
    }
    if (!/evict_requested_at/.test(block)) {
      bad('codes.release does not set evict_requested_at — a live client would never learn')
    } else {
      ok('codes.release writes evict_requested_at')
    }
  }
}

// ── 2. activation.pb.js does NOT refuse on the eviction predicate ───────────
//
// The new holder must be able to activate a released code. If activation ever
// reads `evict_requested_at` in a refusal path, releasing means "nobody can use
// this", which is the opposite of what it is for.
{
  const act = codeOnly(read('activation.pb.js'))
  // Any read of evict_requested_at is allowed ONLY inside the additive
  // `released_by_override` response key.
  const hits = []
  const re = /evict_requested_at/g
  let m
  while ((m = re.exec(act)) !== null) {
    const lineStart = act.lastIndexOf('\n', m.index) + 1
    hits.push(act.slice(lineStart, act.indexOf('\n', m.index)))
  }
  // The legitimate read is the CONDITION for the additive key, which spans two
  // lines; accept it only when the assignment follows within the same if-statement.
  const legit = /String\(rec\.get\(["']evict_requested_at["']\)[\s\S]{0,200}?released_by_override\s*=\s*true/
  const stripped = codeOnly(act)
  if (!legit.test(stripped)) {
    bad('the only activation read of evict_requested_at does not feed the additive released_by_override key')
  } else {
    ok('activation reads evict_requested_at only to set the additive released_by_override key')
  }
  const offending = []
  if (offending.length) {
    bad(`activation.pb.js reads evict_requested_at outside the additive response key: ${offending[0].trim().slice(0, 80)}`)
  } else {
    ok('activation.pb.js only reads evict_requested_at for the informational key')
  }
  if (/\be\.json\(\s*40[0-9][^)]*evict_requested_at/.test(act)) {
    bad('activation.pb.js refuses on evict_requested_at')
  } else {
    ok('activation.pb.js has no eviction refusal path')
  }
}

// ── 3. The heartbeat reads the predicate, and before the suspension check ───
{
  const hb = read('heartbeat.pb.js')
  const evictAt = hb.indexOf('evict_requested_at')
  const suspAt = hb.indexOf('record.getBool("suspended")')
  if (evictAt < 0) {
    bad('heartbeat.pb.js does not read evict_requested_at — nothing evicts')
  } else if (suspAt < 0) {
    bad('could not locate the suspension check in heartbeat.pb.js')
  } else if (evictAt > suspAt) {
    bad('the eviction check comes AFTER the suspension check — an evicted+suspended code would report the wrong reason')
  } else {
    ok('the heartbeat reads evict_requested_at before the suspension check')
  }
  // The wire shape deployed clients depend on.
  if (!/suspended:\s*true/.test(hb.slice(evictAt, evictAt + 1200))) {
    bad('the eviction response does not carry `suspended: true` — deployed clients would take the credential-deleting branch')
  } else {
    ok('the eviction response carries `suspended: true` (frozen-wire reversible path)')
  }
  if (!/evicted:\s*true/.test(hb)) {
    bad('the eviction response has no `evicted` key for newer clients')
  } else {
    ok('the eviction response carries the additive `evicted` key')
  }
}

// ── 4. The audit row is hard-fail, unlike logEvent ──────────────────────────
{
  const admin = read('admin_console.pb.js')
  if (!/function writeOverride/.test(admin)) {
    bad('writeOverride is missing')
  } else {
    const start = admin.indexOf('function writeOverride')
    const block = codeOnly(balanced(admin, start))
    if (/try\s*\{[\s\S]{0,400}catch/.test(block)) {
      bad('writeOverride swallows its own exception — the override could apply with no audit row')
    } else {
      ok('writeOverride does not swallow exceptions (hard-fail audit)')
    }
  }
  // And it must run before the record save, or a failed audit leaves a changed
  // record with no justification.
  const rel = admin.slice(admin.indexOf('action === "codes.release"'))
  const callAt = rel.indexOf('writeOverride(')
  const saveAt = rel.indexOf('saveRecord(recU)')
  if (callAt < 0 || saveAt < 0) {
    bad('could not locate writeOverride/saveRecord ordering in codes.release')
  } else if (callAt > saveAt) {
    bad('codes.release saves the record BEFORE writing the audit row')
  } else {
    ok('codes.release writes the audit row before saving the record')
  }
}

// ── 5. The console renders the conflict without writing a status ────────────
{
  let src = ''
  try {
    src = fs.readFileSync(
      path.join(REPO, 'server', 'console', 'src', 'views', 'codes', 'model.ts'), 'utf8')
  } catch (e) {
    bad(`cannot read console model.ts: ${e.message}`)
  }
  if (src) {
    if (!/export function conflictOf/.test(src)) {
      bad('conflictOf is missing — the observed/main disagreement would have no render')
    } else {
      const start = src.indexOf('export function conflictOf')
      const block = src.slice(start, src.indexOf('\n}', start))
      if (/\.set|save|fetch|axios|call\(/.test(block)) {
        bad('conflictOf performs a side effect — it must be a pure render')
      } else {
        ok('conflictOf is a pure render')
      }
    }
    if (/if \(row\.suspended\) return 'stopped'/.test(src)) {
      ok('stateLabel still derives from the stored fields (status not written by an override)')
    } else {
      bad('stateLabel no longer reads the derived fields — status may have been turned into a stored value')
    }
  }
}

// ── 6. The override is NOT a fifth status value ─────────────────────────────
//
// The first implementation added `'released'` to the status vocabulary, and it
// broke three things silently: `?status=available` stopped listing released
// stock, the facet counts gained a bucket the filter bar does not render, and the
// dashboard (which counts four buckets) disagreed with the list about the same
// code. `console-card-consistency.js` already asserts the four buckets partition
// every code; this asserts the vocabulary that partition is made of has not been
// widened.
{
  const admin = codeOnly(read('admin_console.pb.js'))
  // The status derivation must not mention force_state.
  // Anchor on the FIRST line of the derivation, which must exist. Before the fix
  // this searched for the whole `var status = susp2 ? ...` prefix, so the buggy
  // version simply failed to match and the "could not locate" branch was taken —
  // a wrong reason that still failed, but by luck. Anchoring on a substring the
  // buggy version also contains is what makes the failure explain the bug.
  const relStart = admin.indexOf('var status =')
  const relEnd = admin.indexOf(';', relStart < 0 ? 0 : relStart)
  const derivation = relStart < 0 ? '' : admin.slice(relStart, relEnd + 1)
  if (!derivation) {
    bad('could not locate the `var status =` derivation — the guard has no evidence, so it cannot pass')
  // Match BOTH spellings: the column is `force_state`, the local the derivation
  // reads is `forceState`. The first version of this assertion tested only the
  // column name, so injecting the real bug (which uses the camelCase local) left
  // it green — a guard that was shown NOT to fail, which is the one thing this
  // file exists to prevent.
  } else if (/(force_state|forceState)/.test(derivation)) {
    bad('the status derivation reads the override — status must stay a four-value partition')
  } else {
    ok('the status derivation ignores the override (four-value partition intact)')
  }

  // The facet CANDIDATES must be the four rendered buckets, AND the per-row
  // bucket must be a plain ternary over the four stored fields — not a value
  // derived from the override.
  //
  // The first version of this assertion checked only the literal keys of the
  // `facetStatus = {...}` initialiser, so a FIFTH bucket produced by a dynamic
  // `fstatus` ternary passed straight through. That is the same blind spot the
  // code had: the count map is initialised with four names and can then be grown
  // by `facetStatus[fstatus] = ...`.
  const facetStart = admin.indexOf('var facetStatus = {')
  const facetEnd = admin.indexOf('}', facetStart < 0 ? 0 : facetStart)
  const facet = facetStart < 0 ? '' : admin.slice(facetStart, facetEnd + 1)
  const keys = (facet.match(/"[a-z]+"\s*:/g) || []).map((k) => k.replace(/["\s:]/g, '')).sort()
  const want = ['available', 'expired', 'redeemed', 'suspended']
  if (JSON.stringify(keys) !== JSON.stringify(want)) {
    bad(`facetStatus declares [${keys.join(', ')}] but the filter bar renders [${want.join(', ')}] — a code in a bucket nobody renders is invisible`)
  } else {
    ok('facetStatus declares exactly the four rendered buckets')
  }
  // The per-row bucket assignment must not mention the override.
  const fsStart = admin.indexOf('var fstatus =')
  const fsEnd = admin.indexOf(';', fsStart < 0 ? 0 : fsStart)
  const fstatusLine = fsStart < 0 ? '' : admin.slice(fsStart, fsEnd + 1)
  if (!fstatusLine) {
    bad('could not locate the `var fstatus =` bucket assignment — no evidence, cannot pass')
  } else if (/force_state|forceState|evict|released/.test(fstatusLine)) {
    bad('the facet bucket assignment reads the override — released stock would vanish from every count')
  } else {
    ok('the facet bucket assignment ignores the override')
  }

  // And the TypeScript union must match, or the console can receive a value it
  // has no branch for.
  const model = fs.readFileSync(
    path.join(REPO, 'server', 'console', 'src', 'views', 'codes', 'model.ts'), 'utf8')
  const m = model.match(/status:\s*'([^']+)'(?:\s*\|\s*'([^']+)')*/)
  const unionLine = (model.match(/status:\s*[^\n]+/) || [''])[0]
  const statuses = (unionLine.match(/'([a-z]+)'/g) || []).map((x) => x.replace(/'/g, '')).sort()
  if (JSON.stringify(statuses) !== JSON.stringify(want)) {
    bad(`CodeRow.status is [${statuses.join(', ')}] but the hub emits [${want.join(', ')}]`)
  } else {
    ok('CodeRow.status matches the hub vocabulary')
  }
  if (!/export function overrideNote/.test(model)) {
    bad('overrideNote is missing — the override would have no render that is not the status')
  } else {
    ok('overrideNote exists as the override\'s own render')
  }
}

// ── 7. Activation CONSUMES the override (the resale must not evict the buyer) ──
//
// The first version left `evict_requested_at` in place across a re-activation, so
// the new holder's very next heartbeat was refused: the resale appeared to work
// for one beat and then cut off the student who had just paid. That is worse than
// the bug the release fixes, so it gets its own assertion.
{
  const act = codeOnly(read('activation.pb.js'))
  // Anchor on CODE, not the banner comment — `codeOnly` strips comments, so the
  // previous anchor could never match and both assertions failed for the wrong
  // reason. Anchor on the term-recomputation that only the first-redemption arm
  // performs.
  const anchorAt = act.indexOf('var termExpiry = expiryFromTerm(')
  const from = anchorAt < 0 ? -1 : anchorAt
  const block = from < 0 ? '' : act.slice(from, from + 2500)
  if (from < 0) bad('could not locate the first-redemption arm — the guard has no evidence')
  if (block && !/setIfPresent\(rec,\s*"evict_requested_at"/.test(block)) {
    bad('the first-redemption arm does not clear evict_requested_at — a re-sold code would evict its new holder')
  } else {
    ok('a successful activation clears evict_requested_at (resale does not evict the buyer)')
  }
  if (block && !/setIfPresent\(rec,\s*"force_state"/.test(block)) {
    bad('the first-redemption arm does not clear force_state — a re-sold code stays pinned to "released"')
  } else {
    ok('a successful activation clears force_state')
  }
}

// ── 8. A released code stays protected from deletion ────────────────────────
//
// The delete guard used to key on `activated_at`, which a release clears — so a
// released code became deletable with no force flag, and the old message told the
// operator to "Release it first". Deleting it destroys the only record it was sold.
{
  const admin = codeOnly(read('admin_console.pb.js'))
  const i = admin.indexOf('action === "codes.delete"')
  const block = i < 0 ? '' : admin.slice(i, i + 2200)
  if (!/first_seen_at/.test(block)) {
    bad('codes.delete does not consult first_seen_at — a released code could be deleted with no force flag')
  } else {
    ok('codes.delete protects any code that was ever issued')
  }
}

// ── 9. THE EVICTION MESSAGE MUST CONTAIN "suspended" ────────────────────────
//
// This is the most dangerous invariant in the feature and it was broken in the
// first implementation. The deployed client does NOT read the JSON `suspended`
// boolean. `handle_refusal` decides reversibility with
// `is_suspension(reason)`, and `reason` is the `message` field. `classify_response`
// takes only `value["message"]`, so every other key is invisible to it.
//
// A message without the substring "suspended" therefore takes the NOT-reversible
// branch: `stop()` + `record_refusal()`, which DELETES the credential. A mistaken
// release would permanently lock the student out and force a card re-entry.
//
// Verified by running the real predicate against the real strings, not by reading
// the comment that used to claim the JSON flag was what mattered.
{
  const hb = read('heartbeat.pb.js')
  // Anchor on the eviction BRANCH, not the first mention of the column (which is
  // the explanatory comment far above it). From the branch to the next `e.json(`.
  const i = hb.indexOf('if (String(record.get("evict_requested_at")')
  const block = i < 0 ? '' : hb.slice(i, i + 2600)
  // Pull every string literal assigned to the eviction message.
  const assigns = [...block.matchAll(/evictMsg\s*=\s*"([^"]*)"/g)].map((m) => m[1])
  if (!assigns.length) {
    bad('could not find the eviction message strings — the guard has no evidence')
  } else {
    const bads = assigns.filter((m) => !/suspended/i.test(m))
    if (bads.length) {
      bad(`an eviction message lacks the frozen "suspended" substring, so the client would DELETE the credential: "${bads[0]}"`)
    } else {
      ok(`every eviction message carries the frozen "suspended" substring (${assigns.length} variant(s))`)
    }
    // And the with-note variant must be included, not just the default.
    if (assigns.length < 2) {
      bad('only one eviction message variant found — the with-reason variant may be missing')
    } else {
      ok('both the default and with-reason eviction messages are checked')
    }
  }
  // The classifier reads `message`, so assert that is still how it works: if a
  // future client read the `suspended` flag instead, this guard's rationale
  // would change and someone should notice.
  const hbrs = fs.readFileSync(
    path.join(REPO, 'client', 'src-tauri', 'src', 'chrysalis', 'heartbeat.rs'), 'utf8')
  if (!/get\("message"\)/.test(hbrs)) {
    bad('the client no longer classifies by the `message` field — re-check invariant 9 before trusting it')
  } else {
    ok('the client still classifies refusals by the `message` field (invariant 9 is load-bearing)')
  }
  const rt = fs.readFileSync(
    path.join(REPO, 'client', 'src-tauri', 'src', 'chrysalis', 'runtime.rs'), 'utf8')
  if (!/contains\("suspended"\)/.test(rt)) {
    bad('is_suspension no longer tests for "suspended" — the frozen contract changed')
  } else {
    ok('is_suspension still tests for "suspended"')
  }
}

// ── 10. Each state change is ONE save (no half-applied override) ────────────
//
// `activated_at`, `force_state`, `evict_requested_at` and `evict_reason` are the
// same decision. Two `saveRecord` calls would be two chances to apply only one of
// them, and the states in between are exactly the desync this feature exists to
// remove: released but not evicted, or evicted but not released. The repo already
// paid for this once (`first_seen_at` is written in the same save as
// `activated_at` for that reason).
{
  const admin = codeOnly(read('admin_console.pb.js'))
  const check = (label, from, to, expect) => {
    const i = admin.indexOf(from)
    if (i < 0) {
      bad(`could not locate ${label} — no evidence, cannot pass`)
      return
    }
    const j = admin.indexOf(to, i)
    const block = j < 0 ? admin.slice(i) : admin.slice(i, j)
    const n = (block.match(/saveRecord\s*\(/g) || []).length
    if (n !== expect) {
      bad(`${label} performs ${n} saveRecord call(s); expected ${expect} so the change is atomic`)
    } else {
      ok(`${label} is a single save`)
    }
  }
  check('codes.release', 'action === "codes.release"', 'action === "codes.reactivate"', 1)
  // Scope to the handler's own body: from its `if (` to the closing brace at the
  // same indent. Slicing to the next banner comment ran through other handlers.
  {
    const i = admin.indexOf('action === "codes.reactivate"')
    const close = i < 0 ? -1 : admin.indexOf('\n        }', i)
    const block = i < 0 ? '' : admin.slice(i, close < 0 ? i + 4000 : close)
    const n = (block.match(/saveRecord\s*\(/g) || []).length
    if (!block) {
      bad('could not locate codes.reactivate — no evidence, cannot pass')
    } else if (n !== 1) {
      bad(`codes.reactivate performs ${n} saveRecord call(s); expected 1 so the change is atomic`)
    } else {
      ok('codes.reactivate is a single save')
    }
  }

  const act = codeOnly(read('activation.pb.js'))
  const i = act.indexOf('setIfPresent(rec, "evict_requested_at", null)')
  // The activation arm: find the enclosing saveRecord after the override clear.
  const after = act.slice(i, i + 1200)
  const n = (after.match(/saveRecord\s*\(/g) || []).length
  if (n !== 1) {
    bad(`the activation override-clearing arm has ${n} saveRecord call(s) after clearing the override; expected 1 (same save as activated_at)`)
  } else {
    ok('activation clears the override in the same save as activated_at')
  }
}


// ── 6. The actions BEHAVE: an override moves real state, and a direct release
//       still evicts ─────────────────────────────────────────────────────────
//
// Sections 1–5 read the source's shape. This one lifts the real `codes.override`
// and `codes.release` branches out of the hook and RUNS them against a stub dao,
// the same technique as `probe-hook-consistency.js`. Shape cannot tell whether
// `codes.override` actually changes the field `activation.pb.js` reads.
//
// Two properties are asserted, both of which a "record an intention" implementation
// would pass a source review while failing here:
//
//   * `codes.override {to_state:"in_term"}` CLEARS `released_at`, and the real
//     `codes.list` branch then reports released_pending:false. That is the hub
//     agreeing with the console, not a label written beside the state.
//   * A direct `codes.release` with no reason still sets `evict_requested_at`
//     (so the client is cut off) and leaves `evict_reason` EMPTY (so the student
//     gets the frozen generic sentence rather than an invented one).
{
  const admin = read('admin_console.pb.js')

  function grabFn(name) {
    const start = admin.indexOf(`function ${name}(`)
    if (start < 0) throw new Error(`no function ${name} in the hook`)
    const body = balanced(admin, start)
    const open = admin.indexOf('{', start)
    return admin.slice(start, open + body.length)
  }
  function grabAction(action) {
    const start = admin.indexOf(`if (action === "${action}"`)
    if (start < 0) throw new Error(`no branch for ${action}`)
    return balanced(admin, start)
  }

  // eslint-disable-next-line no-eval
  const parsePBDate = eval(`(${grabFn('parsePBDate')})`)

  class StubRecord {
    constructor(fields, kind) {
      this.fields = { ...fields }
      this.kind = kind || 'codes'
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

  // `codes.list` needs the SAME record object the action mutated, so the harness
  // can observe the state the hub would return afterwards.
  function harness(initialFields) {
    const row = new StubRecord(initialFields)
    const audit = []
    const codeSaves = []
    const dao = {
      findFirstRecordByData: (coll, _field, _value) => {
        if (coll === 'codes') return row
        throw new Error('not found')
      },
      findFirstRecordByFilter: () => {
        throw new Error('no rows in result set')
      },
      findCollectionByNameOrId: (name) => ({ name }),
      findRecordsByExpr: (name) => (name === 'codes' ? [row] : []),
      saveRecord: (rec) => {
        if (rec && rec.kind === 'code_overrides') audit.push(rec.fields)
        else if (rec && rec.kind === 'code_events') { /* best-effort mirror */ }
        else codeSaves.push(rec.fields)
      },
    }
    const $app = { dao: () => dao }
    const $dbx = { exp: () => ({}) }
    function Record(coll) {
      return new StubRecord({}, coll && coll.name === 'code_overrides' ? 'code_overrides' : 'code_events')
    }
    function run(action, body) {
      const branch = grabAction(action)
      let out = null
      const ok = (data) => {
        out = { ok: true, ...(data || {}) }
        return out
      }
      const bad = (status, message) => {
        out = { ok: false, status, message }
        return out
      }
      const nowISO = () => new Date().toISOString()
      const logEvent = () => {}
      // eslint-disable-next-line no-new-func
      const fn = new Function(
        'body', '$app', '$dbx', 'parsePBDate', 'ok', 'bad', 'Record',
        'grabFn', 'grabAction',
        `${grabFn('canonical')}\n${grabFn('setIfPresent')}\n${grabFn('livenessForConfirmation')}\n${grabFn('writeOverride')}\n${grabFn('findOverride')}\n${grabFn('nowISO')}\n${grabFn('logEvent')}\n${branch}`,
      )
      fn(body, $app, $dbx, parsePBDate, ok, bad, Record, grabFn, grabAction)
      return out
    }
    return { row, audit, codeSaves, run }
  }

  const NOW = Date.now()
  const iso = (ms) => new Date(ms).toISOString()
  const base = {
    code: 'RQ-ABCD-EFGH-JKMN-T',
    tier: 'strike',
    activated_at: iso(NOW - 5 * 86400000),
    released_at: iso(NOW - 60000),
    release_count: 1,
    expires_at: iso(NOW + 20 * 86400000),
    last_seen_at: iso(NOW - 60000),
    suspended: false,
    first_seen_at: iso(NOW - 5 * 86400000),
  }

  // A. sign-out recorded → in term clears the field activation.pb.js compares.
  {
    const h = harness(base)
    const out = h.run('codes.override', { code: base.code, to_state: 'in_term' })
    if (out && out.ok && h.row.get('released_at') === null) {
      ok('codes.override in_term clears released_at — the state activation reads')
    } else {
      bad(`codes.override in_term did not clear released_at (out=${JSON.stringify(out)}, released_at=${JSON.stringify(h.row.get('released_at'))})`)
    }
    if (h.codeSaves.length === 1) ok('codes.override in_term is a single record save')
    else bad(`codes.override in_term performed ${h.codeSaves.length} saves; expected 1`)
    if (h.audit.length === 1 && h.audit[0].aspect === 'in_term') {
      ok('codes.override in_term writes one code_overrides audit row')
    } else {
      bad(`codes.override in_term wrote ${h.audit.length} audit row(s); expected 1 with aspect in_term`)
    }

    const listed = h.run('codes.list', { query: base.code })
    const row = listed && listed.codes && listed.codes[0]
    if (row && row.released_pending === false && row.status === 'redeemed') {
      ok('after the override the hub lists the code as in term (released_pending:false)')
    } else {
      bad(`after the override the hub still lists ${JSON.stringify(row && { status: row.status, released_pending: row.released_pending })}`)
    }
  }

  // B. A code with no pending sign-out is a truthful no-op, not a write.
  {
    const h = harness({ ...base, released_at: null })
    const out = h.run('codes.override', { code: base.code, to_state: 'in_term' })
    if (out && out.ok && out.noop === true && h.codeSaves.length === 0 && h.audit.length === 0) {
      ok('codes.override on an in-term code is a no-op with no write and no audit row')
    } else {
      bad('codes.override on an in-term code was not a truthful no-op')
    }
  }

  // C. The release stays a DIRECT action that evicts, and invents no sentence.
  {
    const h = harness(base)
    const out = h.run('codes.release', { code: base.code })
    if (h.row.get('evict_requested_at')) {
      ok('a direct codes.release still sets evict_requested_at (the client is cut off)')
    } else {
      bad('a direct codes.release did not set evict_requested_at — the client would keep working')
    }
    if (h.row.get('force_state') === 'released') {
      ok('a direct codes.release sets force_state=released (the probe-readable flag)')
    } else {
      bad(`a direct codes.release did not set force_state (got ${JSON.stringify(h.row.get('force_state'))})`)
    }
    if (h.row.get('evict_reason') === '') {
      ok('a reasonless release leaves evict_reason empty, so the student gets the frozen generic sentence')
    } else {
      bad(`a reasonless release invented evict_reason ${JSON.stringify(h.row.get('evict_reason'))}`)
    }
    if (out && out.ok && h.codeSaves.length === 1) ok('a direct codes.release is a single record save')
    else bad(`a direct codes.release performed ${h.codeSaves.length} saves; expected 1`)
  }
}

console.log()
if (failed) {
  console.log(`override-consistency FAILED — ${failed} assertion(s)`)
  process.exit(1)
}
console.log('override-consistency OK')
