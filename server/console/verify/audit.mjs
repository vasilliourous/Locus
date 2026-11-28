// Objective usability measurements: contrast, target sizes, focus visibility,
// and how much of the work is reachable without a mouse.
import { chromium } from 'playwright'

const BASE = 'http://localhost:5199/admin'
const b = await chromium.launch()
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } })
await ctx.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 'verify-token'))
const p = await ctx.newPage()

const out = []
const say = (s) => {
  out.push(s)
  console.log(s)
}

// ── Contrast, computed from the live DOM ───────────────────────────────────
// WCAG 2.1 relative luminance. 4.5:1 is AA for normal text; 3:1 for large text
// and UI boundaries.
function auditContrast(page) {
  return page.evaluate(() => {
    const lum = (c) => {
      const [r, g, b] = c.map((v) => {
        v /= 255
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
      })
      return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    const parse = (s) => {
      const m = s.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?\)/)
      if (!m) return null
      return { rgb: [+m[1], +m[2], +m[3]], a: m[4] === undefined ? 1 : +m[4] }
    }
    const bgOf = (el) => {
      let n = el
      while (n && n !== document.documentElement) {
        const cs = getComputedStyle(n)
        const c = parse(cs.backgroundColor)
        if (c && c.a > 0.5) return c.rgb
        n = n.parentElement
      }
      return [26, 27, 38]
    }
    const ratio = (bg, fg) => {
      const L1 = lum(bg),
        L2 = lum(fg)
      const a = Math.max(L1, L2),
        bb = Math.min(L1, L2)
      return (a + 0.05) / (bb + 0.05)
    }
    const bad = []
    const seen = new Set()
    for (const el of document.querySelectorAll('body *')) {
      const cs = getComputedStyle(el)
      if (cs.visibility === 'hidden' || cs.display === 'none' || +cs.opacity === 0) continue
      const txt = [...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent.trim()).join('')
      if (!txt) continue
      const fg = parse(cs.color)
      if (!fg) continue
      const r = ratio(bgOf(el), fg.rgb)
      const size = parseFloat(cs.fontSize)
      const large = size >= 18.66 || (size >= 14 && +cs.fontWeight >= 700)
      const need = large ? 3 : 4.5
      if (r < need) {
        const key = `${cs.color}|${txt.slice(0, 18)}`
        if (seen.has(key)) continue
        seen.add(key)
        bad.push({ text: txt.slice(0, 28), color: cs.color, size, ratio: +r.toFixed(2), need })
      }
    }
    return bad
  })
}

// ── Small targets ──────────────────────────────────────────────────────────
function auditTargets(page) {
  return page.evaluate(() => {
    const small = []
    for (const el of document.querySelectorAll('button, a, input[type="checkbox"], select, summary')) {
      const r = el.getBoundingClientRect()
      if (r.width === 0 || r.height === 0) continue
      // 24x24 is WCAG 2.2 AA (2.5.8); 32px is the comfortable minimum for a
      // mouse-and-touch tool. Report the ones below 24 as errors, 24-32 as tight.
      if (r.width < 24 || r.height < 24) {
        small.push({
          tag: el.tagName,
          label: (el.innerText || el.getAttribute('aria-label') || el.title || '').trim().slice(0, 24),
          w: Math.round(r.width),
          h: Math.round(r.height),
        })
      }
    }
    return small
  })
}

for (const [path, name] of [
  ['/', 'Today'],
  ['/codes', 'Codes'],
  ['/renewals', 'Renewals'],
  ['/distribution', 'Distribution'],
  ['/fleet/tiers', 'Tiers'],
  ['/fleet/releases', 'Releases'],
  ['/fleet/maintenance', 'Maintenance'],
]) {
  await p.goto(BASE + path, { waitUntil: 'networkidle' })
  await p.waitForTimeout(500)
  const bad = await auditContrast(p)
  say(`\n── ${name} (${path})`)
  if (!bad.length) say('   contrast: every text node meets WCAG AA')
  else for (const x of bad) say(`   contrast FAIL ${x.ratio}:1 (needs ${x.need}) "${x.text}" ${x.color} ${x.size}px`)

  const small = await auditTargets(p)
  if (!small.length) say('   targets: every control is at least 24x24')
  else for (const s of small.slice(0, 8)) say(`   target   ${s.w}x${s.h} ${s.tag} "${s.label}"`)
}

// ── Keyboard reachability of the main job ──────────────────────────────────
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(500)
await p.locator('body').click({ position: { x: 700, y: 600 } })
const steps = []
await p.keyboard.press('j')
await p.waitForTimeout(400)
steps.push('j → ' + (p.url().includes('/codes/RQ-') ? 'row selected' : 'FAILED'))
await p.keyboard.press('Enter')
await p.waitForTimeout(500)
steps.push('Enter → ' + ((await p.locator('.drawer').count()) ? 'renew drawer' : 'FAILED'))
await p.keyboard.press('Escape')
await p.waitForTimeout(300)
await p.keyboard.press('?')
await p.waitForTimeout(300)
steps.push('? → ' + ((await p.locator('.help-rail').count()) ? 'help' : 'FAILED'))
say('\n── Keyboard, no mouse: ' + steps.join(' · '))

// ── Tab order: is the first Tab target the skip/nav, and is focus visible? ──
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(400)
const focusRing = []
for (let i = 0; i < 6; i++) {
  await p.keyboard.press('Tab')
  const info = await p.evaluate(() => {
    const a = document.activeElement
    if (!a || a === document.body) return null
    const cs = getComputedStyle(a)
    return {
      tag: a.tagName,
      label: (a.innerText || a.getAttribute('aria-label') || a.placeholder || '').trim().slice(0, 22),
      outline: cs.outlineStyle !== 'none' && cs.outlineWidth !== '0px',
      ring: cs.boxShadow !== 'none',
    }
  })
  if (info) focusRing.push(info)
}
say('\n── Focus (first 6 Tab stops)')
for (const f of focusRing) say(`   ${f.outline || f.ring ? 'visible' : 'NO RING'}  ${f.tag} "${f.label}"`)

// ── How much of the page's information is above the fold at 900px ──────────
await p.goto(BASE + '/codes', { waitUntil: 'networkidle' })
await p.waitForTimeout(500)
const fold = await p.evaluate(() => {
  const vh = innerHeight
  const rows = [...document.querySelectorAll('table.data tbody tr')]
  const visible = rows.filter((r) => r.getBoundingClientRect().top < vh).length
  return { rows: rows.length, visible, vh }
})
say(`\n── Above the fold at ${fold.vh}px: ${fold.visible} of ${fold.rows} rows visible`)

await b.close()
