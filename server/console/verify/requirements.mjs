// The requirement register, driven in a real browser.
//
// R2 (probe), R3 (overrides move real state), R4 (no-op controls deleted) and
// R5/R6 (terminology, hierarchy, no clipped captions) are OBSERVATIONS, not
// source facts. This script performs each action and asserts what the screen
// shows afterwards. It talks to `mock-hub.mjs`, which mirrors the hook's actions
// and the probe's response shape — so it exercises the console's real paths, and
// the hook itself is covered by `check-consistency.sh` (§58, §66).
//
// Run against a FRESH mock (the mutations persist in the mock process):
//   node mock-hub.mjs &         # :8777
//   vite --config vite.verify.config.ts &   # :5199
//   node requirements.mjs

import { chromium } from 'playwright'
import fs from 'node:fs'

const BASE = process.env.BASE || 'http://localhost:5199/admin'
const SHOTS = new URL('./shots/', import.meta.url).pathname
fs.mkdirSync(SHOTS, { recursive: true })

const results = []
function check(name, pass, detail = '') {
  results.push({ name, pass, detail })
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`)
}

const browser = await chromium.launch()
const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
await ctx.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 'verify-token'))
const page = await ctx.newPage()
const consoleErrors = []
// Every admin action the page sends, so "the payload" can be checked as well as
// the DOM — R4 deletes a field, and a deleted input that is still transmitted is
// not deleted.
const sentActions = []
page.on('request', (r) => {
  if (r.url().endsWith('/api/admin/console') && r.method() === 'POST') {
    try {
      sentActions.push(JSON.parse(r.postData() || '{}'))
    } catch {
      /* a non-JSON body is not one of ours */
    }
  }
})
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text())
})
page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message))

async function open(code) {
  await page.goto(`${BASE}/codes/${code}`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  return page.locator('.inspector')
}

// ── R4: the controls that did nothing are GONE ─────────────────────────────
{
  const ins = await open('RQ-ABCD-EFGH-JKMN-T')
  check('R4: no “type the last four characters” input exists', (await ins.locator('input[placeholder*="Last four"]').count()) === 0)
  check('R4: no reason/“shown to the student” textarea exists', (await ins.locator('textarea[placeholder*="Write it for the student"]').count()) === 0)
  check('R4: no “Manual overrides” disclosure exists', (await ins.locator('summary', { hasText: 'Manual overrides' }).count()) === 0)
  check('R4: the release is a direct action (Release is present)', (await ins.getByRole('button', { name: 'Release', exact: true }).count()) === 1)
}

// ── R6b: Term and Expiry are top-level editable tiles ──────────────────────
{
  const ins = await open('RQ-ABCD-EFGH-JKMN-T')
  const tiles = ins.locator('.ins-fact-edit')
  check('R6b: exactly two editable fact tiles (Expires, Term)', (await tiles.count()) === 2)
  const firstText = (await tiles.nth(0).innerText()).toUpperCase()
  const secondText = (await tiles.nth(1).innerText()).toUpperCase()
  check('R6b: the first tile is Expires', firstText.includes('EXPIRES'), firstText.replace(/\n/g, ' '))
  check('R6b: the second tile is Term', secondText.includes('TERM'), secondText.replace(/\n/g, ' '))

  // One step from the panel: clicking Expires opens a date editor in place.
  await tiles.nth(0).click()
  await page.waitForTimeout(100)
  check('R6d: clicking Expires opens its editor in one step', (await ins.locator('input[type="date"]').count()) === 1)
  await ins.getByRole('button', { name: 'Cancel' }).click()

  // R5 (console half): an empty expiry reads "Never", not a countdown.
  const ins2 = await open('RQ-HJKM-NPQR-STVW-8')
  const expiresTile = (await ins2.locator('.ins-fact-edit').nth(0).innerText()).replace(/\n/g, ' ')
  check('R5: an empty expiry reads “Never” at the top level', /Never/.test(expiresTile), expiresTile)
}

// ── R6c: History and Record are at the bottom, as folds ────────────────────
{
  const ins = await open('RQ-ABCD-EFGH-JKMN-T')
  const folds = ins.locator('details.ins-fold')
  const n = await folds.count()
  check('R6c: History and Record are folds (2)', n === 2, `${n} folds`)
  const historyY = (await folds.nth(0).boundingBox()).y
  const recordY = (await folds.nth(1).boundingBox()).y
  const deleteY = (await ins.getByText('Delete permanently').boundingBox()).y
  check('R6c: both folds sit BELOW the delete action', historyY > deleteY && recordY > historyY, `${historyY}/${recordY}/${deleteY}`)
  const historySummary = (await folds.nth(0).locator('summary').innerText()).toUpperCase()
  check('R6c: the History fold names itself and its count', historySummary.includes('HISTORY') && /\d/.test(historySummary), historySummary.replace(/\n/g, ' '))
}

// ── R6a: no clipped caption in the table ──────────────────────────────────
{
  await page.goto(`${BASE}/codes`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  const clipped = await page.evaluate(() => {
    const out = []
    for (const el of document.querySelectorAll('table.codes-table .badge, table.codes-table .chip, table.codes-table .override-note')) {
      if (el.scrollWidth > el.clientWidth + 1) out.push(`${el.className}: "${el.textContent.trim()}" ${el.scrollWidth}>${el.clientWidth}`)
    }
    return out
  })
  check('R6a: no state badge or plan chip is ellipsised', clipped.length === 0, clipped.join('; '))
  await page.screenshot({ path: `${SHOTS}/req-codes.png` })
}

// ── R2: the probe runs on demand and its result is visible ────────────────
{
  const ins = await open('RQ-ABCD-EFGH-JKMN-T')
  const run = ins.getByRole('button', { name: 'Run session probe' })
  check('R2: a labelled probe control exists', (await run.count()) === 1)
  await run.click()
  await page.waitForTimeout(500)
  const text = (await ins.innerText()).replace(/\n/g, ' ')
  check('R2: the result says the hub answered', text.includes('Hub answered'), text.slice(0, 120))
  check('R2: the result names the verdict (reason_code)', /verdict:\s*\w+/.test(text), (text.match(/verdict:[^·]*/) || [''])[0])
  check('R2: the result shows a run timestamp', /ran .*\d{4}/.test(text), (text.match(/ran [^C]*/) || [''])[0])
  check('R2: a successful run shows no failure banner', (await ins.locator('.msg.err', { hasText: 'did not answer' }).count()) === 0)

  // D1: the run is legible from the MAIN VIEW — a persistent line under the page
  // title, with the counts, that survives switching codes.
  const lastRun = (await page.locator('.probe-last').innerText()).replace(/\s+/g, ' ')
  check('D1: the page header reports the last probe run', /Probe ran .*on RQ-/.test(lastRun), lastRun)
  check('D1: the last-run line carries checks/findings/changes/errors', /\d+ checks · \d+ findings? · 0 changes · no errors/.test(lastRun), lastRun)
  // Select a DIFFERENT row in the same SPA session (a reload would legitimately
  // reset the panel, and the point is that changing selection does not erase the
  // evidence).
  await page.locator('table.codes-table tbody tr', { hasText: 'RQ-BCDE-FGHJ-KMNP-2' }).first().click()
  await page.waitForTimeout(400)
  const stillThere = await page.locator('.probe-last').innerText()
  check('D1: the last-run line survives selecting another code', /RQ-ABCD-EFGH-JKMN-T/.test(stillThere), stillThere.replace(/\s+/g, ' '))
  await page.locator('table.codes-table tbody tr', { hasText: 'RQ-ABCD-EFGH-JKMN-T' }).first().click()
  await page.waitForTimeout(400)
  await page.screenshot({ path: `${SHOTS}/req-probe.png` })
}

// ── D1: a run that cannot happen is reported as a failure, not silence ────
{
  const ins = await open('RQ-EFGH-JKMN-PQRS-5') // never issued: no device id
  await ins.getByRole('button', { name: 'Run session probe' }).click()
  await page.waitForTimeout(300)
  const lastRun = (await page.locator('.probe-last').innerText()).replace(/\s+/g, ' ')
  check('D1: a probe that cannot run says so in the header', /and failed:/.test(lastRun), lastRun)
  check('D1: the failure explains why (no device to compare)', /device id/.test(lastRun), lastRun)
}

// ── R5: a real expiry, then cleared, both readable at the top level ───────
{
  const ins = await open('RQ-HJKM-NPQR-STVW-8')
  const tile = ins.locator('.ins-fact-edit').nth(0)
  await tile.click()
  await ins.locator('input[type="date"]').fill('2027-06-30')
  await ins.getByRole('button', { name: 'Set expiry' }).click()
  await page.waitForTimeout(500)
  const withDate = (await ins.locator('.ins-fact-edit').nth(0).innerText()).replace(/\n/g, ' ')
  check('R5: setting a date shows a date and a countdown', /\d{4}/.test(withDate) && /\dd/.test(withDate), withDate)

  await ins.locator('.ins-fact-edit').nth(0).click()
  await ins.locator('input[type="date"]').fill('')
  await ins.getByRole('button', { name: 'Set expiry' }).click()
  await page.waitForTimeout(500)
  const cleared = (await ins.locator('.ins-fact-edit').nth(0).innerText()).replace(/\n/g, ' ')
  check('R5: clearing the date returns to “Never” with no countdown', /Never/.test(cleared) && !/\dd/.test(cleared), cleared)
}

// ── R3: an override moves real state, and the next probe alerts ───────────
{
  const ins = await open('RQ-ABCD-EFGH-JKMN-T')
  const before = (await ins.locator('.badge').first().innerText()).trim()
  await ins.getByRole('button', { name: 'Release', exact: true }).click()
  await page.waitForTimeout(700)
  const afterBadge = (await ins.locator('.badge').first().innerText()).trim()
  check('R3: in term → released (the derived state changed)', before === 'in term' && afterBadge === 'released', `${before} → ${afterBadge}`)

  // R4 also applies to the PAYLOAD: no reason, no typed/claim token, no
  // preview id. A direct action sends the code and nothing else.
  const releaseBody = sentActions.filter((a) => a.action === 'codes.release').pop() || {}
  check('R4: the release payload carries no reason', !('reason' in releaseBody), JSON.stringify(releaseBody))
  check('R4: the release payload carries no claim token', !('claim_token' in releaseBody), JSON.stringify(releaseBody))
  check('R4: the release payload does not preview (no dry_run)', !('dry_run' in releaseBody), JSON.stringify(releaseBody))
  // The eviction has its own marker in the table row; the badge is not repeated
  // as a note in the panel.
  const conflict = await page
    .locator('table.codes-table tbody tr', { hasText: 'RQ-ABCD-EFGH-JKMN-T' })
    .locator('.conflict.evicted')
    .count()
  check('R3: the row is marked as a device being cut off', conflict >= 1, `${conflict} eviction marker(s)`)
  const noteCount = await ins.locator('.override-note').count()
  check('R6: the panel does not repeat the badge as a note', noteCount === 0, `${noteCount} note(s)`)

  await ins.getByRole('button', { name: 'Run session probe' }).click()
  await page.waitForTimeout(500)
  const probeText = (await ins.innerText()).replace(/\n/g, ' ')
  check('R3: the next probe run ALERTS on the override', probeText.includes('Override active'), (probeText.match(/Override active[^.]*/) || [''])[0])
  // I3: a released code was SOLD once, so the hub must not call it "never
  // activated", and the verdict must name the decision rather than "unknown".
  check('I3: the probe calls a released code "released", not "never-activated"', /May be used: released\b/.test(probeText), (probeText.match(/May be used:[^·]*/) || [''])[0])
  check('I3: the probe names the override as the verdict', /verdict:\s*released_by_override/.test(probeText), (probeText.match(/verdict:[^C]*/) || [''])[0])
  await page.screenshot({ path: `${SHOTS}/req-override-probe.png` })
}

// ── R3: unclaimed → in term, and the hub list agrees ──────────────────────
{
  const ins = await open('RQ-FGHJ-KMNP-QRST-6')
  const before = (await ins.locator('.badge').first().innerText()).trim()
  check('R3: the code starts unclaimed', before === 'unclaimed', before)
  await ins.getByRole('button', { name: 'Mark in term' }).click()
  await page.waitForTimeout(700)
  const after = (await ins.locator('.badge').first().innerText()).trim()
  check('R3: unclaimed → in term', after === 'in term', `${before} → ${after}`)

  // The LIST is a second reader of the same row; it must agree with the panel.
  await page.goto(`${BASE}/codes?q=FGHJ`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  const targetRow = page.locator('table.codes-table tbody tr', { hasText: 'RQ-FGHJ-KMNP-QRST-6' }).first()
  const rowText = (await targetRow.count()) ? await targetRow.innerText() : '(row not found)'
  check('R3: the list (the hub view) agrees — the code now reads in term', /in term/i.test(rowText), rowText.replace(/\n/g, ' ').slice(0, 90))
}

check('R2/R3: no application errors were logged', consoleErrors.length === 0, consoleErrors.join(' | '))

await browser.close()
const failed = results.filter((r) => !r.pass)
console.log(`\n${results.length - failed.length}/${results.length} checks passed`)
if (failed.length) {
  console.log('FAILURES:')
  for (const f of failed) console.log(` - ${f.name}${f.detail ? ' — ' + f.detail : ''}`)
  process.exit(1)
}
