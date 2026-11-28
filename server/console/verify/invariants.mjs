// I1–I5, driven in the real console against the stand-in hub.
//
// These are the invariants that replaced "breathing room and versatility", which
// nothing could check. Each one either passes or fails here; the enumerations
// that back them (every control, every label, every container) are in the report,
// and this file is the part a machine can refuse.
//
// It uses `mock-hub.mjs`, which mirrors the hook's actions and response shapes.
// The HOOK's own behaviour is covered by `check-consistency.sh` §66/§67; this
// covers the binding between a control and a state change.
//
// Restart `mock-hub.mjs` before running: several checks mutate the fixture.

import { chromium } from 'playwright'
import fs from 'node:fs'

const BASE = process.env.BASE || 'http://localhost:5199/admin'
const SHOTS = new URL('./shots/', import.meta.url).pathname
fs.mkdirSync(SHOTS, { recursive: true })

const results = []
const check = (name, pass, detail = '') => {
  results.push({ name, pass, detail })
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`)
}

const browser = await chromium.launch()
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } })
await ctx.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 'verify-token'))
const page = await ctx.newPage()
const consoleErrors = []
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text())
})
page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message))

const openInspector = async (code) => {
  await page.goto(`${BASE}/codes/${code}`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  return page.locator('.inspector')
}

const badgeOf = async (ins) => (await ins.locator('.badge').first().innerText()).trim()

// ── I4: nothing operator-facing is clipped at the default window ───────────
//
// Measured, not eyeballed: every element that renders its own text (no element
// children) is compared with its own scroll box. This is the check that would
// have caught "sign-out re…" and "released · cutting off device" — both were
// inside elements narrower than their text.
//
// The two free-text TABLE columns are reported separately rather than exempted:
// they hold operator-entered data (a student's name, a middleman), they ellipsise
// only when the data exceeds the column, and they carry the full value in a
// `title`. Everything the UI itself composes must fit.
{
  const ins = await openInspector('RQ-ABCD-EFGH-JKMN-T')
  const clipped = await page.evaluate(() => {
    const out = []
    const root = document.querySelector('.codes-layout')
    if (!root) return ['(no .codes-layout)']
    for (const el of root.querySelectorAll('*')) {
      if (el.children.length > 0) continue
      const text = (el.textContent || '').trim()
      if (!text) continue
      const style = getComputedStyle(el)
      if (style.display === 'none' || style.visibility === 'hidden') continue
      // A control that owns its text is measured the same way; a badge inside a
      // button would be an element-child and is skipped above.
      if (el.scrollWidth > el.clientWidth + 1 && style.overflow !== 'visible') {
        out.push(`${el.tagName.toLowerCase()}.${el.className || '?'} "${text.slice(0, 48)}" ${el.scrollWidth}>${el.clientWidth}`)
      }
    }
    return out
  })
  check('I4: no UI-composed string in the Codes view is clipped', clipped.length === 0, clipped.join(' | '))

  // The two free-text columns, measured directly: they must WRAP, never ellipsise.
  const ellipsised = await page.evaluate(() => {
    const out = []
    for (const td of document.querySelectorAll('table.codes-table td.wrapcell')) {
      if (td.scrollWidth > td.clientWidth + 1) out.push(`${td.textContent.trim().slice(0, 32)} ${td.scrollWidth}>${td.clientWidth}`)
      if (getComputedStyle(td).textOverflow === 'ellipsis') out.push(`ellipsis on "${td.textContent.trim().slice(0, 24)}"`)
    }
    return out
  })
  check('I4: the name and middleman columns wrap instead of ellipsising', ellipsised.length === 0, ellipsised.join(' | '))
  await page.screenshot({ path: `${SHOTS}/inv-codes-1440.png` })
  void ins
}

// ── I1: every property reachable in ≤ 2 actions from the list ─────────────
//
// One click selects the row; the second clicks the property's own tile/button.
// No fold has to be opened first — the tiles are not inside a `<details>`.
{
  await page.goto(`${BASE}/codes`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  await page.locator('table.codes-table tbody tr').first().click() // action 1
  await page.waitForTimeout(300)
  const ins = page.locator('.inspector')

  const tiles = ins.locator('.ins-fact-edit')
  const tileCount = await tiles.count()
  const hiddenUnderFold = await page.evaluate(() => {
    for (const el of document.querySelectorAll('.ins-fact-edit')) {
      if (el.closest('details:not([open])')) return true
    }
    return false
  })
  check('I1: Expires and Term are two visible tiles, not nested in a fold', tileCount === 2 && !hiddenUnderFold, `${tileCount} tiles`)

  await tiles.nth(0).click() // action 2
  await page.waitForTimeout(150)
  check('I1: Expiry is editable two actions from the list', (await ins.locator('input[type="date"]').count()) === 1)
  await ins.getByRole('button', { name: 'Cancel' }).click()

  await tiles.nth(1).click() // action 2
  await page.waitForTimeout(150)
  check('I1: Term is editable two actions from the list', (await ins.locator('input[type="number"]').count()) === 1)
  await ins.getByRole('button', { name: 'Cancel' }).click()

  check(
    'I1: state is settable two actions from the list',
    (await ins.getByRole('button', { name: 'Release' }).count()) === 1 ||
      (await ins.getByRole('button', { name: 'Mark in term' }).count()) === 1 ||
      (await ins.getByRole('button', { name: 'Stop access…' }).count()) === 1,
  )
}

// ── I3 (second pass): the panel's verdict must agree with its own badge ────
{
  const unclaimed = await openInspector('RQ-FGHJ-KMNP-QRST-6')
  const text = (await unclaimed.innerText()).replace(/\s+/g, ' ')
  check(
    'I3: an unclaimed code is not described as "In use"',
    /sign-out is recorded and not claimed/i.test(text) && !/^In use/m.test(text) && !/In use\./.test(text),
    text.slice(0, 180),
  )
}

// ── I2 (UI binding): every state action the panel offers changes the badge ─
//
// The full 36-cell matrix is driven against the HOOK by
// `server/scripts/state-matrix.js` (§67). This is the binding: the button exists
// where it should, and pressing it moves the badge to the state its label names.
{
  const ins = await openInspector('RQ-FGHJ-KMNP-QRST-6') // unclaimed
  check('I2: an unclaimed code offers Mark in term', (await ins.getByRole('button', { name: 'Mark in term' }).count()) === 1)
  await ins.getByRole('button', { name: 'Mark in term' }).click()
  await page.waitForTimeout(700)
  check('I2: Mark in term moves unclaimed → in term', (await badgeOf(ins)) === 'in term', await badgeOf(ins))
  check('I2: an in-term code no longer offers Mark in term (no inert control)', (await ins.getByRole('button', { name: 'Mark in term' }).count()) === 0)

  // released → in term is the transition that had NO path before this pass.
  await ins.getByRole('button', { name: 'Release' }).click()
  await page.waitForTimeout(700)
  check('I2: Release moves in term → released', (await badgeOf(ins)) === 'released', await badgeOf(ins))
  const releasedOffersUndo = await ins.getByRole('button', { name: 'Mark in term' }).count()
  check('I2: a released code offers Mark in term — the way back', releasedOffersUndo === 1)
  await ins.getByRole('button', { name: 'Mark in term' }).click()
  await page.waitForTimeout(700)
  check('I2: Mark in term moves released → in term', (await badgeOf(ins)) === 'in term', await badgeOf(ins))
  await page.screenshot({ path: `${SHOTS}/inv-state-roundtrip.png` })
}

// ── I5: no inert controls ─────────────────────────────────────────────────
{
  // Delete is disabled on EITHER a live claim or a first-issue record, because
  // the hub refuses both. The old control checked only the claim, so it was
  // enabled and could never work on a released code.
  const issued = await openInspector('RQ-DEFG-HJKM-NPQR-4') // lapsed, issued
  const issuedDelete = issued.getByRole('button', { name: 'Delete…' })
  check(
    'I5: Delete is disabled for a code that has been issued (the hub would refuse it)',
    (await issuedDelete.count()) === 1 && (await issuedDelete.isDisabled()),
  )

  const fresh = await openInspector('RQ-EFGH-JKMN-PQRS-5') // never issued
  const freshDelete = fresh.getByRole('button', { name: 'Delete…' })
  check('I5: Delete is enabled for a never-issued code', (await freshDelete.count()) === 1 && !(await freshDelete.isDisabled()))

  // The bulk selector must not offer rows the batch refuses. Make a released
  // row exist, then look at it in the list.
  const releasable = await openInspector('RQ-ABCD-EFGH-JKMN-T')
  await releasable.getByRole('button', { name: 'Release' }).click()
  await page.waitForTimeout(700)
  await page.goto(`${BASE}/codes`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  const releasedRow = page.locator('table.codes-table tbody tr', { hasText: 'RQ-ABCD-EFGH-JKMN-T' }).first()
  const rowText = (await releasedRow.innerText()).replace(/\s+/g, ' ')
  const boxDisabled = await releasedRow.locator('input[type="checkbox"]').isDisabled()
  check(
    'I5: a released row is badged "released" and its bulk checkbox is refused',
    /released/i.test(rowText) && boxDisabled,
    `${rowText.slice(0, 60)} disabled=${boxDisabled}`,
  )
}

// ── I3: the labels that were fixed say what they do ───────────────────────
{
  await page.goto(`${BASE}/codes`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  const stateFilter = await page.locator('select[aria-label="State"]').innerText()
  check('I3: the available bucket is no longer called "Unused"', /Available/.test(stateFilter) && !/Unused/.test(stateFilter), stateFilter.replace(/\n/g, ' '))

  // A code that is in term at this point in the run (RQ-ABCD was released by the
  // I5 block above; RQ-BCDE was not touched).
  await page.goto(`${BASE}/codes/RQ-BCDE-FGHJ-KMNP-2`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(300)
  const ins = page.locator('.inspector')
  const hint = (await ins.innerText()).replace(/\s+/g, ' ')
  check('I3: Release states that the badge will read "released", not "unused"', /badge then reads “released”, not “unused”/.test(hint), hint.slice(0, 200))

  const today = await (async () => {
    await page.goto(`${BASE}/`, { waitUntil: 'networkidle' })
    await page.waitForTimeout(300)
    return (await page.locator('.stat').nth(2).innerText()).replace(/\s+/g, ' ')
  })()
  check('I3: the Today card no longer claims every free code is "not yet sold"', !/not yet sold/.test(today) && /free to sell/.test(today), today)
}

check('I1–I5: no application errors were logged', consoleErrors.length === 0, consoleErrors.join(' | '))

await browser.close()
const failed = results.filter((r) => !r.pass)
console.log(`\n${results.length - failed.length}/${results.length} invariant checks passed`)
if (failed.length) {
  console.log('FAILURES:')
  for (const f of failed) console.log(` - ${f.name}${f.detail ? ' — ' + f.detail : ''}`)
  process.exit(1)
}
