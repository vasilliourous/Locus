// Extract the layout facts a reviewer would judge by eye: is the vertical rhythm
// consistent, is anything misaligned or overflowing, is the density right, are
// there orphaned elements. Numbers, not opinions.
import { chromium } from 'playwright'

const BASE = 'http://localhost:5199/admin'
const b = await chromium.launch()
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } })
await ctx.addInitScript(() => sessionStorage.setItem('chrysalis_admin_token', 't'))
const p = await ctx.newPage()

for (const [path, name] of [
  ['/', 'Today'],
  ['/codes', 'Codes'],
  ['/renewals', 'Renewals'],
  ['/distribution', 'Distribution'],
]) {
  await p.goto(BASE + path, { waitUntil: 'networkidle' })
  await p.waitForTimeout(600)
  const facts = await p.evaluate(() => {
    const main = document.querySelector('main')
    const vw = innerWidth, vh = innerHeight
    const out = { overflow: [], misaligned: [], tinyText: [], gaps: [], clipped: [] }

    // Anything wider than its parent / the viewport.
    for (const el of document.querySelectorAll('main *')) {
      const r = el.getBoundingClientRect()
      if (r.width === 0) continue
      // A child of a deliberately-scrolling container (the board) is ALLOWED to
      // extend past the viewport — that is what its scrollbar is for. Only count
      // an element that escapes the PAGE.
      const scroller = el.closest('.board, .table-scroll')
      if (r.right > vw + 1 && !scroller)
        out.overflow.push({ cls: el.className.toString().slice(0, 30), tag: el.tagName, right: Math.round(r.right), vw })
    }

    // Elements whose text is clipped without an ellipsis or a title.
    for (const el of document.querySelectorAll('main td, main .ins-fact strong, main .lane-title')) {
      // A cell with NO TEXT cannot clip anything meaningful — the checkbox and
      // verbs columns report a scrollWidth from their controls, not their text.
      if (!el.innerText.trim()) continue
      if (el.scrollWidth > el.clientWidth + 2 && getComputedStyle(el).textOverflow !== 'ellipsis' && !el.title)
        out.clipped.push({ txt: el.innerText.trim().slice(0, 26), scrollW: el.scrollWidth, clientW: el.clientWidth })
    }

    // Type below 11px is hard to read at arm's length.
    for (const el of document.querySelectorAll('main *')) {
      const cs = getComputedStyle(el)
      const fs = parseFloat(cs.fontSize)
      const ownText = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())
      if (ownText && fs < 11) out.tinyText.push({ txt: el.innerText.trim().slice(0, 20), fs })
    }

    // Vertical rhythm: the top edges of the page's direct blocks.
    const blocks = [...main.children].filter((e) => e.getBoundingClientRect().height > 0)
    for (let i = 1; i < blocks.length; i++) {
      const gapBetween = Math.round(blocks[i].getBoundingClientRect().top - blocks[i - 1].getBoundingClientRect().bottom)
      out.gaps.push(gapBetween)
    }
    return out
  })

  console.log(`\n── ${name}`)
  console.log('   section gaps (px):', facts.gaps.join(', ') || '—')
  console.log('   overflowing viewport:', facts.overflow.length ? JSON.stringify(facts.overflow.slice(0, 3)) : 'none')
  console.log('   clipped without ellipsis/title:', facts.clipped.length ? JSON.stringify(facts.clipped.slice(0, 3)) : 'none')
  console.log('   text under 11px:', facts.tinyText.length ? JSON.stringify(facts.tinyText.slice(0, 3)) : 'none')
}

// ── Inspector: is the rail dense enough, and does it scroll sensibly? ──────
await p.goto(BASE + '/codes/RQ-ABCD-EFGH-JKMN-T', { waitUntil: 'networkidle' })
await p.waitForTimeout(900)
const ins = await p.evaluate(() => {
  const el = document.querySelector('.inspector')
  const body = el.querySelector('.ins-body')
  const sections = [...el.querySelectorAll('.section-title')].map((s) => s.innerText.trim())
  return {
    railW: Math.round(el.getBoundingClientRect().width),
    railH: Math.round(el.getBoundingClientRect().height),
    contentH: body.scrollHeight,
    visibleH: body.clientHeight,
    scrollRatio: +(body.scrollHeight / body.clientHeight).toFixed(2),
    sections,
    factsGrid: getComputedStyle(el.querySelector('.ins-facts')).gridTemplateColumns,
  }
})
console.log('\n── Inspector')
console.log('   rail', ins.railW + 'x' + ins.railH, '| content', ins.contentH + 'px in', ins.visibleH + 'px', '=', ins.scrollRatio + ' screens')
console.log('   sections:', ins.sections.join(' → '))
console.log('   fact grid columns:', ins.factsGrid)

await b.close()
