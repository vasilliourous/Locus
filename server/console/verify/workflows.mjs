// Exercise the mutating workflows end to end: does the action actually happen,
// does the screen reflect it, and does the URL survive it?
import { chromium } from 'playwright'

const BASE = 'http://localhost:5199/admin'
const b = await chromium.launch()
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } })
await ctx.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 'verify-token'))
const p = await ctx.newPage()

const results = []
const check = (n, ok, d = '') => {
  results.push({ n, ok, d })
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${n}${d ? ' — ' + d : ''}`)
}
p.on('pageerror', (e) => console.log('  !! pageerror:', e.message))

// ── Renew, via the inspector ───────────────────────────────────────────────
await p.goto(BASE + '/codes/RQ-BCDE-FGHJ-KMNP-2', { waitUntil: 'networkidle' })
await p.waitForTimeout(900)
const before = await p.locator('.ins-fact strong').first().innerText()
await p.getByRole('button', { name: 'Renew…' }).click()
await p.waitForTimeout(600)
check('Renew… opens the drawer', (await p.locator('.drawer').count()) === 1)

// The drawer must PREVIEW the arithmetic before committing — that is the whole
// safety argument for the action.
const drawerText = await p.locator('.drawer').innerText()
check('the drawer previews the new expiry', /→|becomes|will/i.test(drawerText), drawerText.replace(/\n/g, ' ').slice(0, 90))

// Fill a price (optional field) and submit.
const priceInput = p.locator('.drawer input').nth(1)
if (await priceInput.count()) await priceInput.fill('10.00').catch(() => {})
// The drawer's COMMIT button lives in its footer; the title also says "Renew".
await p.locator('.drawer .dw-foot button').filter({ hasText: /Renew/ }).first().click()
await p.waitForTimeout(1200)
const after = await p.locator('.ins-fact strong').first().innerText()
check('the renewal changed the expiry on screen', before !== after, `${before} → ${after}`)

// ── Save the name, and see it in the table ─────────────────────────────────
const uniqueName = 'QA ' + Date.now()
await p.locator('.inspector input').first().fill(uniqueName)
await p.waitForTimeout(200)
const saveBtn = p.getByRole('button', { name: /Save changes/ })
check('Save enables only when dirty', await saveBtn.isEnabled())
await saveBtn.click()
await p.waitForTimeout(1000)
const headerName = await p.locator('.ins-name').innerText()
check('the saved name appears in the inspector header', headerName.includes(uniqueName), headerName)

// ── Plan change ────────────────────────────────────────────────────────────
await p.locator('.inspector select').selectOption('free')
await p.waitForTimeout(200)
await p.getByRole('button', { name: /Save changes/ }).click()
await p.waitForTimeout(1000)
check('the plan change persists', (await p.locator('.inspector select').inputValue()) === 'free')

// ── Unsaved-changes guard ──────────────────────────────────────────────────
await p.locator('.inspector input').first().fill('This will be discarded')
await p.waitForTimeout(200)
// Click a different row: the guard must intervene rather than silently discard.
const rows = p.locator('table.data tbody tr')
const other = rows.nth(3)
const otherCode = (await other.locator('td').nth(1).innerText()).trim()
await other.click()
await p.waitForTimeout(500)
const sheet = await p.locator('.confirm-sheet').count()
check('leaving with unsaved edits asks first', sheet === 1, sheet ? 'sheet shown' : 'no sheet — edits discarded silently')
if (sheet) {
  await p.getByRole('button', { name: /Keep editing/ }).click()
  await p.waitForTimeout(400)
  const stillMine = await p.locator('.inspector input').first().inputValue()
  check('"Keep editing" keeps the edits and the code', stillMine.includes('discarded'), stillMine)
  // now discard for real
  await p.locator('table.data tbody tr').nth(3).click()
  await p.waitForTimeout(400)
  await p.getByRole('button', { name: /Discard and leave/ }).click()
  await p.waitForTimeout(800)
  check('"Discard and leave" moves to the chosen row', p.url().endsWith('/' + otherCode), `${otherCode} | ${p.url().split('/admin')[1]}`)
}

// ── Search + filter combination ────────────────────────────────────────────
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(500)
await p.locator('select[aria-label="Plan"]').selectOption('strike')
await p.waitForTimeout(700)
const rowsStrike = await p.locator('table.data tbody tr').count()
const plans = await p.locator('table.data tbody tr td:nth-child(4)').allTextContents().catch(() => [])
check('the plan filter narrows the table', rowsStrike > 0 && plans.every((x) => x.toLowerCase().includes('strike')), `${rowsStrike} rows, all strike=${plans.every((x) => x.toLowerCase().includes('strike'))}`)
check('the plan filter is in the URL', p.url().includes('tier=strike'), p.url().split('/admin')[1])

// ── Column chooser actually changes columns ────────────────────────────────
await p.locator('.cols-menu > summary').click()
await p.waitForTimeout(200)
const nameToggle = p.locator('.cols-row').filter({ hasText: 'Name' }).locator('input')
await nameToggle.uncheck()
await p.waitForTimeout(500)
const headersAfter = await p.locator('table.data thead th').allTextContents()
check('hiding a column removes it', !headersAfter.some((h) => h.trim() === 'Name ↑'), headersAfter.join('|'))
await nameToggle.check()
await p.waitForTimeout(400)

// ── Bulk delete bar appears on tick ────────────────────────────────────────
await p.goto(BASE + '/codes?status=available', { waitUntil: 'networkidle' })
await p.waitForTimeout(600)
const boxes = p.locator('table.data tbody input[type="checkbox"]')
const n = await boxes.count()
if (n > 0) {
  await boxes.first().check()
  await p.waitForTimeout(300)
  check('ticking a row reveals the bulk bar', (await p.locator('.bulkbar').count()) === 1)
  await p.locator('.bulkbar button').filter({ hasText: 'Clear' }).click()
} else {
  check('ticking a row reveals the bulk bar', false, 'no selectable rows')
}

// ── Create drawer ──────────────────────────────────────────────────────────
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(500)
await p.getByRole('button', { name: /Create codes/ }).click()
await p.waitForTimeout(500)
check('Create codes opens the mint drawer', (await p.locator('.drawer').count()) === 1)
const createTxt = await p.locator('.drawer').innerText()
check('the mint drawer offers the hub’s plans', createTxt.includes('strike') || createTxt.includes('free'), createTxt.replace(/\n/g, ' ').slice(0, 80))
await p.keyboard.press('Escape')

// ── Back button ────────────────────────────────────────────────────────────
await p.goto(BASE + '/', { waitUntil: 'networkidle' })
await p.waitForTimeout(400)
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(400)
await p.locator('table.data tbody tr').first().click()
await p.waitForTimeout(600)
await p.goBack()
await p.waitForTimeout(600)
check('Back returns from a code to the list', p.url().endsWith('/codes'), p.url().split('/admin')[1])

await b.close()
const bad = results.filter((r) => !r.ok)
console.log(`\n${results.length - bad.length}/${results.length} workflow checks passed`)
if (bad.length) process.exit(1)
