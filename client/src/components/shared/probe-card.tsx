import { TravelExploreOutlined } from '@mui/icons-material'
import { Box, Button, Paper, Stack, Typography, useTheme } from '@mui/material'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cardSx } from '@/pages/_surfaces'
import { chrysalisProbe, type ProbeResult } from '@/services/chrysalis'
import { errorDetail } from '@/services/notice-service'

/**
 * "Ask the hub what it thinks about this device".
 *
 * # Why this exists, and why it is a button rather than a check
 *
 * A student reports "I'm signed in but they say I'm not". The two sides hold
 * different facts and neither can see the other's, so the only way to settle it
 * is to ask the hub and compare — which is a deliberate act, not something a
 * timer should do. It is never called on a schedule: that would be the polling
 * storm the heartbeat's backoff exists to prevent, against a 10-per-10-minute
 * rate limit, and the answer does not change on its own.
 *
 * # The rule this component must not break
 *
 * The hub answers TWO axes — `entitlement` ("may this code be used") and
 * `liveness` ("is this device reaching us") — and neither one is "signed in".
 * This card renders them as the hub's own words and shows the raw `reasonCode`,
 * because that code is what `docs/operate/RUNBOOK-DESYNC.md` §4 is keyed on.
 * It deliberately does not fold them into a single verdict: that translation is
 * what produced the original bug, and doing it again here — on the one screen a
 * student is most likely to screenshot — is how it would come back.
 *
 * In particular `liveness: "unknown"` is NOT absence. It means the hub has never
 * seen this code check in, so the question is unanswered. See `livenessSentence`.
 */

/**
 * The client's sentence for the hub's liveness word.
 *
 * Every branch says what was OBSERVED. `live` says "the server heard from this
 * device recently", not "you are online" — the device reaching the hub does not
 * mean the student is at it, and the copy must not imply otherwise.
 *
 * `Record<string, …>` because the hub's vocabulary is the contract and an
 * unrecognised value must fall back to the "unanswered" sentence rather than
 * render `undefined` at a student. That fallback is the `unknown` key, which is
 * also the honest answer: something we do not understand is not evidence of
 * absence.
 */
const LIVENESS_LABEL = {
  live: 'home.components.connection.account.probe.livenessLive',
  stale: 'home.components.connection.account.probe.livenessStale',
  absent: 'home.components.connection.account.probe.livenessAbsent',
  unknown: 'home.components.connection.account.probe.livenessUnknown',
} as const

const LIVENESS_UNKNOWN = LIVENESS_LABEL.unknown

/** The key for a hub liveness word, falling back to the unanswered sentence. */
function livenessKey(
  state: string,
): (typeof LIVENESS_LABEL)[keyof typeof LIVENESS_LABEL] {
  return state in LIVENESS_LABEL
    ? LIVENESS_LABEL[state as keyof typeof LIVENESS_LABEL]
    : LIVENESS_UNKNOWN
}

export function ProbeCard() {
  const { t } = useTranslation()
  const theme = useTheme()
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<ProbeResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  const run = async () => {
    setBusy(true)
    setError(null)
    try {
      setResult(await chrysalisProbe())
    } catch (failure) {
      // A failure is reported as a failure, never as a fallback report. A
      // fabricated "unknown" here would read as the hub's verdict, which is the
      // exact confusion this card exists to end.
      setResult(null)
      setError(errorDetail(failure))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Paper elevation={0} sx={{ p: 2, ...cardSx(theme) }}>
      <Stack
        direction="row"
        spacing={0.75}
        sx={{ alignItems: 'center', mb: 1 }}
      >
        <TravelExploreOutlined
          fontSize="small"
          sx={{ color: 'text.secondary' }}
        />
        <Typography variant="subtitle2">
          {t('home.components.connection.account.probe.title')}
        </Typography>
      </Stack>

      <Typography variant="body2" sx={{ color: 'text.secondary', mb: 1.5 }}>
        {t('home.components.connection.account.probe.intro')}
      </Typography>

      <Button
        variant="outlined"
        size="small"
        disabled={busy}
        onClick={() => void run()}
      >
        {busy
          ? t('home.components.connection.account.probe.running')
          : t('home.components.connection.account.probe.run')}
      </Button>

      {error && (
        <Typography variant="body2" sx={{ color: 'error.main', mt: 1.5 }}>
          {error}
        </Typography>
      )}

      {result && (
        <Stack spacing={0.75} sx={{ mt: 1.5 }}>
          <Typography variant="body2">
            {/* The hub's own word, mapped to a sentence. An unrecognised value falls
                back to the unanswered sentence — never to absence. */}
            {t(livenessKey(result.liveness))}
          </Typography>
          <Typography variant="body2" sx={{ color: 'text.secondary' }}>
            {t('home.components.connection.account.probe.reasonLabel')}{' '}
            {/* The raw code, not a paraphrase: it is what the runbook is
                keyed on, and a student reading it out to support is the
                whole point of showing it. */}
            <Box component="code" sx={{ fontSize: '0.85em' }}>
              {result.reasonCode}
            </Box>
          </Typography>
        </Stack>
      )}
    </Paper>
  )
}
