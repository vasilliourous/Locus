import './assets/styles/index.scss'

import { ResizeObserver } from '@juggle/resize-observer'
import {
  Box,
  CircularProgress,
  CssBaseline,
  ThemeProvider,
} from '@mui/material'
import { ComposeContextProvider } from 'foxact/compose-context-provider'
import React, { useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router'
import { SWRConfig } from 'swr'
import { MihomoWebSocket } from 'tauri-plugin-mihomo-api'

import { BaseErrorBoundary } from './components/base'
import { gateScreen, useEntitlement } from './hooks/use-entitlement'
import { hideInitialOverlay } from './pages/_layout/utils/initial-loading-overlay'
import { router } from './pages/_routers'
import ActivationScreen from './pages/activation'
import SuspendedScreen from './pages/suspended'
import { AppDataProvider } from './providers/app-data-provider'
import { WindowProvider } from './providers/window'
import { FALLBACK_LANGUAGE, initializeLanguage } from './services/i18n'
import { preloadAppData } from './services/preload'
import { swrConfig } from './services/query-client'
import { LoadingCacheProvider } from './services/states'
import { applyThemeWhenReady, chrysalisTheme } from './services/theme'
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

// The window-chrome variables and the global stylesheet, written once before
// React mounts. Explicit rather than a module side effect, so importing the theme
// module has no side effect for a test that only wants the palette.
applyThemeWhenReady()

const initializeApp = () => {
  const root = createRoot(container)

  // The gate wraps everything, so an unactivated device cannot reach the
  // router, the profile machinery or the connect controls by any route — not
  // just by the navigation being hidden. "Render nothing else until activated"
  // is a much easier property to hold than "every page remembers to check".
  const Shell = () => {
    // The entitlement, from the one hook that owns it.
    //
    // This used to be three state variables, a hand-added Tauri listener and a
    // five-minute poll owned here, with the kick-out screen and the activation
    // screen each keeping their own copy of the same facts. Two screens could then
    // disagree about one device, and a change a human had just made took up to
    // five minutes to arrive. `use-entitlement.ts` states the rule once: the
    // backend is the only authority, and everything that can change the answer
    // asks it again immediately.
    const entitlement = useEntitlement()

    // Remove the initial loading overlay once this component has painted.
    //
    // This MUST live here, not in `Layout`. The overlay is an opaque full-screen
    // element at `z-index: 9999` (see `index.html`), and it is only removed by
    // `hideInitialOverlay()`. That used to be called from `useLoadingOverlay`
    // inside `Layout` — but `Layout` is reached through `RouterProvider`, which
    // renders only in the `app` branch below. So on an unactivated device (every
    // fresh install, and the state a student sees first) the overlay was never
    // removed and covered the activation screen permanently: a blank window over a
    // working app. The DOM was correct, the assets all loaded 200, and nothing
    // threw — which is why this presented as an unexplained empty window rather
    // than as an error.
    //
    // The effect is keyed on nothing because it needs to run exactly once, on
    // first mount, whichever branch then renders.
    useEffect(() => {
      hideInitialOverlay()
    }, [])

    // `gateScreen` is a pure function so the decision is testable without mounting
    // React against a Tauri mock — see its own note for why the ORDER is the fix.
    const screen = gateScreen(entitlement.activated, entitlement.suspended)

    if (screen === 'loading') {
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

    // ── The activation gate, BEFORE the kick-out screen ──
    //
    // A suspended device still holds its code (that is what makes unsuspending
    // work without re-entry), so in the healthy case this branch and the one below
    // are mutually exclusive. The ORDER still matters, and it is the difference
    // between a recoverable device and a bricked install: a suspension is a
    // statement about a code, so with no code it means nothing. A device that is
    // signed out is shown the code prompt — never a page whose remedy is "wait for
    // the middleman" and whose only control is the support address.
    //
    // That pair used to be reachable: signing out of a suspended device removed the
    // code and left the flag, and the reported exit was wiping the app's data. The
    // backend no longer produces the pair (`store::is_suspended` refuses it, and
    // `clear_entitlement` clears the flag); this ordering means a config already
    // written by an affected build cannot trap anyone either.
    if (screen === 'activation') {
      return (
        <ActivationScreen
          onActivated={() => {
            // The hub has accepted a code, so the account changed. The hook
            // re-reads rather than trusting this callback's optimism, but the read
            // is immediate and the gate leaves the prompt on this same tick.
            entitlement.refresh()
          }}
        />
      )
    }

    // ── The kick-out screen ──
    //
    // What a suspended student gets: it says what happened and who to ask. It is
    // not folded into `!activated` because the device still holds its code, and
    // offering a code prompt would invite them to re-enter a key the hub already
    // knows and already refuses. The backend returns them here — and back out of it
    // — as the hub answers, so no control is needed.
    if (screen === 'suspended') {
      return <SuspendedScreen status={entitlement.status} />
    }

    const contexts = [<LoadingCacheProvider key="loading" />]

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

  // The provider is ABOVE the activation gate and the suspension screen, not
  // below the router.
  //
  // It used to sit here as `createTheme()` — Material UI's stock default — while
  // the real theme was built inside `Layout`, which those two screens never
  // reach. So the first thing a student ever saw was MUI's blue button and MUI's
  // type scale over hand-written colour constants. With one permanent appearance
  // there is exactly one theme to provide, and it is provided here, once, for
  // every screen.
  root.render(
    <React.StrictMode>
      <ThemeProvider theme={chrysalisTheme}>
        <CssBaseline />
        <BaseErrorBoundary>
          <Shell />
        </BaseErrorBoundary>
      </ThemeProvider>
    </React.StrictMode>,
  )
}

const bootstrap = async () => {
  await preloadAppData()
  initializeApp()
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
      initializeApp()
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
