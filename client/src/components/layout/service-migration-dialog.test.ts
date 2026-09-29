import { useState } from 'react'
import { beforeEach, expect, it, vi } from 'vitest'

import type { RunState } from '@/services/cmds'
import { useQuery } from '@/services/query-client'

import { ServiceMigrationDialog } from './service-migration-dialog'

vi.mock('react', () => ({ useState: vi.fn() }))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('@mui/material', () => ({ Alert: 'div' }))
vi.mock('@/components/base', () => ({ BaseDialog: 'dialog' }))
vi.mock('@/hooks/use-visibility', () => ({ useVisibility: () => true }))
vi.mock('@/hooks/use-system-state', () => ({ runStateQueryKey: ['state'] }))
vi.mock('@/services/cmds', () => ({ getRuntimeState: vi.fn() }))
vi.mock('@/services/notice-service', () => ({ showNotice: {} }))
vi.mock('@/services/query-client', () => ({
  useQuery: vi.fn(),
  setCacheData: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(useState)
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([false, vi.fn()])
    .mockReturnValueOnce([true, vi.fn()])
})

/**
 * The dialog is the whole recovery story now.
 *
 * The client used to offer "Continue with Sidecar" as a way past a Service it
 * could not fix, which meant a broken Service was a choice rather than a
 * problem. The Service is the only supported route to a tunnel, so every state
 * that needs attention must present a repair and no way around it — asserted
 * here as "no cancel button", because that button was the escape hatch.
 */
it.each([
  { service: 'ready', mode: 'NotRunning' },
  { service: 'unknown', mode: 'NotRunning' },
  { service: 'unavailable', mode: 'Service' },
  { service: 'unavailable', mode: 'NotRunning', opInFlight: true },
  { service: 'unavailable', mode: 'NotRunning', pendingAction: 'reinstall' },
] satisfies Partial<RunState>[])(
  'offers no way past the service from %j',
  (state) => {
    vi.mocked(useQuery).mockReturnValue({ data: state } as ReturnType<
      typeof useQuery
    >)
    const dialog = ServiceMigrationDialog()
    expect(dialog.props.open).toBe(true)
    expect(dialog.props.cancelBtn).toBeUndefined()
  },
)

it.each([
  { service: 'notInstalled', mode: 'NotRunning' },
  { service: 'versionMismatch', mode: 'NotRunning' },
  { service: 'unavailable', mode: 'NotRunning' },
  { service: 'notInstalled', mode: 'NotRunning', pendingAction: 'install' },
] satisfies Partial<RunState>[])(
  'offers no way past the service from %j either',
  (state) => {
    vi.mocked(useQuery).mockReturnValue({ data: state } as ReturnType<
      typeof useQuery
    >)
    expect(ServiceMigrationDialog().props.cancelBtn).toBeUndefined()
  },
)
