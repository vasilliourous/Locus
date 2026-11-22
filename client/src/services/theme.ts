import {
  alpha,
  createTheme,
  type Shadows,
  type Theme as MuiTheme,
} from '@mui/material'

import {
  DEFAULT_SHAPE,
  defaultTheme,
  locusFontFamily,
  LOCUS_COLORS,
} from '@/pages/_theme'

/**
 * The application's one and only MUI theme.
 *
 * # Why this is a module constant and not a hook
 *
 * This file replaces `use-custom-theme`, which rebuilt the theme, re-injected a
 * global stylesheet and re-applied a decoration preset on every change to
 * `theme_mode`, `theme_id` or the OS appearance. With one permanent appearance
 * none of those can change, and a hook that returns a constant would re-render
 * every consumer whenever its parent did — the "a context value with a fresh
 * identity defeats every `memo` below it" trap the client has already paid for.
 *
 * So the theme is built exactly once, at module load, and the window-chrome
 * variables are written exactly once. Nothing below has a runtime input except
 * `theme_setting`, which is a stored preference rather than a mode — see
 * {@link applyThemeSetting}.
 *
 * # The trap this file was written around
 *
 * The palette fields come from exactly one place ({@link defaultTheme}), and the
 * generated stylesheet reads the same object. The old hook had a documented
 * failure where the palette was built from one answer and the stylesheet from
 * another, which produced a white dialog paper under dark text — unreadable, and
 * only on a pop-up. One source, read twice, in one pass.
 */

/** The control radius MUI uses for buttons, inputs and selects. */
const CONTROL_RADIUS = DEFAULT_SHAPE.controlRadius

const buildTheme = (): MuiTheme =>
  createTheme({
    breakpoints: {
      values: { xs: 0, sm: 650, md: 900, lg: 1200, xl: 1536 },
    },
    palette: {
      mode: 'dark',
      primary: { main: defaultTheme.primary_color },
      secondary: { main: defaultTheme.secondary_color },
      info: { main: defaultTheme.info_color },
      error: { main: defaultTheme.error_color },
      warning: { main: defaultTheme.warning_color },
      success: { main: defaultTheme.success_color },
      text: {
        primary: defaultTheme.primary_text,
        secondary: defaultTheme.secondary_text,
      },
      background: {
        paper: defaultTheme.background_color,
        default: defaultTheme.background_color,
      },
    },
    // Zeroed app-wide, deliberately: Locus separates surfaces with a 1px border
    // and a one-step-up surface (`_surfaces.ts`), not with elevation. A card that
    // relied on `elevation` would be invisible.
    shadows: Array(25).fill('none') as Shadows,
    shape: { borderRadius: CONTROL_RADIUS },
    typography: { fontFamily: locusFontFamily },
  })

/** The application theme. Built once; never rebuilt. */
export const locusTheme = buildTheme()

/**
 * Writes the window-chrome variables and the global stylesheet.
 *
 * Called once at module load. These values are all derived from
 * {@link LOCUS_COLORS}, so this is a projection rather than a second palette:
 * change the palette and the scrollbars, selection colour and dialog paper follow
 * without an edit here.
 *
 * Idempotent and safe to call again — which is what makes it usable from a
 * `theme_setting` change (a stored preference) without rebuilding the theme.
 */
export const applyTheme = (theme: MuiTheme = locusTheme) => {
  // Guarded rather than assumed. This module is imported by the palette test,
  // which runs under Node with no DOM, so an unguarded `document` would make the
  // guard itself unimportable. Calling it before the document exists is a no-op
  // and the caller retries once — see `applyThemeWhenReady`.
  if (typeof document === 'undefined') return
  const root = document.documentElement
  if (!root) return

  const backgroundColor = LOCUS_COLORS.background
  const selectColor = LOCUS_COLORS.surfaceHover
  const scrollColor = alpha(LOCUS_COLORS.textSecondary, 0.5)
  const dividerColor = alpha(LOCUS_COLORS.textPrimary, 0.08)

  root.style.setProperty('--divider-color', dividerColor)
  root.style.setProperty('--background-color', backgroundColor)
  root.style.setProperty('--selection-color', selectColor)
  root.style.setProperty('--scroller-color', scrollColor)
  root.style.setProperty('--primary-main', theme.palette.primary.main)
  root.style.setProperty(
    '--background-color-alpha',
    alpha(theme.palette.primary.main, 0.1),
  )
  root.style.setProperty('--window-border-color', LOCUS_COLORS.border)
  root.style.setProperty('--scrollbar-bg', LOCUS_COLORS.surface)
  root.style.setProperty('--scrollbar-thumb', LOCUS_COLORS.border)
  root.style.setProperty('--card-radius', `${DEFAULT_SHAPE.cardRadius}px`)
  root.style.setProperty('--control-radius', `${CONTROL_RADIUS}px`)
  root.setAttribute('data-css-injection-root', 'true')

  let styleElement = document.querySelector('style#verge-theme')
  if (!styleElement) {
    styleElement = document.createElement('style')
    styleElement.id = 'verge-theme'
    document.head.appendChild(styleElement)
  }

  // The scrollbar, the surface border and the dialog paper. One source for all
  // three: the palette above.
  //
  // The `.MuiDialog-paper` rule carries `!important` because it has to beat
  // MUI's own `.MuiPaper-root` background, which MUI computes from the palette
  // mode. Both are now the same value, so the `!important` is belt and braces —
  // kept because an MUI upgrade that changes the computation order would
  // otherwise silently reintroduce a mismatched pop-up.
  //
  // NOTE: there is deliberately no `* { outline: none; box-shadow: none }` rule
  // here. The rule this file replaced had lost its selector and was being
  // discarded by the parser, so the app has never actually received either
  // reset — and adding it back would remove every focus ring in the product in
  // the same change that recolours it. Whether focus rings are visible is an
  // accessibility decision, not a theme one, and it is tracked separately.
  styleElement.textContent = `
    ::-webkit-scrollbar {
      width: 8px;
      height: 8px;
      background-color: var(--scrollbar-bg);
    }
    ::-webkit-scrollbar-thumb {
      background-color: var(--scrollbar-thumb);
      border-radius: 4px;
    }
    ::-webkit-scrollbar-thumb:hover {
      background-color: ${LOCUS_COLORS.textSecondary};
    }

    body {
      font-family: ${locusFontFamily};
      background-color: var(--background-color);
    }

    .MuiPaper-root {
      border-color: var(--window-border-color) !important;
    }

    .MuiDialog-paper {
      background-color: ${LOCUS_COLORS.surface} !important;
    }
  `
}

/**
 * Applies the theme once the document is ready.
 *
 * Kept as a function rather than a module-level call so importing this module has
 * no side effect — the palette test imports it under Node, where `document` does
 * not exist. `main.tsx` calls this at bootstrap, before React renders.
 */
export const applyThemeWhenReady = () => {
  applyTheme()
}
