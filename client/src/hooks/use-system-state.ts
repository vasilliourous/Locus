import {
  getRuntimeState,
  type RunState,
  type RunningMode,
} from '@/services/cmds'
import { useQuery } from '@/services/query-client'

export const runStateQueryKey = ['getRuntimeState'] as const

/** Fail closed until the first snapshot so TUN never flashes as available. */
const unknownRunState: RunState = {
  mode: 'NotRunning',
  service: 'unknown',
  serviceUnavailableReason: null,
  pendingAction: null,
  isAdmin: false,
  opInFlight: false,
  serviceUsable: false,
  tunCapable: false,
  serviceNeedsAttention: false,
}

/** Event-driven run state; Rust owns all derived availability decisions. */
export function useSystemState() {
  const {
    data: runState = unknownRunState,
    refetch: mutateSystemState,
    isLoading,
  } = useQuery({
    queryKey: runStateQueryKey,
    queryFn: getRuntimeState,
    // Event-driven: `verge://run-state-changed` carries a full snapshot on every
    // transition, and `use-layout-events` writes it straight into this cache key.
    // A focus or reconnect re-read covers the window where a transition happened
    // before the listener existed.
    //
    // There is deliberately NO interval here. There used to be a 30 s one,
    // described as "a safety net only" — but a poll that returns an equal
    // snapshot still produces a NEW object identity, and that identity is a
    // dependency of derived context values, so every 30 s the whole consumer tree
    // re-rendered to display exactly the same state. A safety net that costs a
    // re-render of the app is not free, and the event path plus the focus re-read
    // already close the race it was covering.
    refetchOnWindowFocus: true,
    refetchOnReconnect: true,
  })

  return {
    runState,
    runningMode: runState.mode as RunningMode,
    isAdminMode: runState.isAdmin,
    isServiceMode: runState.mode === 'Service',
    isTunModeAvailable: runState.tunCapable,
    serviceNeedsAttention: runState.serviceNeedsAttention,
    mutateSystemState,
    isLoading,
  }
}

// `useAppUptime` used to live here: a `getAppUptime` query on a 3-second
// interval, ungated by visibility, with no consumer anywhere in the app. It was
// inherited from the upstream dashboard's status bar, and the surfaces that showed
// it were removed with the Proxies/Logs/Settings pages.
//
// Deleted rather than kept for a future screen, because its cost was not zero and
// its benefit was zero: three IPC calls every three seconds, forever, on every
// page, to feed a number nothing rendered. If an uptime readout is wanted later,
// the Rust command is still registered (`get_app_uptime`, from the sysinfo
// plugin) and `getAppUptime` is still in `services/cmds.ts`; reinstating it is a
// small hook, and anyone doing so should gate it on visibility.
