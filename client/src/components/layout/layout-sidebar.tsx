import { DragDropProvider, KeyboardSensor, PointerSensor } from '@dnd-kit/react'
import { Box, List, Menu, MenuItem, SvgIcon, useTheme } from '@mui/material'
import { useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'

import iconDark from '@/assets/image/icon_dark.svg?react'
import iconLight from '@/assets/image/icon_light.svg?react'
import LogoSvg from '@/assets/image/logo.svg?react'
import { useVerge } from '@/hooks/use-verge'
import { useNavMenuOrder } from '@/pages/_layout/hooks'
import { navItems } from '@/pages/_navigation'

import { SortableItem } from '../base'

import { LayoutItem } from './layout-item'
import { LayoutSubscription } from './layout-subscription'
import { LayoutTraffic } from './layout-traffic'

type MenuContextPosition = { top: number; left: number }

interface LayoutSidebarProps {
  isDark: boolean
  isCollapsed: boolean
}

const SENSORS = [PointerSensor, KeyboardSensor]

export const LayoutSidebar = (props: LayoutSidebarProps) => {
  const { isDark, isCollapsed } = props
  const { t } = useTranslation()
  const theme = useTheme()
  const { verge, mutateVerge, patchVerge } = useVerge()
  // Both SVGs are `fill: currentColor` now, so the dark/light pair is no longer
  // two colours but two *glyph weights*: the dark variant is the solid mark and
  // the light one the outlined mark, which is a legibility choice about the
  // background rather than about hue. The colour comes from the theme.
  const iconMark = isDark ? iconDark : iconLight
  const [menuUnlocked, setMenuUnlocked] = useState(false)
  const [menuContextPosition, setMenuContextPosition] =
    useState<MenuContextPosition | null>(null)

  const handleMenuOrderOptimisticUpdate = useCallback(
    (order: string[]) => {
      mutateVerge(
        (prev) => (prev ? { ...prev, menu_order: order } : prev),
        false,
      )
    },
    [mutateVerge],
  )

  const handleMenuOrderPersist = useCallback(
    (order: string[]) => patchVerge({ menu_order: order }),
    [patchVerge],
  )

  const {
    menuOrder,
    navItemMap,
    handleMenuDragEnd,
    isDefaultOrder,
    resetMenuOrder,
  } = useNavMenuOrder({
    enabled: menuUnlocked,
    items: navItems,
    storedOrder: verge?.menu_order,
    onOptimisticUpdate: handleMenuOrderOptimisticUpdate,
    onPersist: handleMenuOrderPersist,
  })

  const handleMenuContextMenu = useCallback(
    (event: React.MouseEvent<HTMLElement>) => {
      event.preventDefault()
      event.stopPropagation()
      setMenuContextPosition({ top: event.clientY, left: event.clientX })
    },
    [],
  )

  const handleMenuContextClose = useCallback(() => {
    setMenuContextPosition(null)
  }, [])

  const handleResetMenuOrder = useCallback(() => {
    setMenuContextPosition(null)
    void resetMenuOrder()
  }, [resetMenuOrder])

  const handleUnlockMenu = useCallback(() => {
    setMenuUnlocked(true)
    setMenuContextPosition(null)
  }, [])

  const handleLockMenu = useCallback(() => {
    setMenuUnlocked(false)
    setMenuContextPosition(null)
  }, [])

  const handleToggleNavCollapsed = useCallback(() => {
    setMenuContextPosition(null)
    void patchVerge({ collapse_navbar: !isCollapsed })
  }, [isCollapsed, patchVerge])

  // Navigation menu items
  const navMenuItems = menuOrder.map((path, index) => {
    const item = navItemMap.get(path)
    if (!item) return null

    return (
      <SortableItem
        key={item.path}
        id={item.path}
        index={index}
        disabled={!menuUnlocked}
      >
        {(sortable) => (
          <LayoutItem to={item.path} icon={item.icon} sortable={sortable}>
            {t(item.label)}
          </LayoutItem>
        )}
      </SortableItem>
    )
  })

  return (
    <div className="layout-content__left">
      {/* Logo.
          //
          // Both marks are painted in the theme's accent, and neither is a
          // per-theme asset: the SVGs were converted to `fill: currentColor`, so
          // the colour arrives from CSS rather than from nine copies of the same
          // file. That is the durable form of "recolour the logo per theme" — a
          // duplicated asset per theme is a file that can be forgotten, and the
          // one thing this wordmark used to do was ignore the theme entirely
          // (`fill={isDark ? 'white' : 'black'}`, which the SVG's own `.st1`
          // class overrode anyway, so it never changed at all).
          //
          // The accent is safe here by construction: the registry test asserts
          // every theme's accent clears 3:1 against both its page and its
          // surface, which is the floor for a non-text UI mark.
          //
          // `color` rather than `fill`: `SvgIcon` and the imported component
          // both resolve `currentColor` from the inherited `color`, so one
          // declaration drives the shield and the wordmark together. */}
      <div className="the-logo" data-tauri-drag-region="false">
        <div
          data-tauri-drag-region="true"
          style={{
            height: '27px',
            display: 'flex',
            justifyContent: 'space-between',
            color: theme.palette.primary.main,
          }}
        >
          <SvgIcon
            component={iconMark}
            style={{
              height: '36px',
              width: '36px',
              marginTop: '-3px',
              marginRight: '5px',
              marginLeft: '-3px',
            }}
            inheritViewBox
          />
          <LogoSvg />
        </div>
      </div>

      {/* Edit navigation menu badge */}
      {menuUnlocked && (
        <Box
          sx={(theme) => ({
            px: 1.5,
            py: 0.75,
            mx: 'auto',
            mb: 1,
            maxWidth: 250,
            borderRadius: 1.5,
            fontSize: 12,
            fontWeight: 600,
            textAlign: 'center',
            color: theme.palette.warning.contrastText,
            bgcolor:
              theme.palette.mode === 'light'
                ? theme.palette.warning.main
                : theme.palette.warning.dark,
          })}
        >
          {t('layout.components.navigation.menu.reorderMode')}
        </Box>
      )}

      {/* Navigation menu */}
      <List className="the-menu" onContextMenu={handleMenuContextMenu}>
        <DragDropProvider sensors={SENSORS} onDragEnd={handleMenuDragEnd}>
          {navMenuItems}
        </DragDropProvider>
      </List>

      {/* Context menu */}
      <Menu
        open={Boolean(menuContextPosition)}
        onClose={handleMenuContextClose}
        anchorReference="anchorPosition"
        anchorPosition={
          menuContextPosition
            ? {
                top: menuContextPosition.top,
                left: menuContextPosition.left,
              }
            : undefined
        }
        transitionDuration={200}
        slotProps={{
          list: {
            sx: { py: 0.5 },
          },
        }}
      >
        <MenuItem onClick={handleToggleNavCollapsed} dense>
          {isCollapsed
            ? t('layout.components.navigation.menu.expandNavBar')
            : t('layout.components.navigation.menu.collapseNavBar')}
        </MenuItem>
        <MenuItem
          onClick={menuUnlocked ? handleLockMenu : handleUnlockMenu}
          dense
        >
          {menuUnlocked
            ? t('layout.components.navigation.menu.lock')
            : t('layout.components.navigation.menu.unlock')}
        </MenuItem>
        <MenuItem
          onClick={handleResetMenuOrder}
          dense
          disabled={isDefaultOrder}
        >
          {t('layout.components.navigation.menu.restoreDefaultOrder')}
        </MenuItem>
      </Menu>

      {/* Subscription expiry, above the traffic figures so it reads as a
          standing notice about the account rather than a live reading. Renders
          nothing when there is no date to show. */}
      <LayoutSubscription />

      {/* Traffic */}
      <div className="the-traffic">
        <LayoutTraffic />
      </div>
    </div>
  )
}
