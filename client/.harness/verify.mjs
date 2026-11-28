/**
 * The sidebar marks, measured in a real browser.
 *
 * # Why this is not a unit test
 *
 * Every way these two marks have gone wrong was a LAYOUT failure, and none of
 * them is visible from the DOM: the wordmark rendered 0×0 (an SVG with no
 * intrinsic size resolves `width: auto` to 0 rather than from its viewBox), the
 * glyph rendered at MUI's 24 px default (emotion's `.MuiSvgIcon-root` beats a
 * plain rule at the same specificity, because it lands later), the wordmark was
 * clamped to a 5.25 ratio instead of 8.0 and squashed, and the glyph was 2.3× the
 * ink height of the word beside it. `vitest` has no layout engine and would have
 * passed all four.
 *
 * Runtime in CI is ~3 s. It needs `npx playwright install chromium`.
 *
 *     node client/.harness/verify.mjs     # from `client/`
 */
import { chromium } from 'playwright'

const BASE = 'http://localhost:3100/index.html'

/** The slot the marks live in, derived rather than guessed (see the CSS). */
const SLOT = 200 - 20 - 20 - 1

const ART_RATIO = 376 / 47 // logo.svg's own ratio
const failures = []

const check = (ok, message) => {
  if (!ok) failures.push(message)
}

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 520, height: 120 } })
await page.goto(`${BASE}?which=logo`, { waitUntil: 'networkidle' })
await page.waitForTimeout(300)

const measure = () =>
  page.evaluate(() => {
    const svgs = [...document.querySelectorAll('.the-logo svg')]
    const box = (el) => {
      const r = el.getBoundingClientRect()
      return { w: +r.width.toFixed(2), h: +r.height.toFixed(2) }
    }
    const glyph = svgs[0]
    const word = svgs[svgs.length - 1]
    return {
      glyph: box(glyph),
      word: box(word),
      // The ink, as a fraction of each box, measured from the artwork.
      glyphInkRatio: 222.77 / 240,
      wordInkRatio: 46.67 / 47,
      slot: document.querySelector('.the-logo > div').getBoundingClientRect()
        .width,
    }
  })

const m = await measure()

// 1. Both marks have a size. The 0×0 case.
check(
  m.word.w > 0 && m.word.h > 0,
  `the wordmark is ${m.word.w}×${m.word.h} — an SVG with no width/height ` +
    'attributes needs an explicit size, or `width: auto` resolves to 0',
)
check(m.glyph.w > 0, `the glyph is ${m.glyph.w}px wide`)

// 2. The wordmark keeps its artwork's ratio. The squashed-letters case.
const ratio = m.word.w / m.word.h
check(
  Math.abs(ratio - ART_RATIO) < 0.15,
  `the wordmark box is ${ratio.toFixed(2)}:1 but its artwork is ` +
    `${ART_RATIO.toFixed(2)}:1 — the letters are distorted. A ` +
    '`max-width` beside a definite `height` does this.',
)

// 3. The glyph is not MUI's default. The emotion-wins-the-cascade case.
check(
  m.glyph.w !== 24,
  'the glyph is 24px — MUI’s `font-size` default. Its emotion rule for ' +
    '`.MuiSvgIcon-root` sets `width: 1em; height: 1em; font-size: 1.5rem`, so a ' +
    'rule that sizes only width/height can be left at the default em size.',
)

// 4. The glyph is a mark beside a name, not a symbol beside a footnote.
//    Bounded, not pinned: 12–28 px leaves room to retune the pair without
//    opening the door to 36 px (which is 2.0× the word's ink) or to MUI's 24.
check(
  m.glyph.w >= 12 && m.glyph.w <= 28,
  `the glyph is ${m.glyph.w}px; outside 12–28 it is either a speck or a blob ` +
    'beside a 16.4 px wordmark. 36 px gave an ink ratio of 2.0:1.',
)

// 4. The marks are comparable in weight. The blob-with-a-caption case.
//    Compared as INK heights, because the glyph fills 93 % of its box and the
//    wordmark 99 % of its own — equal boxes would not be equal weight.
const glyphInk = m.glyph.h * m.glyphInkRatio
const wordInk = m.word.h * m.wordInkRatio
const inkRatio = glyphInk / wordInk
check(
  inkRatio < 1.45,
  `the glyph's ink is ${inkRatio.toFixed(2)}× the wordmark's (${glyphInk.toFixed(
    1,
  )}px vs ${wordInk.toFixed(1)}px) — the mark dwarfs the name`,
)

// 5. They fit the slot. The overflow case.
const left = await page.evaluate(
  () => document.querySelector('.layout-content__left').clientWidth,
)
check(
  m.glyph.w + 8 + m.word.w <= left,
  `the marks need ${m.glyph.w + 8 + m.word.w}px but the sidebar is ${left}px`,
)

// 6. The collapsed rail draws the same mark, not a bigger one.
const rail = await page.evaluate(() => {
  document.querySelector('.layout').classList.add('layout--nav-collapsed')
  const svgs = [...document.querySelectorAll('.the-logo svg')]
  return {
    glyph: getComputedStyle(svgs[0]).width,
    word: getComputedStyle(svgs[svgs.length - 1]).display,
  }
})
check(
  rail.glyph === `${m.glyph.w}px` || parseFloat(rail.glyph) === m.glyph.w,
  `the collapsed rail draws the glyph at ${rail.glyph} but the open rail at ` +
    `${m.glyph.w}px — a mark should not change size with the layout`,
)
check(rail.word === 'none', 'the wordmark is still drawn in the 72px rail')

await browser.close()

if (failures.length) {
  console.error(`\n${failures.length} mark failure(s):\n`)
  for (const f of failures) console.error(`  ✗ ${f}\n`)
  process.exit(1)
}
console.log(
  `marks OK — glyph ${m.glyph.w}×${m.glyph.h}, word ${m.word.w}×${m.word.h} ` +
    `(${ratio.toFixed(2)}:1), ink ratio ${inkRatio.toFixed(2)}, slot ${left}px`,
)
