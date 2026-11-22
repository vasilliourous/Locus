/// <reference types="node" />
import { readFileSync } from 'node:fs'
import path from 'node:path'

import { describe, expect, it } from 'vitest'

import {
  DEFAULT_SHAPE,
  defaultTheme,
  locusFontFamily,
  LOCUS_COLORS,
  TIER_COLORS,
} from '../src/pages/_theme'
import { locusTheme } from '../src/services/theme'

/**
 * The palette, and the fact that there is exactly one.
 *
 * # What this file used to be
 *
 * It guarded a nine-theme registry: that every id matched its key, that an
 * unknown id resolved to a default, that nine palettes cleared the contrast
 * floors, that a decoration preset could not escape its scope, and that the light
 * and dark halves never disagreed. On 2026-11-19 the theme layer was deleted and
 * the client became permanently Dawn (`/COLOURS.md` §0). Almost every one of
 * those tests lost its subject.
 *
 * # Deleting them was the honest move; this is what replaced them
 *
 * A test whose subject no longer exists does not fail — it passes vacuously, and
 * a check that cannot fail reads exactly like one that passed. So the registry
 * tests are gone rather than weakened, and three kinds of assertion remain:
 *
 *  1. **The contrast floors** (`§1`). A palette still has to be legible. These
 *     are unchanged apart from losing the "for every theme" loop.
 *  2. **The paint layers agree** (`§2`). There are still several files that each
 *     paint the same ground at a different moment, and the copies cannot be
 *     removed (one is Rust, one is parsed before any module loads). That test is
 *     the only thing keeping them honest, and it now compares one value in three
 *     files instead of a pair in six.
 *  3. **One palette, and it is Dawn** (`§3`). New. This is the assertion that
 *     makes the decision a property rather than a paragraph: a second palette
 *     reappearing anywhere fails here, and the values are pinned to Dawn so a
 *     "small tweak" to the ground is a deliberate act.
 *
 * # Every remaining test has been shown to fail
 *
 * `docs/reference/DEBUGGING-METHOD.md`: a guard that cannot fail reads exactly
 * like a guard that passed. Each assertion below was run against a deliberately
 * broken file before being kept — the note on each says which break was used.
 */

const read = (relative: string) =>
  readFileSync(path.resolve(__dirname, relative), 'utf8')

/**
 * File contents with comments removed, lowercased.
 *
 * Every file checked here explains WHICH upstream colours it replaced, so the
 * raw text legitimately contains hex codes in prose. Without stripping, these
 * tests fail on a correct file and the tempting fix is to delete the
 * explanation — which is the documentation that stops the colours coming back.
 */
const readCode = (relative: string) =>
  read(relative)
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/.*$/gm, '')
    .toLowerCase()

/** WCAG relative luminance. Not a crude byte average. */
const channel = (c: number) => {
  const s = c / 255
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
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

const palette = LOCUS_COLORS

describe('the one palette is legible', () => {
  it('body text clears 4.5:1 on both the page and the card', () => {
    // The floor WCAG AA sets for body text. Checked against `background` AND
    // `surface` because a card is where most of this app's text lives, and a
    // palette can clear the page while failing the card.
    //
    // Demonstrated failing by setting `textPrimary` near `surface`.
    expect(
      contrast(palette.textPrimary, palette.background),
    ).toBeGreaterThanOrEqual(4.5)
    expect(
      contrast(palette.textPrimary, palette.surface),
    ).toBeGreaterThanOrEqual(4.5)
  })

  it('secondary text clears 4.5:1 on the card', () => {
    // Labels and hints are still text a student must read, so they get the body
    // floor rather than the 3:1 non-text one. Demonstrated failing by narrowing
    // the gap between `textSecondary` and `surface`.
    expect(
      contrast(palette.textSecondary, palette.surface),
    ).toBeGreaterThanOrEqual(4.5)
  })

  it('the accent is legible as a non-text mark on the page', () => {
    // The wordmark and the primary action are painted in the accent, so this is
    // both a button-label question and "can you see the brand" (3:1).
    expect(contrast(palette.accent, palette.background)).toBeGreaterThanOrEqual(
      3,
    )
  })

  it('every status role is legible on the card', () => {
    // A status colour that cannot be seen is worse than no status colour: the
    // dot is the whole signal. 3:1 is the floor for a non-text UI component.
    for (const role of ['success', 'warning', 'error'] as const) {
      expect(
        contrast(palette[role], palette.surface),
        `${role} is not legible on the card surface`,
      ).toBeGreaterThanOrEqual(3)
    }
  })

  it('no value is pure white or pure black', () => {
    // The rule the redesign exists to enforce, and the one easiest to
    // reintroduce: both are what a palette reaches for by default. Pure white on
    // a panel glares and makes text halo; pure black crushes the surface ladder
    // until a card and the page are the same plane.
    const PURE = new Set(['#ffffff', '#000000'])
    for (const [field, value] of Object.entries(palette)) {
      expect(
        PURE.has(String(value).toLowerCase()),
        `${field} is ${value}`,
      ).toBe(false)
    }
  })

  it('the page and a card are two planes, not one with a line on it', () => {
    // MUI's shadow scale is zeroed app-wide, so a card is separated from the page
    // by its 1px border *and* a one-step-up surface. If the two surfaces collapse
    // to the same value the border is doing all the work, which is what the old
    // default-dark did — measured at 1.14:1.
    //
    // A low bar on purpose: this asserts the ladder exists, not that it is
    // strong. The exact separation is a design decision, recorded in
    // /COLOURS.md §2, and a tighter floor here would turn a taste question into a
    // test failure.
    expect(contrast(palette.surface, palette.background)).toBeGreaterThan(1.15)
  })
})

describe('the paint layers agree', () => {
  /**
   * There are three, and each exists for a different moment:
   *
   *   1. the NATIVE window (`src-tauri/.../window.rs`) — painted by the OS
   *      before any web content exists;
   *   2. the DOCUMENT (`src/index.html`) — parsed before any bundle runs, so it
   *      cannot import the theme module;
   *   3. the APP (`_theme.tsx` via `services/theme.ts`) — the themed surface.
   *
   * A mismatch is a visible flash of the wrong colour on every launch. The copies
   * cannot be removed — layer 2 cannot import layer 3, and layer 1 is Rust — so
   * this is what keeps them honest.
   */
  it('the native window uses the palette ground', () => {
    // The Rust side cannot import the TS palette, so its literal is compared
    // here. This is the one place a JS test reads Rust source.
    //
    // Demonstrated failing by changing the constant in `window.rs` alone.
    const rust = read('../src-tauri/src/utils/resolve/window.rs')
    expect(rust).toContain(
      `const BACKGROUND_HEX: &str = "${palette.background.toUpperCase()}";`,
    )
  })

  it('the document background matches the palette ground', () => {
    // Demonstrated failing by editing `index.html`'s `--bg-color` alone.
    const html = read('../src/index.html')
    expect(html).toContain(`--bg-color: ${palette.background.toLowerCase()}`)
  })

  it('the document text colour matches the palette', () => {
    const html = read('../src/index.html')
    expect(html).toContain(`--text-color: ${palette.textPrimary.toLowerCase()}`)
  })

  it('the native window text colour is the palette body colour', () => {
    // Passed through the bootstrap rather than written into the script, so there
    // is one literal. Demonstrated failing by changing `TEXT_HEX` alone.
    const rust = read('../src-tauri/src/utils/resolve/window.rs')
    expect(rust).toContain(
      `const TEXT_HEX: &str = "${palette.textPrimary.toUpperCase()}";`,
    )
  })

  it('the pre-bundle bootstrap script carries the palette ground and text', () => {
    // `window_script.rs` paints the page before the bundle runs, and it cannot
    // import `LOCUS_COLORS`. Its fallback values are a fourth copy of both.
    //
    // Demonstrated failing by changing either fallback alone.
    const script = read('../src-tauri/src/utils/resolve/window_script.rs')
    expect(script).toContain(`darkBg: "${palette.background.toUpperCase()}"`)
    expect(script).toContain(`textFg: "${palette.textPrimary.toUpperCase()}"`)
  })

  it('no Clash Verge grey survives in the document', () => {
    // The two literals this fork inherited from upstream. Either reappearing
    // means the pre-bundle frame flashes another product's colours.
    const html = read('../src/index.html').toLowerCase()
    expect(html).not.toContain('#2e303d')
    expect(html).not.toContain('#f5f5f5')
    expect(html).not.toContain('#181a1b')
  })

  it('the palette carries no Clash Verge blue or grey', () => {
    // Comments are stripped first, deliberately: the file explains which
    // upstream colours it replaced, so the raw text contains them in prose.
    // Scanning prose would fail on a correct file and tempt the author to delete
    // the explanation.
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
    // code too, and the tests above would pass vacuously. A value that IS in the
    // code must still be found — taken from the palette itself, so this stays
    // true whatever the colour design becomes.
    const theme = readCode('../src/pages/_theme.tsx')
    expect(theme).toContain(palette.accent.toLowerCase())
  })

  it('the stylesheet fallback matches the palette', () => {
    // The fourth place the palette is written, and the easiest to forget: SCSS
    // `:root` variables are overwritten at runtime, so a wrong value here is only
    // visible for a moment on each launch — which is exactly how it survives
    // review. Asserted against the palette rather than a literal, so it keeps its
    // meaning through a recolour.
    const scss = readCode('../src/assets/styles/index.scss')
    expect(scss).toContain(`--primary-main: ${palette.accent.toLowerCase()}`)
    expect(scss).toContain(
      `--background-color: ${palette.background.toLowerCase()}`,
    )
    expect(scss).toContain(
      `--text-primary: ${palette.textPrimary.toLowerCase()}`,
    )
  })

  it('no component hardcodes an upstream surface colour', () => {
    // A component that names a colour directly is a component that does not
    // follow the palette. The allowlist is the palette module itself plus the
    // tier badge's frozen wire-name table.
    const offenders: string[] = []
    const files = [
      '../src/pages/connection.tsx',
      '../src/pages/account.tsx',
      '../src/pages/activation.tsx',
      '../src/pages/suspended.tsx',
      '../src/components/shared/support-block.tsx',
    ]
    for (const file of files) {
      const code = readCode(file)
      // A literal hex in a component is the defect, whatever its value.
      const hits = code.match(/#[0-9a-f]{6}\b/g) ?? []
      if (hits.length > 0) offenders.push(`${file}: ${hits.join(', ')}`)
    }
    expect(offenders).toEqual([])
  })
})

describe('there is exactly one appearance', () => {
  it('the palette is Dawn', () => {
    // The decision (`/COLOURS.md` §0) is that Locus is permanently Dawn. Pinned
    // here so a change to any of these is a deliberate edit to the decision
    // rather than a tweak nobody noticed.
    //
    // Demonstrated failing by changing `background`.
    expect(palette).toEqual({
      background: '#1A1B26',
      surface: '#24283B',
      surfaceHover: '#2F3349',
      border: '#3B4261',
      textPrimary: '#C0CAF5',
      textSecondary: '#9AA5CE',
      accent: '#7AA2F7',
      accentHover: '#9AB8FF',
      success: '#9ECE6A',
      warning: '#E0AF68',
      error: '#F7768E',
      secondary: '#BB9AF7',
      info: '#7DCFFF',
    })
  })

  it('the MUI projection is built from the palette, field for field', () => {
    // `defaultTheme` is the bridge between this file's vocabulary and MUI's. A
    // missing field does not crash — the palette silently loses a colour — so
    // every field is asserted against its source rather than spot-checked.
    expect(defaultTheme.primary_color).toBe(palette.accent)
    expect(defaultTheme.secondary_color).toBe(palette.secondary)
    expect(defaultTheme.primary_text).toBe(palette.textPrimary)
    expect(defaultTheme.secondary_text).toBe(palette.textSecondary)
    expect(defaultTheme.info_color).toBe(palette.info)
    expect(defaultTheme.error_color).toBe(palette.error)
    expect(defaultTheme.warning_color).toBe(palette.warning)
    expect(defaultTheme.success_color).toBe(palette.success)
    expect(defaultTheme.background_color).toBe(palette.background)
    expect(defaultTheme.font_family).toBe(locusFontFamily)
  })

  it('the MUI theme is dark, and reads the palette rather than repeating it', () => {
    // The theme is a projection: if any of these disagreed with the palette, the
    // app would have two palettes that gradually diverged. `mode` is asserted
    // because a light `mode` would make MUI compute its own light defaults for
    // every component that does not read a colour directly.
    expect(locusTheme.palette.mode).toBe('dark')
    expect(locusTheme.palette.background.default).toBe(palette.background)
    expect(locusTheme.palette.background.paper).toBe(palette.background)
    expect(locusTheme.palette.text.primary).toBe(palette.textPrimary)
    expect(locusTheme.palette.text.secondary).toBe(palette.textSecondary)
    expect(locusTheme.palette.primary.main).toBe(palette.accent)
    expect(locusTheme.palette.success.main).toBe(palette.success)
    expect(locusTheme.palette.warning.main).toBe(palette.warning)
    expect(locusTheme.palette.error.main).toBe(palette.error)
    expect(locusTheme.shape.borderRadius).toBe(DEFAULT_SHAPE.controlRadius)
  })

  it('the shadow scale is zeroed, so nothing relies on elevation', () => {
    // `_surfaces.ts` documents that a card is separated by a border and a
    // surface step. That is only true while every shadow is `none`; an MUI
    // default shadow reappearing would make the border look doubled.
    expect(new Set(locusTheme.shadows).size).toBe(1)
    expect(locusTheme.shadows[24]).toBe('none')
  })

  it('no theme machinery survives in the source tree', () => {
    // The point of the change, asserted as a property. These names are the layer
    // that was deleted; any of them reappearing means it came back.
    //
    // Demonstrated failing by leaving `useThemeMode` exported from `states.ts`.
    const files = [
      '../src/services/states.ts',
      '../src/pages/_theme.tsx',
      '../src/services/theme.ts',
      '../src/services/preload.ts',
      '../src/main.tsx',
      '../src/pages/_layout.tsx',
    ]
    const banned = [
      'THEMES',
      'THEME_IDS',
      'resolveTheme',
      'isThemeId',
      'themeDefaultFor',
      'DECORATIONS',
      'useCustomTheme',
      'ThemeModeProvider',
      'useThemeMode',
      'resolveThemeMode',
      'LOCUS_LIGHT',
    ]
    const offenders: string[] = []
    for (const file of files) {
      const code = readCode(file)
      for (const name of banned) {
        // Word-boundary match, lowercased because `readCode` lowercases the whole
        // file. A plain `includes` would fire on `useWindowDecorations`, which is
        // a window-chrome hook and has nothing to do with paint presets — the
        // guard has to name the identifier, not any string containing it.
        const pattern = new RegExp(`\\b${name.toLowerCase()}\\b`)
        if (pattern.test(code)) offenders.push(`${file}: ${name}`)
      }
    }
    expect(offenders).toEqual([])
  })

  it('the theme registry file is gone', () => {
    // A file that no longer exists cannot be imported, but it can be recreated
    // by a merge. This is the cheapest way to notice.
    expect(() => read('../src/pages/_themes.ts')).toThrow()
  })

  it('every tier wire name still has a badge colour', () => {
    // The tier table was NOT removed with the theme layer: its keys are frozen
    // wire names and the tint is redundant encoding for a label, not a palette.
    // Losing one here means a paying student sees a bare wire name.
    for (const wire of ['strike', 'budget', 'free', 'eco']) {
      expect(TIER_COLORS[wire], `${wire} has no badge colour`).toBeDefined()
    }
  })
})
