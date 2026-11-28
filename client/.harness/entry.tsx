/**
 * A rendering harness for the marks, used only to LOOK at them.
 *
 * It mounts the real components — the sidebar's logo block, the real
 * `LayoutSidebar` — inside the real theme and the real stylesheet. That is
 * deliberate: every defect this exists to catch was a disagreement between an
 * SVG, a stylesheet and a theme value, and a mock of any one of the three would
 * reproduce the disagreement rather than reveal it.
 *
 * Not shipped and not imported by the app. `vitest` has no layout; this has a
 * browser, and `shoot.mjs` turns what it draws into numbers.
 *
 * Delete `.harness/`, `harness.vite.config.mts` and the `playwright` devDependency
 * when the marks are confirmed.
 */
import { SvgIcon, ThemeProvider } from '@mui/material'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router'

import SuspendedScreen from '@/pages/suspended'
import { LayoutSubscription } from '@/components/layout/layout-subscription'
import { initializeLanguage } from '@/services/i18n'
import { gateScreen } from '@/hooks/use-entitlement'
import type { ChrysalisStatus } from '@/services/chrysalis'
import glyph from '@/assets/image/glyph.svg?react'
import LogoSvg from '@/assets/image/logo.svg?react'
import { LayoutSidebar } from '@/components/layout/layout-sidebar'
import { chrysalisTheme } from '@/services/theme'
import '@/assets/styles/index.scss'

/** The sidebar's logo block, copied from `layout-sidebar.tsx`. */
const LogoBlock = () => (
  <div className="the-logo">
    <div
      style={{
        height: '27px',
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
        color: chrysalisTheme.palette.primary.main,
      }}
    >
      <SvgIcon component={glyph} inheritViewBox />
      <LogoSvg />
    </div>
  </div>
)

/**
 * A fixed-size page. Every scenario is measured as this box, so a screenshot is
 * the thing under test and nothing else — a `width: 100%` layout filling the
 * viewport is how an earlier version "measured" a mostly-empty 3816 px image.
 */
const Frame = ({
  w,
  h,
  children,
}: {
  w: number
  h: number
  children: React.ReactNode
}) => (
  <div
    style={{ width: w, height: h, background: '#1A1B26', overflow: 'visible' }}
  >
    {children}
  </div>
)

/**
 * The status the GATE already read, as the suspension screen receives it.
 *
 * Only `subscription` is read by `SuspendedScreen`; the other fields are present
 * because the prop's type is the whole status, and a cast is honest here — the
 * harness is supplying the fact the app supplies, not re-deriving it.
 */
const refusedStatus = (reason: string): ChrysalisStatus =>
  ({
    activated: true,
    suspended: true,
    subscription: { state: 'refused', reason },
  }) as unknown as ChrysalisStatus

const Scenarios = () => {
  const params = new URLSearchParams(location.search)
  const which = params.get('which') ?? 'logo'
  const collapsed = params.get('collapsed') === '1'
  const w = Number(params.get('w') ?? 240)
  // `menu_icon` decides which of the two icon sets the nav draws; the default in
  // `config/verge.rs` is `monochrome`, and `colorful` is the disc art.
  const menuIcon = params.get('menu_icon') ?? 'monochrome'

  if (which === 'logo') {
    /* `.the-logo`'s rules are keyed on the FULL ancestor chain
     * (`.layout .layout-content__left .the-logo > div > svg`), so the wrappers are
     * not decoration — without them the selector matches nothing and the
     * measurement is of an unstyled SVG.
     *
     * The right pane is present because `.layout-content__left` is
     * `flex: 1 0 200px` AGAINST `flex: 1 1 100%`: with no sibling, the left pane
     * stretches to whatever width the frame is and the slot measured is not the
     * one that ships. It is 200 px in the app at every window size from 520 to
     * 1400, and reproducing that is the whole point of this scenario. */
    return (
      <Frame w={520} h={90}>
        <div className="layout" style={{ width: 520, display: 'flex' }}>
          <div
            className="layout-content"
            style={{ display: 'flex', width: '100%' }}
          >
            <div className="layout-content__left" style={{ flex: '1 0 200px' }}>
              <LogoBlock />
            </div>
            <div style={{ flex: '1 1 100%' }} />
          </div>
        </div>
      </Frame>
    )
  }

  // The kick-out screen, rendered from the status shape a release-eviction
  // produces. This is the screen R3 requires the client to end up on; the gate
  // that chooses it is `gateScreen` (unit-tested), and this asserts what the
  // student actually reads once it has.
  if (which === 'suspended') {
    const reason =
      params.get('reason') ??
      'Account suspended — this code was released by your provider'
    // THE SHIPPED DECISION, called with the pair a release-eviction produces
    // (`store::is_suspended` requires a code, so `activated` is true).
    const gate = gateScreen(true, true)
    return (
      <Frame w={520} h={520}>
        <div data-gate={gate}>
          {gate === 'suspended' ? (
            <SuspendedScreen status={refusedStatus(reason)} />
          ) : (
            <div data-gate-wrong="1">gate chose {gate}</div>
          )}
        </div>
      </Frame>
    )
  }

  // The subscription chip, from a `SubscriptionState` fixture. `state=never` is
  // the hub confirming there is no expiry; `state=active` is the countdown.
  if (which === 'subscription') {
    const state = params.get('state') ?? 'never'
    const days = Number(params.get('days') ?? 30)
    window.__SUBSCRIPTION_FIXTURE__ = {
      activated: true,
      subscription:
        state === 'never'
          ? { state: 'never' }
          : state === 'lapsed'
            ? { state: 'lapsed' }
            : {
                state: 'active',
                daysRemaining: days,
                urgent: days <= 7,
                expiresAt: '2027-06-30 00:00:00.000Z',
              },
    }
    return (
      <Frame w={280} h={80}>
        <div
          className="layout"
          style={{ width: 280, height: 80, display: 'flex' }}
        >
          <div
            className="layout-content"
            style={{ display: 'flex', height: '100%', width: '100%' }}
          >
            <div
              className="layout-content__left"
              style={{ flex: '0 0 280px', width: 280, paddingTop: 16 }}
            >
              <LayoutSubscription />
            </div>
          </div>
        </div>
      </Frame>
    )
  }

  if (which === 'activation') {
    return (
      <Frame w={420} h={220}>
        <div
          style={{
            color: chrysalisTheme.palette.text.primary,
            padding: 24,
            textAlign: 'center',
            fontFamily: chrysalisTheme.typography.fontFamily,
          }}
        >
          <SvgIcon
            component={glyph}
            inheritViewBox
            titleAccess="Chrysalis"
            sx={{ width: 64, height: 64, mb: 1.5 }}
          />
          <div style={{ fontSize: 34, fontWeight: 700, color: '#7AA2F7' }}>
            Chrysalis
          </div>
        </div>
      </Frame>
    )
  }

  // The real sidebar. `.the-logo` and the collapsed rail are styled by
  // `layout.scss` keyed on the app's own class names, so the CSS under
  // measurement is the shipped CSS.
  return (
    <Frame w={w} h={collapsed ? 160 : 420}>
      <div
        className={`layout ${collapsed ? 'layout--nav-collapsed' : ''}`}
        data-menu-icon={menuIcon}
        style={{ width: w, height: '100%' }}
      >
        <div
          className="layout-content"
          style={{ display: 'flex', height: '100%' }}
        >
          <div
            className="layout-content__left"
            style={{ flex: `0 0 ${w}px`, width: w }}
          >
            <LayoutSidebar isCollapsed={collapsed} />
          </div>
        </div>
      </div>
    </Frame>
  )
}

const router = createMemoryRouter([{ path: '*', element: <Scenarios /> }], {
  initialEntries: ['/'],
})

// The locale bundles arrive by dynamic import, exactly as they do at app
// start-up, so `t()` renders sentences rather than keys. Render FIRST and let the
// bundles land: the provider re-renders on the i18n `initialized` event, and
// gating the mount on the import made the marks scenarios render later than the
// 300 ms their guard allows.
void initializeLanguage('en')

createRoot(document.getElementById('root')!).render(
  <ThemeProvider theme={chrysalisTheme}>
    <RouterProvider router={router} />
  </ThemeProvider>,
)
