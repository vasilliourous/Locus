/**
 * The two client screens R3 and R5 are stated against, rendered for real.
 *
 * # Why this exists
 *
 * R3's acceptance is "the client app leaves its normal page for the stopped
 * page", and R5's is "the client shows no countdown" when the hub says there is
 * no expiry. Both are statements about what the STUDENT reads, and neither is
 * provable from the Rust layer alone: the Rust tests prove the status the gate
 * receives, and this proves what the screen does with it.
 *
 * It mounts the SHIPPED components — `SuspendedScreen` as the gate renders it,
 * and `LayoutSubscription` with only its data source swapped (the real hook needs
 * the Tauri IPC, which a browser does not have) — inside the real theme and the
 * real locale bundles. That is the same trade `verify.mjs` makes for the marks: a
 * mock of the thing under test would reproduce the defect rather than reveal it.
 *
 * It does NOT prove the gate chose this screen. That decision is `gateScreen`,
 * unit-tested in `src/hooks/use-entitlement.test.ts`, and the hub bytes that make
 * `suspended` true are pinned by
 * `src-tauri/tests/eviction_contract.rs`. This is the last link in that chain.
 *
 *     pnpm exec vite --config harness.vite.config.mts --port 3100 &   # client/
 *     node .harness/verify-states.mjs
 */
import { chromium } from 'playwright'

const BASE = 'http://localhost:3100/index.html'
const OUT = new URL('./shots/', import.meta.url).pathname
const failures = []
const check = (ok, message) => {
  if (!ok) failures.push(message)
}

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 560, height: 560 } })
const errors = []
page.on('pageerror', (e) => errors.push('pageerror: ' + e.message))

const textOf = async (query) => {
  await page.goto(BASE + query, { waitUntil: 'networkidle' })
  // The locale bundles load by dynamic import after mount; the first paint is the
  // key names, not the sentences.
  await page.waitForTimeout(900)
  return (await page.locator('body').innerText()).replace(/\s+/g, ' ')
}

// ── R3: the stopped page, showing the hub's own sentence ────────────────────
{
  const text = await textOf('?which=suspended')
  const gate = await page.locator('[data-gate]').getAttribute('data-gate')
  check(
    gate === 'suspended',
    `the shipped gate chose "${gate}" for a suspended device, not the stopped page`,
  )
  check(
    /Account suspended — this code was released/.test(text),
    `the sticky sentence the hub sends is not on the screen: ${text.slice(0, 160)}`,
  )
  check(
    !/In use|days? left in the term/.test(text),
    'the normal connection page still rendered — the gate did not leave it',
  )
  // I3: the device is not signed out — it keeps its code — so the title must not
  // say so. It said "This device has been signed out" above a body explaining a
  // suspension, which is a label contradicting its own screen.
  check(
    !/signed out/i.test(text),
    `the stopped page claims the device was signed out: ${text.slice(0, 120)}`,
  )
  check(
    /access to this device is paused/i.test(text),
    `the stopped page does not state what happened: ${text.slice(0, 120)}`,
  )
  // The one control the screen must NOT offer: a code prompt invites a student to
  // re-enter a code the hub already holds and already refuses.
  const inputs = await page.locator('input, textarea, button').count()
  check(
    inputs === 0,
    `the stopped page offers ${inputs} control(s); it must offer none`,
  )
  await page.screenshot({ path: `${OUT}state-suspended.png` })
}

// ── R5: no expiry reads as "No expiry", a real one counts down ──────────────
{
  const never = await textOf('?which=subscription&state=never')
  check(
    /No expiry/.test(never),
    `a hub-confirmed absence does not read "No expiry": ${never.slice(0, 160)}`,
  )
  check(
    !/Expires in/.test(never),
    `"no expiry" still counts down: ${never.slice(0, 160)}`,
  )

  const active = await textOf('?which=subscription&state=active&days=30')
  check(
    /Expires in 30 days/.test(active),
    `a real expiry does not count down: ${active.slice(0, 160)}`,
  )

  await page.goto(BASE + '?which=subscription&state=never', {
    waitUntil: 'networkidle',
  })
  await page.waitForTimeout(900)
  await page.screenshot({ path: `${OUT}state-subscription-never.png` })
}

check(
  errors.length === 0,
  `the harness logged ${errors.length} page error(s): ${errors.join(' | ')}`,
)

await browser.close()

if (failures.length) {
  console.log(`${failures.length} state check(s) FAILED`)
  for (const f of failures) console.log(` - ${f}`)
  process.exit(1)
}
console.log(
  'state screens OK — suspended page shows the hub sentence; no-expiry does not count down',
)
