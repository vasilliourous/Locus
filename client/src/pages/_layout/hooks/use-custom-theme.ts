import { alpha, createTheme, Theme as MuiTheme, Shadows } from '@mui/material'
import {
  getCurrentWebviewWindow,
  WebviewWindow,
} from '@tauri-apps/api/webviewWindow'
import { Theme as TauriOsTheme } from '@tauri-apps/api/window'
import { useEffect, useMemo } from 'react'

import { useVerge } from '@/hooks/use-verge'
import {
  defaultDarkTheme,
  defaultTheme,
  LOCUS_COLORS,
  LOCUS_LIGHT,
} from '@/pages/_theme'
import {
  DECORATIONS,
  resolveTheme,
  themeDefaultFor,
} from '@/pages/_themes'
import { useSetThemeMode, useThemeMode } from '@/services/states'

const CSS_INJECTION_SCOPE_ROOT = '[data-css-injection-root]'
const CSS_INJECTION_SCOPE_LIMIT =
  ':is(.monaco-editor .view-lines, .monaco-editor .view-line, .monaco-editor .margin, .monaco-editor .margin-view-overlays, .monaco-editor .view-overlays, .monaco-editor [class^="mtk"], .monaco-editor [class*=" mtk"])'
const TOP_LEVEL_AT_RULES = [
  '@charset',
  '@import',
  '@namespace',
  '@font-face',
  '@keyframes',
  '@counter-style',
  '@page',
  '@property',
  '@font-feature-values',
  '@color-profile',
]
let cssScopeSupport: boolean | null = null

const canUseCssScope = () => {
  if (cssScopeSupport !== null) {
    return cssScopeSupport
  }
  try {
    const testStyle = document.createElement('style')
    testStyle.textContent = '@scope (:root) { }'
    document.head.appendChild(testStyle)
    cssScopeSupport = !!testStyle.sheet?.cssRules?.length
    document.head.removeChild(testStyle)
  } catch {
    cssScopeSupport = false
  }
  return cssScopeSupport
}

const wrapCssInjectionWithScope = (css?: string) => {
  if (!css?.trim()) {
    return ''
  }
  const lowerCss = css.toLowerCase()
  const hasTopLevelOnlyRule = TOP_LEVEL_AT_RULES.some((rule) =>
    lowerCss.includes(rule),
  )
  if (hasTopLevelOnlyRule) {
    return null
  }
  const scopeRoot = CSS_INJECTION_SCOPE_ROOT
  const scopeLimit = CSS_INJECTION_SCOPE_LIMIT
  const scopedBlock = `@scope (${scopeRoot}) to (${scopeLimit}) {
${css}
}`
  return scopedBlock
}

/**
 * custom theme
 */
export const useCustomTheme = () => {
  const appWindow: WebviewWindow = useMemo(() => getCurrentWebviewWindow(), [])
  const { verge } = useVerge()
  const { theme_mode, theme_setting, theme_id } = verge ?? {}
  const mode = useThemeMode()
  const setMode = useSetThemeMode()
  const userBackgroundImage = theme_setting?.background_image || ''
  const hasUserBackground = !!userBackgroundImage

  /**
   * The selected theme. `undefined`/empty/unknown all resolve to `default-dark`.
   *
   * `resolveTheme` never throws and never returns nothing, so every consumer
   * below can treat `spec` as a real theme. That is the same "never invent a
   * state" rule the subscription union follows -- see `docs/reference/THEMES.md`
   * section 2.
   */
  const spec = useMemo(() => resolveTheme(theme_id), [theme_id])

  /**
   * Whether a named theme is selected.
   *
   * This is the single branch that makes the whole layer additive: when it is
   * false, every effect below behaves exactly as it did before themes existed,
   * because a device that never opened the theme dropdown has no `theme_id`.
   *
   * **If you are debugging a theme bug, start here.** `hasTheme` decides both
   * the mode (`effectiveMode`) and which window-chrome branch runs, so a mistake
   * in it presents as two unrelated symptoms at once. `THEMES.md` §9 has the
   * symptom-to-cause table; `docs/reference/FIXES.md` (2026-10-01) records what
   * was built and what was demonstrated to fail.
   */
  const hasTheme = typeof theme_id === 'string' && theme_id.length > 0

  /**
   * The effective mode.
   *
   * A theme carries its own mode (one mode per theme -- `THEMES.md` section 2), so
   * selecting a theme supersedes `theme_mode`. With no theme selected, `mode` is
   * the context value, which is the `theme_mode`/system resolution as before.
   */
  const effectiveMode: 'light' | 'dark' = hasTheme ? spec.mode : mode

  useEffect(() => {
    if (theme_mode === 'light' || theme_mode === 'dark') {
      setMode(theme_mode)
    }
  }, [theme_mode, setMode])

  useEffect(() => {
    if (theme_mode !== 'system') {
      return
    }

    let isMounted = true

    const timerId = setTimeout(() => {
      if (!isMounted) return
      appWindow
        .theme()
        .then((systemTheme) => {
          if (isMounted && systemTheme) {
            setMode(systemTheme)
          }
        })
        .catch((err) => {
          console.error('Failed to get initial system theme:', err)
        })
    }, 0)

    const unlistenPromise = appWindow.onThemeChanged(({ payload }) => {
      if (isMounted) {
        setMode(payload)
      }
    })

    return () => {
      isMounted = false
      clearTimeout(timerId)
      unlistenPromise
        .then((unlistenFn) => {
          if (typeof unlistenFn === 'function') {
            unlistenFn()
          }
        })
        .catch((err) => {
          console.error('Failed to unlisten from theme changes:', err)
        })
    }
  }, [theme_mode, appWindow, setMode])

  useEffect(() => {
    if (hasTheme) {
      // The native window chrome follows the theme's mode, so a dark theme does
      // not get light title-bar buttons. `setTheme` is the OS-level hint only;
      // it does not paint the app.
      appWindow.setTheme(spec.mode as TauriOsTheme).catch((err) => {
        console.error(`Failed to set window theme to ${spec.mode}:`, err)
      })
      return
    }

    if (theme_mode === undefined) {
      return
    }

    if (theme_mode === 'system') {
      appWindow.setTheme(null).catch((err) => {
        console.error(
          'Failed to set window theme to follow system (setTheme(null)):',
          err,
        )
      })
    } else if (mode) {
      appWindow.setTheme(mode as TauriOsTheme).catch((err) => {
        console.error(`Failed to set window theme to ${mode}:`, err)
      })
    }
  }, [mode, appWindow, theme_mode, hasTheme, spec.mode])

  /**
   * The decoration and shape layer.
   *
   * Purely additive, and deliberately kept out of the MUI theme object: a theme's
   * decoration is one named preset from `DECORATIONS` (never CSS text -- see
   * `THEMES.md` section 1), written as a single `<style>` element scoped to
   * `[data-theme-skin]`. A preset is a structure of one or more declaration
   * blocks (see `DecorationSpec`); each is scoped identically, so a layer cannot
   * introduce a selector of its own. Nothing here targets a component, a class
   * or the document, so deleting this effect removes every trace of the layer
   * and leaves the shipped appearance byte-identical.
   *
   * The radii go out as CSS variables *in addition to* being applied through MUI,
   * because `_surfaces.ts` reads them for the shared card. A theme that sets no
   * radius writes today's values, so an unthemed app is unchanged.
   */
  useEffect(() => {
    const root = document.documentElement
    if (!root) {
      return
    }

    root.setAttribute('data-theme-id', spec.id)
    root.style.setProperty('--card-radius', `${spec.shape.cardRadius}px`)
    root.style.setProperty('--control-radius', `${spec.shape.controlRadius}px`)

    const preset = spec.decoration ? DECORATIONS[spec.decoration] : null
    if (preset) {
      root.setAttribute('data-theme-skin', spec.decoration!)
    } else {
      root.removeAttribute('data-theme-skin')
    }

    let el = document.querySelector('style#locus-theme-decoration')
    if (!preset) {
      el?.remove()
      return
    }
    if (!el) {
      el = document.createElement('style')
      el.id = 'locus-theme-decoration'
      document.head.appendChild(el)
    }
    // Scoped to the skin attribute, so a preset cannot reach a component even by
    // accident. `data-theme-skin` is only present while a decorated theme is
    // active, which is why removing the attribute above also disarms the CSS.
    //
    // A preset is a *structure* now: `short` is the root declarations, and each
    // entry of `layers` is a further declaration block painted on top, in order.
    // Both forms go through the same scoping, so no layer can introduce a
    // selector of its own.
    //
    // When `layers` is empty this emits exactly what it emitted before the type
    // changed — one scoped block, no extra whitespace beyond the template's — so
    // a flat preset like `forest-glow` is byte-identical to the shipped output.
    const blocks = [preset.short, ...preset.layers]
      .map((block) => `[data-theme-skin] { ${block} }`)
      .join('\n')
    el.textContent = blocks
  }, [spec])

  const theme = useMemo(() => {
    const setting = theme_setting || {}
    // `font_family` is a preference rather than a theme field, so it is read from
    // the same shipped defaults it always came from and handed to the projection.
    // A theme supplies every other value; none supplies a font.
    const legacyFont =
      effectiveMode === 'light' ? defaultTheme : defaultDarkTheme
    const dt = themeDefaultFor(spec, legacyFont.font_family)
    let muiTheme: MuiTheme

    try {
      muiTheme = createTheme({
        breakpoints: {
          values: { xs: 0, sm: 650, md: 900, lg: 1200, xl: 1536 },
        },
        palette: {
          mode: effectiveMode,
          primary: { main: setting.primary_color || dt.primary_color },
          secondary: { main: setting.secondary_color || dt.secondary_color },
          info: { main: setting.info_color || dt.info_color },
          error: { main: setting.error_color || dt.error_color },
          warning: { main: setting.warning_color || dt.warning_color },
          success: { main: setting.success_color || dt.success_color },
          text: {
            primary: setting.primary_text || dt.primary_text,
            secondary: setting.secondary_text || dt.secondary_text,
          },
          background: {
            paper: dt.background_color,
            default: dt.background_color,
          },
        },
        shadows: Array(25).fill('none') as Shadows,
        shape: { borderRadius: spec.shape.controlRadius },
        typography: {
          fontFamily: setting.font_family
            ? `${setting.font_family}, ${dt.font_family}`
            : dt.font_family,
        },
      })
    } catch (e) {
      console.error('Error creating MUI theme, falling back to defaults:', e)
      muiTheme = createTheme({
        breakpoints: {
          values: { xs: 0, sm: 650, md: 900, lg: 1200, xl: 1536 },
        },
        palette: {
          mode: effectiveMode,
          primary: { main: dt.primary_color },
          secondary: { main: dt.secondary_color },
          info: { main: dt.info_color },
          error: { main: dt.error_color },
          warning: { main: dt.warning_color },
          success: { main: dt.success_color },
          text: { primary: dt.primary_text, secondary: dt.secondary_text },
          background: {
            paper: dt.background_color,
            default: dt.background_color,
          },
        },
        typography: { fontFamily: dt.font_family },
      })
    }

    const rootEle = document.documentElement
    if (rootEle) {
      // These were Verge's greys. Every one of them is visible against Locus's
      // green-black surface (a neutral `#3E3E3E` selection block next to a green
      // page reads as a rendering bug), so they follow the brand palette while
      // keeping the same light/dark relationship.
      const backgroundColor =
        mode === 'light' ? LOCUS_LIGHT.background : dt.background_color
      const selectColor =
        mode === 'light' ? LOCUS_LIGHT.surfaceHover : LOCUS_COLORS.surfaceHover
      const scrollColor =
        mode === 'light' ? '#8CA59680' : LOCUS_COLORS.textSecondary
      const dividerColor =
        mode === 'light'
          ? 'rgba(11, 31, 20, 0.08)'
          : 'rgba(234, 242, 236, 0.08)'
      rootEle.style.setProperty('--divider-color', dividerColor)
      rootEle.style.setProperty('--background-color', backgroundColor)
      rootEle.style.setProperty('--selection-color', selectColor)
      rootEle.style.setProperty('--scroller-color', scrollColor)
      rootEle.style.setProperty('--primary-main', muiTheme.palette.primary.main)
      rootEle.style.setProperty(
        '--background-color-alpha',
        alpha(muiTheme.palette.primary.main, 0.1),
      )
      rootEle.style.setProperty(
        '--window-border-color',
        mode === 'light' ? LOCUS_LIGHT.border : LOCUS_COLORS.border,
      )
      rootEle.style.setProperty(
        '--scrollbar-bg',
        mode === 'light' ? LOCUS_LIGHT.background : LOCUS_COLORS.surface,
      )
      rootEle.style.setProperty(
        '--scrollbar-thumb',
        mode === 'light' ? LOCUS_LIGHT.border : LOCUS_COLORS.border,
      )
      rootEle.style.setProperty(
        '--user-background-image',
        hasUserBackground ? `url('${userBackgroundImage}')` : 'none',
      )
      rootEle.style.setProperty(
        '--background-blend-mode',
        setting.background_blend_mode || 'normal',
      )
      rootEle.style.setProperty(
        '--background-opacity',
        setting.background_opacity !== undefined
          ? String(setting.background_opacity)
          : '1',
      )
      rootEle.setAttribute('data-css-injection-root', 'true')
    }

    let styleElement = document.querySelector('style#verge-theme')
    if (!styleElement) {
      styleElement = document.createElement('style')
      styleElement.id = 'verge-theme'
      document.head.appendChild(styleElement!)
    }

    if (styleElement) {
      let scopedCss: string | null = null
      if (canUseCssScope() && setting.css_injection) {
        scopedCss = wrapCssInjectionWithScope(setting.css_injection)
      }
      const effectiveInjectedCss = scopedCss ?? setting.css_injection ?? ''
      const globalStyles = `
        /* 修复滚动条样式 */
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
          background-color: ${mode === 'light' ? LOCUS_LIGHT.border : LOCUS_COLORS.textSecondary};
        }

        /* 背景图处理 */
        body {
          font-family: ${dt.font_family};
          background-color: var(--background-color);
          ${
            hasUserBackground
              ? `
            background-image: var(--user-background-image);
            background-size: cover;
            background-position: center;
            background-attachment: fixed;
            background-blend-mode: var(--background-blend-mode);
            opacity: var(--background-opacity);
          `
              : ''
          }
        }

        /* 修复可能的白色边框 */
        .MuiPaper-root {
          border-color: var(--window-border-color) !important;
        }

        /* 确保模态框和对话框也使用暗色主题 */
        .MuiDialog-paper {
          background-color: ${mode === 'light' ? LOCUS_LIGHT.surface : LOCUS_COLORS.surface} !important;
        }

        /* 移除可能的白色点或线条 */
        * {
          outline: none !important;
          box-shadow: none !important;
        }
      `

      styleElement.innerHTML = effectiveInjectedCss + globalStyles
    }

    return muiTheme
  }, [
    effectiveMode,
    // `mode` is read only to pick the legacy font fallback, but it is a real
    // input: a student on `theme_mode: system` who switches their OS from light
    // to dark must re-create the theme. Omitting it is a stale-closure bug the
    // linter caught, not a spurious warning.
    mode,
    theme_setting,
    userBackgroundImage,
    hasUserBackground,
    spec,
  ])

  useEffect(() => {
    const id = setTimeout(() => {
      const dom = document.querySelector('#Gradient2')
      if (dom) {
        dom.innerHTML = `
        <stop offset="0%" stop-color="${theme.palette.primary.main}" />
        <stop offset="80%" stop-color="${theme.palette.primary.dark}" />
        <stop offset="100%" stop-color="${theme.palette.primary.dark}" />
        `
      }
    }, 0)
    return () => clearTimeout(id)
  }, [theme.palette.primary.main, theme.palette.primary.dark])

  return { theme }
}
