import {
  alpha,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
} from '@mui/material'
import type { ReactNode } from 'react'
import { useMatch, useNavigate, useResolvedPath } from 'react-router'

import type { SortableItemRenderProps } from '@/components/base/sortable-item'
import { useVerge } from '@/hooks/use-verge'

interface Props {
  to: string
  children: string
  icon: ReactNode[]
  sortable?: SortableItemRenderProps
}
export const LayoutItem = (props: Props) => {
  const { to, children, icon, sortable } = props
  const { verge } = useVerge()
  const { menu_icon } = verge ?? {}
  const navCollapsed = verge?.collapse_navbar ?? false
  const resolved = useResolvedPath(to)
  const match = useMatch({ path: resolved.pathname, end: true })
  const navigate = useNavigate()

  const effectiveMenuIcon =
    navCollapsed && menu_icon === 'disable' ? 'monochrome' : menu_icon

  return (
    <ListItem
      ref={sortable?.ref}
      style={sortable?.style}
      sx={{ py: 0.5, maxWidth: 250, mx: 'auto', padding: '4px 0px' }}
    >
      <ListItemButton
        ref={sortable?.handleRef}
        selected={!!match}
        sx={[
          {
            borderRadius: 2,
            marginLeft: 1.25,
            paddingLeft: 1,
            paddingRight: 1,
            marginRight: 1.25,
            cursor: 'pointer',
            '& .MuiListItemText-primary': {
              color: 'text.primary',
              fontWeight: '700',
            },
          },
          ({ palette: { mode, primary, text } }) => {
            const bgcolor =
              mode === 'light'
                ? alpha(primary.main, 0.15)
                : alpha(primary.main, 0.35)
            // The selected nav item's label, read from the theme rather than
            // from a pair of hardcoded neutrals, so it tracks the palette in
            // both modes.
            const color = text.primary
            return {
              '&.Mui-selected': { bgcolor },
              '&.Mui-selected:hover': { bgcolor },
              '&.Mui-selected .MuiListItemText-primary': { color },
            }
          },
        ]}
        title={navCollapsed ? children : undefined}
        aria-label={navCollapsed ? children : undefined}
        onClick={() => navigate(to)}
      >
        {(effectiveMenuIcon === 'monochrome' || !effectiveMenuIcon) && (
          <ListItemIcon
            sx={{
              color: 'text.primary',
              marginLeft: '6px',
              cursor: 'inherit',
            }}
          >
            {icon[0]}
          </ListItemIcon>
        )}
        {effectiveMenuIcon === 'colorful' && (
          /* The disc is the accent colour and the glyph is the canvas ground,
             which is a 17:1 pair. The pair this replaces was white ink on the
             same disc — 2.5:1, under the 3:1 that WCAG 1.4.11 asks of a non-text
             icon, so the two-icon state read as a solid dot at the sidebar's
             size.

             Both are CSS custom properties rather than literal colours in the
             SVGs: an SVG's own `fill`/`stop-color` are not reachable from the
             theme, and a literal would have to be restated in four files
             whenever the palette moved. They are declared here, on the wrapper
             the SVG is a child of, so they inherit into it. */
          <ListItemIcon
            sx={(theme) => ({
              cursor: 'inherit',
              '--itemicon-disc': theme.palette.primary.main,
              '--itemicon-glyph': theme.palette.background.paper,
            })}
          >
            {icon[1]}
          </ListItemIcon>
        )}
        <ListItemText
          sx={{
            textAlign: 'center',
            marginLeft: effectiveMenuIcon === 'disable' ? '' : '-35px',
          }}
          primary={children}
        />
      </ListItemButton>
    </ListItem>
  )
}
