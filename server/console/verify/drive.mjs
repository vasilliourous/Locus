// Drive the real console in Chromium and report what a user would hit.
//
// This is a VERIFICATION harness, not a test suite: it navigates the ways an
// operator navigates, acts the ways they act, and then checks the properties that
// make an interface usable (is anything clipped, does a click land where it
// should, can the keyboard reach it, does the layout survive a small screen). It
// fails loudly rather than collecting results quietly, because a green run that
// checked nothing reads exactly like one that checked everything.

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
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await ctx.newPage()

// Surface anything the app logs as an error: a Vue warning or a thrown handler is
// a defect even if the pixel still looks right.
const consoleErrors = []
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text())
})
page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message))

// ── Sign in ────────────────────────────────────────────────────────────────
// The token is in sessionStorage; seeding it skips the login form so each run
// starts at the same place. The login form itself is verified separately.
async function seedToken(p) {
  await p.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 'verify-token'))
}

// ── 1. Login screen ────────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/', { waitUntil: 'networkidle' })
  const hasTokenField = await p.locator('input[type="password"]').count()
  check('login screen asks for the admin token', hasTokenField === 1)
  await p.screenshot({ path: `${SHOTS}/01-login.png` })
  await p.close()
}

await seedToken(ctx)

// ── 2. Today ───────────────────────────────────────────────────────────────
await page.goto(BASE + '/', { waitUntil: 'networkidle' })
await page.waitForTimeout(400)

const statCards = await page.locator('.stat').count()
check('Today shows the five number cards', statCards === 5, `${statCards} found`)
const queueRows = await page.locator('.table-wrap table tbody tr').count()
check('Today shows the renewal queue as rows', queueRows > 0, `${queueRows} rows`)

// Every number is a link to the list it counts: the claim the whole page rests on.
const firstStat = page.locator('.stat').first()
const label = (await firstStat.locator('.l').textContent())?.trim()
await firstStat.click()
await page.waitForTimeout(500)
const landed = new URL(page.url()).search
check('a number card navigates to a filtered list', landed.includes('status='), `${label} → ${landed}`)

await page.goto(BASE + '/', { waitUntil: 'networkidle' })
await page.waitForTimeout(300)
await page.screenshot({ path: `${SHOTS}/02-today.png`, fullPage: true })

// ── 3. Codes table ─────────────────────────────────────────────────────────
await page.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await page.waitForTimeout(400)

const rows = page.locator('table.data tbody tr')
const rowCount = await rows.count()
check('Codes lists the hub’s codes', rowCount === 15, `${rowCount} rows`)

// The visible columns a table is judged on.
const headers = await page.locator('table.data thead th').allTextContents()
const wanted = ['Code', 'Name', 'Plan', 'State', 'Last seen', 'Middleman', 'Days left']
for (const w of wanted) {
  check(`table has a "${w}" column`, headers.some((h) => h.includes(w)), headers.join(' | '))
}

// No horizontal clipping: a table you must scroll sideways at 1440px is a defect.
const metrics = await page.evaluate(() => {
  const el = document.querySelector('.table-scroll')
  if (!el) return null
  return { scrollW: el.scrollWidth, clientW: el.clientWidth, scrollH: el.scrollHeight, clientH: el.clientHeight }
})
if (metrics) {
  check('table needs no horizontal scroll at 1440px', metrics.scrollW <= metrics.clientW + 2, `${metrics.scrollW} vs ${metrics.clientW}`)
}

await page.screenshot({ path: `${SHOTS}/03-codes.png` })

// ── 4. Open a code: the inspector ──────────────────────────────────────────
await page.locator('table.data tbody tr', { hasText: 'Tom' }).first().click()
await page.waitForTimeout(900)
const inspectorVisible = await page.locator('.inspector:not(.inspector-idle)').count()
check('clicking a row opens the inspector', inspectorVisible === 1)
check('the open code is in the URL', /\/codes\/RQ-[A-Z0-9-]+$/.test(page.url()), page.url())

// The list must still be visible beside it — the point of a rail, not a modal.
const listStillThere = await page.locator('table.data tbody tr').count()
check('the list stays visible beside the inspector', listStillThere === 15, `${listStillThere} rows still rendered`)

// The facts the brief named: name, plan, middleman, history all reachable at once.
const insText = await page.locator('.inspector').innerText()
for (const [what, needle] of [
  ['the name (label)', 'Tom'],
  ['the plan', 'strike'],
  ['the middleman', 'Sarah'],
  ['the history, inline', 'renewed'],
  ['the renewal action', 'Renew'],
  ['the stop action', 'Stop access'],
]) {
  check(`inspector shows ${what} without another click`, insText.includes(needle))
}
await page.screenshot({ path: `${SHOTS}/04-inspector.png` })

// ── 5. Direct address ──────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(`${BASE}/codes/RQ-BCDE-FGHJ-KMNP-2`, { waitUntil: 'networkidle' })
  await p.waitForTimeout(800)
  const t = await p.locator('.inspector').innerText().catch(() => '')
  check('a deep link opens that code directly', t.includes('Priya'), t.slice(0, 60).replace(/\n/g, ' '))
  await p.screenshot({ path: `${SHOTS}/05-deeplink.png` })
  await p.close()
}

// ── 6. Filters survive a reload ────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(`${BASE}/codes?status=expired`, { waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const before = await p.locator('table.data tbody tr').count()
  await p.reload({ waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const after = await p.locator('table.data tbody tr').count()
  const sel = await p.locator('select[aria-label="State"]').inputValue().catch(() => '')
  check('a filter survives F5', before === after && sel === 'expired', `${before}→${after} rows, select="${sel}"`)
  await p.close()
}

// ── 7. Search by name ──────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
  await p.locator('input[type="search"]').fill('Priya')
  await p.waitForTimeout(700)
  const n = await p.locator('table.data tbody tr').count()
  const txt = await p.locator('table.data tbody').innerText().catch(() => '')
  check('searching a student’s name finds their code', n === 1 && txt.includes('Priya'), `${n} rows`)
  check('search is in the URL', page.url().length > 0 && (await p.url()).includes('q='), await p.url())
  await p.close()
}

// ── 8. Keyboard ────────────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
  await p.waitForTimeout(400)
  await p.locator('body').click({ position: { x: 700, y: 500 } })
  await p.keyboard.press('j')
  await p.waitForTimeout(500)
  const afterJ = p.url()
  check('j selects the first row', /\/codes\/RQ-/.test(afterJ), afterJ.split('/admin')[1])
  await p.keyboard.press('j')
  await p.waitForTimeout(500)
  check('j again moves to the next row', p.url() !== afterJ, p.url().split('/admin')[1])
  await p.keyboard.press('Escape')
  await p.waitForTimeout(400)
  check('Escape closes the inspector', p.url().endsWith('/codes'), p.url().split('/admin')[1])
  await p.close()
}

// ── 9. The palette ─────────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/', { waitUntil: 'networkidle' })
  await p.waitForTimeout(300)
  await p.keyboard.press('Control+k')
  await p.waitForTimeout(300)
  const open = await p.locator('.palette').count()
  check('Ctrl-K opens the palette', open === 1)
  await p.locator('.palette input').fill('Priya')
  await p.waitForTimeout(600)
  const items = await p.locator('.palette .item').count()
  check('the palette finds a code by name', items > 0, `${items} results`)
  await p.screenshot({ path: `${SHOTS}/06-palette.png` })
  await p.keyboard.press('Enter')
  await p.waitForTimeout(700)
  const insp = await p.locator('.inspector').innerText().catch(() => '')
  check('picking a palette result opens that code, not a list', insp.includes('Priya'), p.url().split('/admin')[1])
  await p.close()
}

// ── 10. Renewals board ─────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/renewals', { waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const lanes = await p.locator('.lane').count()
  check('the board has a lane per urgency bucket', lanes === 6, `${lanes} lanes`)
  // The board must account for EVERY code the hub holds. It used to fetch a
  // 365-day window, which excludes lapsed/suspended/no-expiry rows, and then
  // rendered lanes for exactly those — so two lanes were permanently empty and
  // the page under-reported the book. This assertion is what would have caught it.
  const counts = await p.locator('.lane-count').allTextContents()
  const cards = await p.locator('.card-code').count()
  const total = counts.reduce((n, c) => n + Number(c.trim() || 0), 0)
  check('the lane counts sum to every code the hub holds', total === 15 && cards === 15, `${counts.join('+')} = ${total}, ${cards} cards`)
  await p.screenshot({ path: `${SHOTS}/07-renewals.png` })
  await p.close()
}

// ── 11. Distribution, Tiers, Releases, Maintenance ─────────────────────────
for (const [path, needle, name] of [
  ['/distribution', 'Sarah', 'Distribution lists middlemen'],
  ['/fleet/tiers', '8445', 'Tiers shows the server config'],
  ['/fleet/releases', '2.2.0', 'Releases shows the offered version'],
  ['/fleet/maintenance', 'Import', 'Maintenance offers the recovery actions'],
]) {
  const p = await ctx.newPage()
  await p.goto(BASE + path, { waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const t = await p.locator('main').innerText().catch(() => '')
  check(name, t.includes(needle), t.slice(0, 50).replace(/\n/g, ' '))
  await p.screenshot({ path: `${SHOTS}/08${path.replace(/\//g, '-')}.png` })
  await p.close()
}

// ── 12. The help rail ──────────────────────────────────────────────────────
{
  const p = await ctx.newPage()
  await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
  await p.waitForTimeout(300)
  await p.locator('body').click({ position: { x: 700, y: 600 } })
  await p.keyboard.press('?')
  await p.waitForTimeout(300)
  const rail = await p.locator('.help-rail').count()
  const t = await p.locator('.help-rail').innerText().catch(() => '')
  check('? opens the help rail', rail === 1)
  check('the help rail explains this screen', t.includes('LIVENESS') || t.includes('Last seen'))
  await p.screenshot({ path: `${SHOTS}/09-help.png` })
  await p.close()
}

// ── 13. Narrow viewport (the phone case) ───────────────────────────────────
{
  const p = await ctx.newPage()
  await p.setViewportSize({ width: 390, height: 844 })
  await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const toggle = await p.locator('.nav-toggle').isVisible()
  check('a menu button exists on a phone-width screen', toggle)
  const overflow = await p.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
  check('nothing overflows the page horizontally on a phone', overflow <= 2, `${overflow}px over`)
  await p.screenshot({ path: `${SHOTS}/10-mobile-codes.png` })
  await p.locator('.nav-toggle').click()
  await p.waitForTimeout(300)
  check('the menu opens the sidebar drawer', await p.locator('.sidebar.open').isVisible())
  await p.screenshot({ path: `${SHOTS}/11-mobile-nav.png` })
  await p.close()
}

// ── 14. Console cleanliness ────────────────────────────────────────────────
check('no application errors were logged', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '))

await browser.close()

// ── Verdict ────────────────────────────────────────────────────────────────
const failed = results.filter((r) => !r.pass)
console.log(`\n${results.length - failed.length}/${results.length} checks passed`)
if (failed.length) {
  console.log('\nFAILURES:')
  for (const f of failed) console.log(' -', f.name, f.detail ? `(${f.detail})` : '')
  process.exit(1)
}
