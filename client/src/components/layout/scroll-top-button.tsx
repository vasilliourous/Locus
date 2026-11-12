import KeyboardArrowUpIcon from '@mui/icons-material/KeyboardArrowUp'
import { alpha, IconButton, Fade, SxProps, Theme } from '@mui/material'

interface Props {
  onClick: () => void
  show: boolean
  sx?: SxProps<Theme>
}

export const ScrollTopButton = ({ onClick, show, sx }: Props) => {
  return (
    <Fade in={show}>
      <IconButton
        onClick={onClick}
        sx={{
          position: 'absolute',
          bottom: '20px',
          right: '20px',
          backgroundColor: (theme) =>
            alpha(theme.palette.text.primary, 0.1),
          '&:hover': {
            backgroundColor: (theme) =>
              alpha(theme.palette.text.primary, 0.2),
          },
          visibility: show ? 'visible' : 'hidden',
          ...sx,
        }}
      >
        <KeyboardArrowUpIcon />
      </IconButton>
    </Fade>
  )
}
