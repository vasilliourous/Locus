import { alpha, Box, CircularProgress } from '@mui/material'

interface BaseLoadingOverlayProps {
  isLoading: boolean
}

export const BaseLoadingOverlay: React.FC<BaseLoadingOverlayProps> = ({
  isLoading,
}) => {
  if (!isLoading) return null

  return (
    <Box
      sx={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        // Respect current theme; avoid a bright flash in dark mode. The scrim
        // is the page ground at alpha, so it darkens and lightens with the mode.
        backgroundColor: (theme) =>
          alpha(theme.palette.background.default, 0.6),
        zIndex: 1000,
      }}
    >
      <CircularProgress />
    </Box>
  )
}
