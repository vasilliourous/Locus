import './assets/styles/index.scss'

import { ResizeObserver } from '@juggle/resize-observer'
import {
  Box,
  CircularProgress,
  createTheme,
  CssBaseline,
  ThemeProvider,
} from '@mui/material'
import { ComposeContextProvider } from 'foxact/compose-context-provider'
import React, { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router'
import { SWRConfig } from 'swr'
import { MihomoWebSocket } from 'tauri-plugin-mihomo-api'

import { BaseErrorBoundary } from './components/base'
import { SUBSCRIPTION_POLL_MS as ENTITLEMENT_POLL_MS } from './hooks/use-subscription'
import { hideInitialOverlay } from './pages/_layout/utils/initial-loading-overlay'
import { router } from './pages/_routers'
import ActivationScreen from './pages/activation'
import { AppDataProvider } from './providers/app-data-provider'
import { WindowProvider } from './providers/window'
import { FALLBACK_LANGUAGE, initializeLanguage } from './services/i18n'
import { locusStatus } from './services/locus'
import {
  preloadAppData,
  resolveThemeMode,
  getPreloadConfig,
} from './services/preload'
import { swrConfig } from './services/query-client'
import { LoadingCacheProvider, ThemeModeProvider } from './services/states'
import { disableWebViewShortcuts } from './utils/disable-webview-shortcuts'

if (!window.ResizeObserver) {
  window.ResizeObserver = ResizeObserver
}

const mainElementId = 'root'
const container = document.getElementById(mainElementId)

if (!container) {
  throw new Error(`No container '${mainElementId}' found to render application`)
}

disableWebViewShortcuts()

const initializeApp = (initialThemeMode: 'light' | 'dark') => {
  const root = createRoot(container)

  // The gate wraps everything, so an unactivated device cannot reach the
  // router, the profile machinery or the connect controls by any route — not
  // just by the navigation being hidden. "Render nothing else until activated"
  // is a much easier property to hold than "every page remembers to check".
  const Shell = () => {
    const [activated, setActivated] = useState<boolean | null>(null)

    // Remove the initial loading overlay once this component has painted.
    //
    // This MUST live here, not in `Layout`. The overlay is an opaque full-screen
    // element at `z-index: 9999` (see `index.html`), and it is only removed by
    // `hideInitialOverlay()`. That used to be called from `useLoadingOverlay`
    // inside `Layout` — but `Layout` is reached through `RouterProvider`, which
    // renders only in the `activated === true` branch below. So on an
    // unactivated device (every fresh install, and the state a student sees
    // first) the overlay was never removed and covered the activation screen
    // permanently: a blank window over a working app. The DOM was correct, the
    // assets all loaded 200, and nothing threw — which is why this presented as
    // an unexplained empty window rather than as an error.
    //
    // The effect is keyed on nothing because it needs to run exactly once, on
    // first mount, whichever branch then renders.
    useEffect(() => {
      hideInitialOverlay()
    }, [])

    useEffect(() => {
      let cancelled = false

      const read = () =>
        locusStatus()
          .then((status) => {
            if (cancelled) return
            // Re-evaluated on every poll, not just on mount. The entitlement can
            // be withdrawn by a background heartbeat at any moment — a
            // suspension, an expiry, a refund — and this used to read the status
            // exactly once, so the app kept rendering a connection screen for a
            // device that no longer had one. The only way a student saw it was to
            // close and reopen the app, which is why that became the workaround.
            setActivated(status.activated)
          })
          .catch(() => {
            // If we cannot read the state, assume NOT activated — but only on
            // the FIRST read. A transient failure while running must not tear a
            // working connection screen away from a student mid-session; that
            // would turn a momentary storage hiccup into a surprise logout.
            if (cancelled) return
            setActivated((current) => current ?? false)
          })

      void read()

      // The same interval the subscription query uses, so the gate and the
      // sidebar indicator cannot disagree about the same device.
      const timer = window.setInterval(() => void read(), ENTITLEMENT_POLL_MS)
      return () => {
        cancelled = true
        window.clearInterval(timer)
      }
    }, [])

    if (activated === null) {
      return (
        <Box
          sx={{
            width: '100vw',
            height: '100vh',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <CircularProgress size={28} />
        </Box>
      )
    }

    if (!activated) {
      return <ActivationScreen onActivated={() => setActivated(true)} />
    }

    const contexts = [
      <ThemeModeProvider key="theme" initialState={initialThemeMode} />,
      <LoadingCacheProvider key="loading" />,
    ]

    return (
      <ComposeContextProvider contexts={contexts}>
        <BaseErrorBoundary>
          <SWRConfig value={swrConfig}>
            <WindowProvider>
              <AppDataProvider>
                <RouterProvider router={router} />
              </AppDataProvider>
            </WindowProvider>
          </SWRConfig>
        </BaseErrorBoundary>
      </ComposeContextProvider>
    )
  }

  root.render(
    <React.StrictMode>
      <ThemeProvider theme={createTheme()}>
        <CssBaseline />
        <BaseErrorBoundary>
          <Shell />
        </BaseErrorBoundary>
      </ThemeProvider>
    </React.StrictMode>,
  )
}

const bootstrap = async () => {
  const appDataPromise = preloadAppData()

  const { initialThemeMode } = await appDataPromise
  initializeApp(initialThemeMode)
}

bootstrap().catch((error) => {
  console.error(
    '[main.tsx] App bootstrap failed, falling back to default language:',
    error,
  )
  initializeLanguage(FALLBACK_LANGUAGE)
    .catch((fallbackError) => {
      console.error(
        '[main.tsx] Fallback language initialization failed:',
        fallbackError,
      )
    })
    .finally(() => {
      initializeApp(resolveThemeMode(getPreloadConfig()))
    })
})

// Error handling
window.addEventListener('error', (event) => {
  console.error('[main.tsx] Global error:', event.error)
})

window.addEventListener('unhandledrejection', (event) => {
  console.error('[main.tsx] Unhandled promise rejection:', event.reason)
})

// Page close/refresh events
window.addEventListener('beforeunload', () => {
  // Clean up all WebSocket instances to prevent memory leaks
  MihomoWebSocket.cleanupAll()
})

// Page loaded event
window.addEventListener('DOMContentLoaded', () => {
  // Clean up all WebSocket instances to prevent memory leaks
  MihomoWebSocket.cleanupAll()
})
