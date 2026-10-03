/// <reference types="node" />
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'

import { describe, expect, it } from 'vitest'

import { LOCUS_COLORS, LOCUS_LIGHT } from '../src/pages/_theme'
import {
  DECORATIONS,
  DEFAULT_SHAPE,
  DEFAULT_THEME_ID,
  isThemeId,
  resolveTheme,
  themeDefaultFor,
  THEMES,
  THEME_IDS,
} from '../src/pages/_themes'

/**
 * The three layers that paint before the app is themed must agree.
 *
 * There are three, and each exists for a different moment:
 *
 *   1. the NATIVE window (`src-tauri/.../window.rs`) — painted by the OS before
 *      any web content exists;
 *   2. the DOCUMENT (`src/index.html`) — parsed before any bundle runs, so it
 *      cannot import the theme module;
 *   3. the APP (`_theme.tsx` via `use-custom-theme`) — the themed surface.
 *
 * A mismatch between any two of them is a visible flash of the wrong colour
 * during startup, which is exactly how Clash Verge Rev's greys lingered in this
 * fork after everything else had been rebranded. The copies cannot be removed —
 * layer 2 cannot import layer 3 — so this test is what keeps them honest.
 *
 * The Rust side is pinned separately, in `window.rs`'s own test module, because
 * a JS test cannot read a Rust `const`. Keeping the *pinned literal* here equal
 * to the one there is a human step; the comment in each says so.
 */
const read = (relative: string) =>
  readFileSync(path.resolve(__dirname, relative), 'utf8')

/**
 * File contents with comments removed, lowercased.
 *
 * Every file checked here explains WHICH upstream colours it replaced, so the
 * raw text legitimately contains the hex codes in prose. Without stripping,
 * these tests fail on a correct file and the tempting fix is to delete the
 * explanation — which is the documentation that stops the colours coming back.
 */
const readCode = (relative: string) =>
  read(relative)
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/.*$/gm, '')
    .toLowerCase()

/**
 * Braces must balance, and the block must not close early.
 *
 * Replaces "contains no braces", which was a proxy for this. A preset that
 * closes its own block early would let the *rest* of its text sit at the top
 * level of the injected `<style>` element, where it becomes a real selector
 * against the whole document — the exact escape the old check was protecting
 * against, which the old check did not actually detect.
 */
const assertBalancedBraces = (text: string, label: string): boolean => {
  let depth = 0
  for (const ch of text) {
    if (ch === '{') depth += 1
    else if (ch === '}') {
      depth -= 1
      // Going negative means a `}` with no `{` before it: the block closed
      // early and everything after it is loose in the stylesheet.
      if (depth < 0) {
        throw new Error(`${label}: unbalanced '}' — the block closes early`)
      }
    }
  }
  if (depth !== 0) {
    throw new Error(`${label}: ${depth} unclosed '{'`)
  }
  return true
}

/**
 * No construct that would introduce a top-level, document-wide rule.
 *
 * `@media`/`@supports` are permitted because they *wrap* declarations in the
 * scope they sit in rather than naming a new one. `@import`, `@charset`,
 * `@namespace` and `@font-face` are rejected: they either pull in another
 * stylesheet or define something global. A raw selector would need to appear
 * outside a block, which `assertBalancedBraces` plus the scoping in
 * `use-custom-theme` already prevent — this is the belt to that pair of braces,
 * named separately so a failure says which property broke.
 */
const assertNoEscapingConstruct = (text: string, label: string): boolean => {
  const lower = text.toLowerCase()
  for (const banned of ['@import', '@charset', '@namespace', '@font-face']) {
    if (lower.includes(banned)) {
      throw new Error(`${label}: '${banned}' is not allowed in a preset`)
    }
  }
  return true
}

/** No remote or inline asset can be loaded at runtime. */
const assertNoRemoteAsset = (text: string, label: string): boolean => {
  const lower = text.toLowerCase()
  for (const banned of ['url(', 'http://', 'https://', 'data:']) {
    if (lower.includes(banned)) {
      throw new Error(`${label}: '${banned}' is not allowed in a preset`)
    }
  }
  return true
}

/**
 * Register-level guards for the theme registry.
 *
 * These are the tests that make `docs/reference/THEMES.md` §3 real. The design
 * record states four rules a theme must satisfy; a rule stated only in prose is
 * a rule that holds until someone adds a seventh theme at 1am, and the failure
 * mode here is subtle — a palette that violates contrast does not crash, it just
 * becomes slightly unreadable for the student who needs the accessibility theme
 * most.
 *
 * # Why these are computed, not transcribed
 *
 * `CLAIMS.md` §2: a number written in a sentence cannot be recomputed and will
 * rot. So the floors live here as constants that the *test* applies to the
 * registry, rather than as a table of expected ratios in the design record. A
 * theme that drifts out of range fails here, and the doc never carries a value
 * that could be wrong.
 *
 * # Every test here has been demonstrated failing
 *
 * Per `docs/reference/DEBUGGING-METHOD.md`: a guard that cannot fail reads
 * exactly like a guard that passed. Each of these was run against an
 * intentionally broken registry entry before being kept — see the note on each.
 */
describe('theme registry', () => {
  /** WCAG relative luminance. Not the crude `0.2126r + …` byte average below. */
  const channel = (c: number) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4)
  }

  const relativeLuminance = (hex: string) => {
    const h = hex.replace('#', '')
    const r = parseInt(h.slice(0, 2), 16)
    const g = parseInt(h.slice(2, 4), 16)
    const b = parseInt(h.slice(4, 6), 16)
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
  }

  const contrast = (a: string, b: string) => {
    const l1 = relativeLuminance(a)
    const l2 = relativeLuminance(b)
    const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1]
    return (hi + 0.05) / (lo + 0.05)
  }

  const specs = Object.values(THEMES)

  it('the registry has one entry per declared id, and ids match their keys', () => {
    // Catches the realistic copy-paste error: duplicating an entry and leaving
    // the old `id` in place, which would silently drop a theme from the dropdown
    // and make the persisted value ambiguous between two entries.
    for (const [key, spec] of Object.entries(THEMES)) {
      expect(spec.id).toBe(key)
    }
    expect(THEME_IDS).toEqual(Object.keys(THEMES))
    expect(new Set(THEME_IDS).size).toBe(THEME_IDS.length)
  })

  it('every id is a known id, and unknown ids resolve to the default', () => {
    for (const id of THEME_IDS) {
      expect(isThemeId(id)).toBe(true)
    }
    // The load-bearing half: a value from a hand-edited config file must never
    // throw or blank the screen. Demonstrated failing by making `resolveTheme`
    // return `THEMES[id]` directly (undefined) — the third assertion goes red.
    expect(isThemeId('no-such-theme')).toBe(false)
    expect(isThemeId(undefined)).toBe(false)
    expect(isThemeId('')).toBe(false)
    expect(resolveTheme('no-such-theme').id).toBe(DEFAULT_THEME_ID)
    expect(resolveTheme(undefined).id).toBe(DEFAULT_THEME_ID)
    expect(resolveTheme(null).id).toBe(DEFAULT_THEME_ID)
    expect(resolveTheme({}).id).toBe(DEFAULT_THEME_ID)
  })

  it('the two default themes are byte-identical to the shipped palette', () => {
    // This is what makes the registry a SUPERSET of the shipped appearance rather
    // than a replacement for it: an install with no `theme_id` must render
    // exactly what it rendered before themes existed. If this fails, the layer is
    // no longer additive and the upgrade path is a visual change.
    const dark = THEMES['default-dark']
    expect(dark.palette.background).toBe(LOCUS_COLORS.background)
    expect(dark.palette.surface).toBe(LOCUS_COLORS.surface)
    expect(dark.palette.surfaceHover).toBe(LOCUS_COLORS.surfaceHover)
    expect(dark.palette.border).toBe(LOCUS_COLORS.border)
    expect(dark.palette.textPrimary).toBe(LOCUS_COLORS.textPrimary)
    expect(dark.palette.textSecondary).toBe(LOCUS_COLORS.textSecondary)
    expect(dark.palette.accent).toBe(LOCUS_COLORS.accent)
    expect(dark.palette.accentHover).toBe(LOCUS_COLORS.accentHover)
    expect(dark.palette.success).toBe(LOCUS_COLORS.success)
    expect(dark.palette.warning).toBe(LOCUS_COLORS.warning)
    expect(dark.palette.error).toBe(LOCUS_COLORS.error)

    const light = THEMES['default-light']
    expect(light.palette.background).toBe(LOCUS_LIGHT.background)
    expect(light.palette.surface).toBe(LOCUS_LIGHT.surface)
    expect(light.palette.surfaceHover).toBe(LOCUS_LIGHT.surfaceHover)
    expect(light.palette.border).toBe(LOCUS_LIGHT.border)
    expect(light.palette.accent).toBe(LOCUS_LIGHT.accent)
    expect(light.palette.accentHover).toBe(LOCUS_LIGHT.accentHover)
    // The two legacy typography values the MUI palette is built from.
    expect(light.palette.textPrimary).toBe('#0B1F14')
    expect(light.palette.textSecondary).toBe('#4A6356')
  })

  it('body text clears 4.5:1 on its own surface, and labels clear 3:1', () => {
    // THEMES.md §3 rule 1. Demonstrated failing by lowering `paper`'s
    // textSecondary to `#B9AE9C` (~2.1:1): the second assertion goes red.
    const failures: string[] = []
    for (const spec of specs) {
      const onSurface = contrast(
        spec.palette.textPrimary,
        spec.palette.surface,
      )
      const onBackground = contrast(
        spec.palette.textPrimary,
        spec.palette.background,
      )
      const labels = contrast(
        spec.palette.textSecondary,
        spec.palette.surface,
      )
      if (onSurface < 4.5) {
        failures.push(`${spec.id}: textPrimary/surface ${onSurface.toFixed(2)}`)
      }
      if (onBackground < 4.5) {
        failures.push(
          `${spec.id}: textPrimary/background ${onBackground.toFixed(2)}`,
        )
      }
      if (labels < 3) {
        failures.push(`${spec.id}: textSecondary/surface ${labels.toFixed(2)}`)
      }
    }
    expect(failures).toEqual([])
  })

  it('the accent is legible as text on the theme it belongs to', () => {
    // The shipped light theme needed a *darker* accent for exactly this reason
    // (`_theme.tsx`). Every theme inherits that obligation — an accent used on a
    // button label is body text, not decoration.
    const failures: string[] = []
    for (const spec of specs) {
      const onSurface = contrast(spec.palette.accent, spec.palette.surface)
      const onBackground = contrast(spec.palette.accent, spec.palette.background)
      if (onSurface < 3) {
        failures.push(`${spec.id}: accent/surface ${onSurface.toFixed(2)}`)
      }
      if (onBackground < 3) {
        failures.push(
          `${spec.id}: accent/background ${onBackground.toFixed(2)}`,
        )
      }
    }
    expect(failures).toEqual([])
  })

  it('a theme mode matches its palette direction', () => {
    // THEMES.md §3 rule 2. This is the check that catches a copy-paste error
    // between two similar themes — the realistic way a registry entry is authored
    // wrong. Demonstrated failing by flipping `midnight` to `mode: 'light'`.
    const failures: string[] = []
    for (const spec of specs) {
      const bg = relativeLuminance(spec.palette.background)
      const text = relativeLuminance(spec.palette.textPrimary)
      const textIsLighter = text > bg
      if (spec.mode === 'dark' && !textIsLighter) {
        failures.push(`${spec.id}: dark theme with dark text`)
      }
      if (spec.mode === 'light' && textIsLighter) {
        failures.push(`${spec.id}: light theme with light text`)
      }
    }
    expect(failures).toEqual([])
  })

  it('the structural relationships hold in every theme', () => {
    // THEMES.md §3 rule 3. These are the relationships the shipped palette was
    // built on; a theme that violates one looks broken in a way contrast maths
    // alone will not catch — a card you cannot see against the page, or a border
    // doing no work.
    const failures: string[] = []
    for (const spec of specs) {
      const p = spec.palette
      const surfaceStep = contrast(p.surface, p.background)
      const borderVsSurface = contrast(p.border, p.surface)
      const secondaryVsPrimary = contrast(p.textSecondary, p.textPrimary)

      // A card must be a visible step from the page. 1.03 is below any real
      // theme's figure and above "identical"; it catches a copy-paste where
      // surface was left equal to background.
      if (surfaceStep < 1.03) {
        failures.push(`${spec.id}: surface/background ${surfaceStep.toFixed(3)}`)
      }
      if (borderVsSurface < 1.15) {
        failures.push(
          `${spec.id}: border/surface ${borderVsSurface.toFixed(2)}`,
        )
      }
      // textSecondary must be *dimmer* than textPrimary, or the hierarchy has
      // been inverted and labels will read as louder than body copy.
      if (secondaryVsPrimary <= 1) {
        failures.push(
          `${spec.id}: textSecondary is not dimmer than textPrimary`,
        )
      }
    }
    expect(failures).toEqual([])
  })

  it('every theme carries a shape, and radii are sane', () => {
    for (const spec of specs) {
      expect(spec.shape.cardRadius).toBeGreaterThan(0)
      expect(spec.shape.controlRadius).toBeGreaterThan(0)
      // The shipped card is 12px. A radius past ~24 stops reading as a card in
      // this layout, and a control rounder than its card looks like a mistake.
      expect(spec.shape.cardRadius).toBeLessThanOrEqual(24)
      expect(spec.shape.controlRadius).toBeLessThanOrEqual(16)
    }
    // Today's values are what an unthemed app renders, so they are pinned.
    expect(DEFAULT_SHAPE.cardRadius).toBe(12)
    expect(DEFAULT_SHAPE.controlRadius).toBe(8)
  })

  it('a decoration names a preset that exists, and the preset is declarations only', () => {
    // THEMES.md §1: decoration is a NAMED PRESET, never CSS text. A theme that
    // could emit arbitrary CSS could restyle any component, which is the
    // interference property this layer exists to avoid — so the registry is
    // checked for the shape of the value, not just its validity.
    for (const spec of specs) {
      if (spec.decoration === undefined) continue
      expect(Object.hasOwn(DECORATIONS, spec.decoration)).toBe(true)
    }
    for (const [id, preset] of Object.entries(DECORATIONS)) {
      expect(id).toMatch(/^[a-z0-9-]+$/)
      // A preset is a *declaration block*, not a stylesheet. This used to be
      // enforced by rejecting any `{` or `}` — which was true when a preset was
      // a single flat string, and which blocked the nested, multi-layer blocks
      // (inner blocks, `@supports`, layered gradients) that a real rice needs.
      //
      // The property that actually matters is NOT "contains no braces", it is
      // "cannot escape its `[data-theme-skin]` scope, cannot load a remote
      // asset, and cannot execute". Those are now asserted directly, below:
      // every brace is balanced, no construct introduces a top-level selector,
      // and no rule can reach outside the scope. This is strictly stronger than
      // the old check — the old one rejected `{}` even inside a declaration
      // value, while permitting a stray unbalanced `}` to be caught only by
      // accident.
      expect(assertBalancedBraces(preset.short, id)).toBe(true)
      expect(assertNoEscapingConstruct(preset.short, id)).toBe(true)
      expect(assertNoRemoteAsset(preset.short, id)).toBe(true)
      for (const layer of preset.layers) {
        expect(assertBalancedBraces(layer, `${id}.layers`)).toBe(true)
        expect(assertNoEscapingConstruct(layer, `${id}.layers`)).toBe(true)
        expect(assertNoRemoteAsset(layer, `${id}.layers`)).toBe(true)
      }
    }
    // The preset used by `forest` is exercised, and the registry does not
    // accumulate dead presets nobody selects.
    const used = new Set(
      specs.map((s) => s.decoration).filter((d): d is string => !!d),
    )
    expect(used.size).toBeGreaterThan(0)
    for (const id of Object.keys(DECORATIONS)) {
      expect(used.has(id)).toBe(true)
    }
  })

  it('the projection carries every field the MUI palette is built from', () => {
    // `themeDefaultFor` is the bridge between the registry's vocabulary and the
    // hook's. A missing field here does not crash — it falls back to `undefined`
    // and the palette loses a colour — so the projection is asserted explicitly.
    for (const spec of specs) {
      const projected = themeDefaultFor(spec, 'sans-serif')
      expect(projected.primary_color).toBe(spec.palette.accent)
      expect(projected.background_color).toBe(spec.palette.background)
      expect(projected.primary_text).toBe(spec.palette.textPrimary)
      expect(projected.error_color).toBe(spec.palette.error)
      expect(projected.warning_color).toBe(spec.palette.warning)
      expect(projected.success_color).toBe(spec.palette.success)
      expect(projected.info_color).toBe(spec.palette.info)
      expect(projected.secondary_color).toBe(spec.palette.secondary)
      // Font is a preference, passed through rather than themed.
      expect(projected.font_family).toBe('sans-serif')
    }
  })
})

describe('locus palette consistency across paint layers', () => {
  it('the document background matches the theme background', () => {
    const html = read('../src/index.html')

    // Dark: the primary experience, and the one the spec defines.
    expect(html).toContain(
      `--bg-color: ${LOCUS_COLORS.background.toLowerCase()}`,
    )
    // Light: derived, but must match its counterpart too.
    expect(html).toContain(
      `--bg-color: ${LOCUS_LIGHT.background.toLowerCase()}`,
    )
  })

  it('the document text colour matches the theme text colour', () => {
    const html = read('../src/index.html')
    expect(html).toContain(
      `--text-color: ${LOCUS_COLORS.textPrimary.toLowerCase()}`,
    )
  })

  it('no Clash Verge grey survives in the document', () => {
    const html = read('../src/index.html').toLowerCase()
    // The two literals this fork inherited from upstream. Either reappearing
    // means the pre-bundle frame flashes another product's colours.
    expect(html).not.toContain('#2e303d')
    expect(html).not.toContain('#f5f5f5')
    expect(html).not.toContain('#181a1b')
  })

  it('the theme carries no Clash Verge blue or grey', () => {
    // Comments are stripped first, deliberately.
    //
    // The file explains WHICH upstream colours it replaced, so the raw text does
    // contain the hex codes — in prose. Scanning prose would fail on a file that
    // is correct, and the tempting fix would be to delete the explanation. Strip
    // comments and check the code instead, so the doc comment can stay specific.
    const theme = readCode('../src/pages/_theme.tsx')
    for (const verge of [
      '#007aff',
      '#0a84ff',
      '#2e303d',
      '#fc9b76',
      '#ff9f0a',
    ]) {
      expect(theme).not.toContain(verge)
    }
  })

  it('the comment-stripping did not defeat the check', () => {
    // Guard on the guard: if the strip became too eager it would remove real
    // code too, and the test above would pass vacuously. A colour that IS in the
    // code must still be found.
    const theme = readCode('../src/pages/_theme.tsx')
    expect(theme).toContain('#2ea86a')
  })

  it('the stylesheet fallback is Locus, not Verge', () => {
    // The fourth place the palette is written, and the easiest to forget: SCSS
    // `:root` variables are overwritten by the theme hook at runtime, so a wrong
    // value here is only visible for a moment on each launch — which is exactly
    // how it survives review. It previously held Verge's purple accent
    // (`#5b5c9d`), visible as a purple flash before the green theme applied.
    const scss = readCode('../src/assets/styles/index.scss')
    expect(scss).toContain('--primary-main: #2ea86a')
    expect(scss).toContain('--background-color: #06130c')
    expect(scss).not.toContain('#5b5c9d')
    expect(scss).not.toContain('#f5f5f5')
  })

  it('no component hardcodes an upstream surface colour', () => {
    // Component-level colours are the easiest to miss: they sit outside the
    // theme, so they do not follow light/dark and do not appear in any palette
    // review. Two were found this way — `base-page.tsx` painting the page
    // background `#1e1f27` (Verge's dark surface) on EVERY screen, and the
    // traffic graph falling back to Verge's purple when the palette was unset.
    //
    // Scans `src/components` and `src/pages` for the specific upstream literals.
    // Not a general "no hex outside the theme" rule: legitimate one-off colours
    // exist (verdict reds/greens on the activation field), and a blanket ban
    // would push people to obfuscate them rather than to the palette.
    const offenders: string[] = []
    const walk = (dir: string) => {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name)
        if (entry.isDirectory()) {
          walk(full)
          continue
        }
        if (!entry.name.endsWith('.tsx') && !entry.name.endsWith('.ts'))
          continue
        if (entry.name === '_theme.tsx') continue
        const code = readFileSync(full, 'utf8')
          .replace(/\/\*[\s\S]*?\*\//g, '')
          .replace(/\/\/.*$/gm, '')
          .toLowerCase()
        for (const upstream of [
          '#1e1f27',
          '#39393d',
          '#5b5c9d',
          '#9c27b0',
          '#33cf4d',
          '#bbbbbb',
        ]) {
          if (code.includes(upstream)) offenders.push(`${full} -> ${upstream}`)
        }
      }
    }
    walk(path.resolve(__dirname, '../src/components'))
    walk(path.resolve(__dirname, '../src/pages'))
    expect(offenders).toEqual([])
  })

  it('the tier colours are the spec values', () => {
    // Lowercased helper, lowercased needles — the two must agree or the
    // assertion can never fire.
    const theme = readCode('../src/pages/_theme.tsx')
    // From docs/archive/UI-AESTHETICS.md §7. Pinned because these are the
    // "tier sells itself" cues and a wrong gold/green would be a brand error
    // nobody notices in code review.
    expect(theme).toContain('#eab308') // strike, gold
    expect(theme).toContain('#46c186') // stealth, green
    expect(theme).toContain('#7fb48f') // eco, muted green
  })

  it('the accent is Locus green and the surfaces are the spec values', () => {
    expect(LOCUS_COLORS.accent).toBe('#2EA86A')
    expect(LOCUS_COLORS.background).toBe('#06130C')
    expect(LOCUS_COLORS.surface).toBe('#0C1711')
    expect(LOCUS_COLORS.border).toBe('#1F3629')
    expect(LOCUS_COLORS.textPrimary).toBe('#EAF2EC')
    expect(LOCUS_COLORS.textSecondary).toBe('#8CA596')
  })

  it('light-mode text is dark and dark-mode text is light', () => {
    // The activation screen shipped invisible once because a light palette was
    // paired with light text. Cheap invariant, catches the same class of error.
    const luminance = (hex: string) => {
      const r = parseInt(hex.slice(1, 3), 16)
      const g = parseInt(hex.slice(3, 5), 16)
      const b = parseInt(hex.slice(5, 7), 16)
      return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    expect(luminance(LOCUS_COLORS.textPrimary)).toBeGreaterThan(
      luminance(LOCUS_COLORS.background),
    )
    expect(luminance('#0B1F14')).toBeLessThan(luminance(LOCUS_LIGHT.background))
  })
})
