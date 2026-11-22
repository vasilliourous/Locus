import { Box, Button, CircularProgress, Typography, alpha } from '@mui/material'
import { useTheme } from '@mui/material/styles'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SupportBlock } from '@/components/shared/support-block'
import { useSubscription } from '@/hooks/use-subscription'
import { monoFontFamily } from '@/pages/_surfaces'
import { activationRefusal } from '@/pages/connect-notice'
import {
  chrysalisActivate,
  chrysalisCheckCode,
  chrysalisStatus,
  chrysalisValidateCode,
  type ActivationResult,
  type ChrysalisStatus,
  type ValidateCodeResult,
} from '@/services/chrysalis'
import { errorDetail } from '@/services/notice-service'

/**
 * First-run activation gate.
 *
 * The app renders this and NOTHING else until a code is accepted. That is the
 * whole product decision: a Chrysalis student does not configure Clash, choose a
 * profile or pick a node — they enter the code on the card they were sold.
 *
 * Behaviour deliberately preserved from the retired client, because it was
 * learned from real support conversations:
 *
 *  * The checksum is validated LOCALLY as you type. `/api/activate` allows 5
 *    attempts per 10 minutes per address, so four typos would lock a student out
 *    of the one screen that could explain their mistake. A local check costs
 *    nothing and never counts against them.
 *
 *  * Formatting is normalised before anything is sent. A code is stored and
 *    looked up hyphenated, so a paste without hyphens would 404 and read as
 *    "your code is not recognised" — which is the single most annoying failure
 *    this screen can produce.
 *
 *  * "Already used on THIS device" is a success, not an error. Students
 *    reinstall, and re-pasting their own code must not tell them it is dead.
 */

type Phase = 'idle' | 'checking' | 'activating' | 'done'

const CODE_LENGTH = 15 // RQ + 3x4 + checksum, hyphens excluded
/** Strips formatting and uppercases, mirroring the backend's normalisation. */
const clean = (input: string) => input.replace(/[^a-z0-9]/gi, '').toUpperCase()

/** Groups a bare code into the hyphenated form as the student types. */
const present = (input: string) => {
  const raw = clean(input)
  const parts = [
    raw.slice(0, 2),
    raw.slice(2, 6),
    raw.slice(6, 10),
    raw.slice(10, 14),
    raw.slice(14, 15),
  ]
  return parts.filter(Boolean).join('-')
}

interface Props {
  onActivated: (result: ActivationResult) => void
}

const ActivationScreen = ({ onActivated }: Props) => {
  const { t } = useTranslation()
  const theme = useTheme()
  const [input, setInput] = useState('')
  const [phase, setPhase] = useState<Phase>('idle')
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  // `null` means "not checked yet". Derived state holds the answer rather than
  // being pushed into another state variable on every keystroke.
  const [check, setCheck] = useState<ValidateCodeResult | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  // Why this device has no entitlement, when the hub refused it.
  //
  // This screen is what a refused device renders — the refusal cleared the
  // entitlement, so `main.tsx` gates to here — which means it is the ONLY place
  // the reason can be told to a student. It used to be shown on the connection
  // page, where it was unreachable precisely when it mattered.
  //
  // The hub's own sentence, rendered verbatim and never paraphrased: the operator
  // writes an instruction ("contact your middleman", "renew your code"), and
  // rewording it here would replace an action with a description.
  const { subscription } = useSubscription()
  const refusal = activationRefusal(subscription)

  // One field, read once on mount, only to say whether the backup route is
  // carrying this screen's requests. Deliberately not polled and not a second
  // state system: the screen's job is the code, and this is a caption.
  const [status, setStatus] = useState<ChrysalisStatus | null>(null)
  useEffect(() => {
    let cancelled = false
    void chrysalisStatus().then((value) => {
      if (!cancelled) setStatus(value)
    })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // Local checksum check, run only once the code is the right length.
  //
  // Set from inside the async callback rather than synchronously in the effect
  // body: writing state during an effect causes an extra render on every
  // keystroke, and the incomplete-code case needs no state at all — it is
  // simply "no check has run yet", which the derived values below already
  // express.
  useEffect(() => {
    const raw = clean(input)
    if (raw.length !== CODE_LENGTH) return

    let cancelled = false
    chrysalisValidateCode(raw)
      .then((result) => {
        if (!cancelled) setCheck(result)
      })
      .catch(() => {
        if (!cancelled) setCheck(null)
      })

    return () => {
      cancelled = true
    }
  }, [input])

  const raw = clean(input)
  const complete = raw.length === CODE_LENGTH

  // The check belongs to THIS input only. While the student is still typing, a
  // verdict about a previous, longer code must not colour the field.
  const verdict = complete && check?.canonical === present(raw) ? check : null
  const localValid = verdict?.valid === true

  // A malformed-code complaint is shown only once the code is complete, so the
  // student is not scolded mid-typing. Anything raised by activation itself
  // (`error`) takes precedence and is shown regardless.
  const shownError = error ?? (complete ? (verdict?.message ?? null) : null)

  const canSubmit = localValid && phase === 'idle'

  const submit = useCallback(async () => {
    const raw = clean(input)
    if (raw.length !== CODE_LENGTH) return

    setError(null)
    setNotice(null)

    // Ask the hub whether the code is usable BEFORE activating. This is what
    // turns "activation failed" into a specific reason — "this code has already
    // been activated", "this code has been suspended", "the code has expired" —
    // and it costs one read-only request instead of a failed activation attempt
    // that counts against the rate limit.
    setPhase('checking')
    try {
      const check = await chrysalisCheckCode(raw)
      if (!check.ready) {
        setError(check.message)
        setPhase('idle')
        return
      }
      // A code the student has already used is still activatable — doing so
      // restores their access — and the hub's own sentence says so. Show that
      // rather than the generic tier line: "already activated, entering it
      // again will restore your access" is the reassurance a returning student
      // needs, and it is the whole point of the single-use model.
      setNotice(
        check.tier
          ? `Ready — this code gives you the ${check.tier} tier`
          : check.message,
      )
    } catch {
      // The lookup is advisory. If it cannot be reached we still try to
      // activate, because the hub will make the same call anyway and refusing
      // here would block a student on a flaky connection for no reason.
      setNotice(null)
    }

    setPhase('activating')
    try {
      const result = await chrysalisActivate(raw)
      setPhase('done')
      onActivated(result)
    } catch (err) {
      // `errorDetail`, not `String`: a Rust failure arrives as
      // `CommandFailure { code, detail }`, so `String(err)` renders as
      // "[object Object]" and throws away the only useful sentence — "this code
      // has already been activated", "the code has expired", and so on.
      setError(errorDetail(err))
      setPhase('idle')
    }
  }, [input, onActivated])

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Enter' && canSubmit) void submit()
  }

  const busy = phase === 'checking' || phase === 'activating'

  return (
    <Box
      sx={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        // ── The theme, inherited, not restated ────────────────────────────
        //
        // This screen used to render OUTSIDE the theme provider, so it got a
        // bare `createTheme()`: MUI's stock blue button and type scale, over a
        // page whose colours were written from the palette constants by hand.
        // That pairing once made every label invisible while present and
        // correct in the DOM. `main.tsx` now mounts the provider above the
        // activation gate, so this screen is themed like every other one and
        // there is nothing left to state here.
        //
        // It is the highest-value surface in the product: the first thing a
        // student sees, and the only screen a person who has not paid yet will
        // ever look at.
        bgcolor: 'background.default',
        color: 'text.primary',
      }}
    >
      <Box
        sx={{ width: 420, maxWidth: 'calc(100vw - 48px)', textAlign: 'center' }}
      >
        {/* Brand mark. There is no Chrysalis pictorial mark in the repo, so the mark
            is the wordmark alone — deliberately, rather than substituting
            another product's logo. See `/COLOURS.md` §8. */}
        <Typography
          variant="h4"
          sx={{ fontWeight: 700, letterSpacing: '-0.01em', mb: 0.5 }}
          color="primary.main"
        >
          Chrysalis
        </Typography>
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ mb: 4, letterSpacing: '0.01em' }}
        >
          {t('home.components.connection.activation.title')}
        </Typography>

        {/* The maintenance tunnel, said plainly while it is carrying this
            screen's own requests.

            This screen makes the two hub calls that enrolment depends on, and on
            a network that blocks the hub by name those calls travel through the
            free-tier relay. Without a line saying so, the only visible effect is
            that enrolment works — or, when it does not, that it fails with no
            indication of which route was tried.
            One line, from the backend's own verdict (`ChrysalisStatus.bootstrapping`),
            never inferred here. */}
        {status?.bootstrapping && (
          <Typography variant="caption" color="text.secondary" sx={{ mb: 2 }}>
            {t('home.components.connection.activation.backupRoute')}
          </Typography>
        )}

        {/* A refusal, said before anything else on the screen.
            A suspended or expired device lands here with no other explanation:
            the entitlement is gone, so there is no tier, no date, and no
            connection page to carry a warning. Without this the student is asked
            for a code with no hint that their account was ended — which is the
            "no indication that it's happened" report.

            Above the prompt, not below it, because it changes what the student
            should do next: entering the same code again will simply be refused
            again. */}
        {refusal && (
          <Box
            sx={{
              width: '100%',
              boxSizing: 'border-box',
              mb: 2.5,
              px: 2,
              py: 1.5,
              borderRadius: 1.5,
              border: '1px solid',
              borderColor: 'error.main',
              bgcolor: alpha(theme.palette.error.main, 0.12),
            }}
          >
            <Typography
              variant="body2"
              color="error.main"
              sx={{ lineHeight: 1.5, fontWeight: 500 }}
            >
              {refusal}
            </Typography>
          </Box>
        )}

        <Typography variant="body2" sx={{ mb: 2.5, fontWeight: 500 }}>
          {t('home.components.connection.activation.lede')}
        </Typography>

        <input
          ref={inputRef}
          value={present(input)}
          onChange={(event) => {
            // An event handler is the right place for this: the previous
            // verdict no longer describes what is in the field.
            setCheck(null)
            setError(null)
            setInput(event.target.value)
          }}
          onKeyDown={onKeyDown}
          disabled={busy}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="characters"
          placeholder={t(
            'home.components.connection.activation.codePlaceholder',
          )}
          aria-label={t('home.components.connection.activation.codeLabel')}
          style={{
            width: '100%',
            boxSizing: 'border-box',
            padding: '16px 18px',
            fontSize: 20,
            letterSpacing: '0.08em',
            fontFamily: monoFontFamily,
            textAlign: 'center',
            textTransform: 'uppercase',
            borderRadius: theme.shape.borderRadius,
            // The verdict colours are the theme's status roles, read through the
            // provider this screen now sits under. Before that it was OUTSIDE
            // the provider, so `palette.divider` was the light theme's divider —
            // nearly invisible on a dark page — which is why the colours were
            // written from the constants by hand.
            border: `1px solid ${
              shownError
                ? theme.palette.error.main
                : localValid
                  ? theme.palette.success.main
                  : theme.palette.divider
            }`,
            // A filled field rather than a transparent one: on a dark surface a
            // transparent input reads as a bare underline and looks broken.
            background: theme.palette.background.paper,
            color: theme.palette.text.primary,
            transition: 'border-color 0.15s ease-out',
          }}
        />

        {/* Reserved space, so the button does not jump as messages appear. */}
        <Box
          sx={{
            minHeight: 44,
            mt: 1.5,
            display: 'flex',
            alignItems: 'flex-start',
            justifyContent: 'center',
          }}
        >
          {shownError && (
            <Typography
              variant="body2"
              color="error.main"
              sx={{ lineHeight: 1.4 }}
            >
              {shownError}
            </Typography>
          )}
          {!shownError && notice && (
            <Typography
              variant="body2"
              color="success.main"
              sx={{ lineHeight: 1.4 }}
            >
              {notice}
            </Typography>
          )}
        </Box>

        <Button
          fullWidth
          size="large"
          variant="contained"
          disabled={!canSubmit}
          onClick={() => void submit()}
          sx={{ mt: 1, py: 1.4, fontWeight: 600, fontSize: 16 }}
        >
          {busy ? (
            <CircularProgress size={22} color="inherit" />
          ) : (
            t('home.components.connection.activation.submit')
          )}
        </Button>

        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', mt: 3, mb: 2 }}
        >
          {t('home.components.connection.activation.alphabet')}
        </Typography>

        {/* The support route, on the one screen a stuck student is guaranteed to
            be looking at.
            //
            // This space used to be empty ON PURPOSE, and the comment that stood
            // here argued for it: the hub host is infrastructure and must not be
            // shown, and "the card is the support route — the code is printed on
            // it, beside whatever contact details the reseller puts there".
            //
            // The first half is still right and the link was never restored: the
            // hub is an address the client CALLS, not product surface, and a
            // hostname cannot answer "my code says it is already used".
            //
            // The second half was wrong about this screen in particular. It
            // assumed every student still has the card in front of them, and the
            // student who arrives here is precisely the one who may not: a code
            // that was refused, a card lost, a device handed to them without it,
            // or a middleman they no longer have a number for. Telling them
            // nothing leaves the product's least self-sufficient screen as its
            // only dead end — and it is the screen that stands between a sale
            // and a paying customer.
            //
            // So the block names the middleman FIRST (the card's contact still
            // applies, and the middleman can fix most of this on the spot) and
            // then gives the operator's address and the reply promise for
            // everyone the middleman could not help. It deliberately does not
            // mention the hub. */}
        <SupportBlock />
      </Box>
    </Box>
  )
}

export default ActivationScreen
