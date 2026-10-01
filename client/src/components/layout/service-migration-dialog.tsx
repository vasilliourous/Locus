import { Alert } from '@mui/material'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog } from '@/components/base'
import { runStateQueryKey } from '@/hooks/use-system-state'
import {
  getRuntimeState,
  installService,
  reinstallService,
  repairService,
  restartCore,
  type RunState,
} from '@/services/cmds'
import { showNotice } from '@/services/notice-service'
import { setCacheData, useQuery } from '@/services/query-client'

export const ServiceMigrationDialog = () => {
  const { t } = useTranslation()
  const [loading, setLoading] = useState(false)
  const [stateRefreshFailed, setStateRefreshFailed] = useState(false)
  const [workflowIncomplete, setWorkflowIncomplete] = useState(false)
  const { data: runState } = useQuery({
    queryKey: runStateQueryKey,
    queryFn: getRuntimeState,
    enabled: true,
    retry: 1,
    // No `refetchInterval` here, deliberately.
    //
    // This ran its own 30 s poll on the same cache key as `useSystemState` —
    // a second poller of one key, on a screen the student is not looking at,
    // for a value that already arrives by event (`verge://run-state-changed`
    // carries a full snapshot and `use-layout-events` writes it into this entry).
    // `useSystemState` owns the read path for this key; a dialog that also polls
    // it doubles the IPC for no extra information and makes the two disagree
    // about when the state is fresh.
    //
    // The dialog still re-reads on its own terms: `refreshRunState` below is
    // called after every remedy, which is the moment the value actually changes.
  })
  // Whether the service needs a decision is derived once, in Rust, and travels with the
  // snapshot; a failed refresh is treated as needing one, since we cannot tell otherwise.
  const needsDecision =
    stateRefreshFailed || Boolean(runState?.serviceNeedsAttention)
  // Treat refresh failures as unreachable; an absent Service still needs install.
  const remedy: 'install' | 'repair' | 'reinstall' =
    runState?.pendingAction === 'install'
      ? 'install'
      : stateRefreshFailed || runState?.service === 'unavailable'
        ? 'repair'
        : runState?.service === 'notInstalled'
          ? 'install'
          : 'reinstall'
  const open = loading || workflowIncomplete || needsDecision
  const checking =
    loading ||
    !runState ||
    runState.opInFlight ||
    runState.service === 'unknown'
  const showCheckingMessage = checking || !needsDecision

  // One cache entry to refresh, so there is nothing left to keep coherent by hand.
  const refreshRunState = async () => {
    try {
      const data = await getRuntimeState()
      await setCacheData<RunState>(runStateQueryKey, data)
      setStateRefreshFailed(false)
      return data
    } catch (error) {
      setStateRefreshFailed(true)
      throw error
    }
  }

  const handleServiceAction = async () => {
    setLoading(true)
    setWorkflowIncomplete(true)
    let actionSucceeded = false
    try {
      if (remedy === 'install') {
        await installService()
      } else if (remedy === 'repair') {
        await repairService()
      } else {
        await reinstallService()
      }
      actionSucceeded = true
    } catch (error) {
      showNotice.error(
        'layout.components.serviceMigration.errors.actionFailed',
        error,
      )
    }

    let initialRefreshSucceeded = false
    try {
      await refreshRunState()
      initialRefreshSucceeded = true
    } catch (error) {
      showNotice.error(
        'layout.components.serviceMigration.errors.stateRefreshFailed',
        error,
      )
    }
    if (!actionSucceeded || !initialRefreshSucceeded) {
      setLoading(false)
      return
    }

    let restartSucceeded = false
    try {
      await restartCore()
      restartSucceeded = true
    } catch (error) {
      showNotice.error(
        'layout.components.serviceMigration.errors.restartFailed',
        error,
      )
    }

    let finalRefreshSucceeded = false
    try {
      await refreshRunState()
      finalRefreshSucceeded = true
    } catch (error) {
      showNotice.error(
        'layout.components.serviceMigration.errors.stateRefreshFailed',
        error,
      )
    }
    if (restartSucceeded && finalRefreshSucceeded) {
      setWorkflowIncomplete(false)
      showNotice.success('layout.components.serviceMigration.success')
    }
    setLoading(false)
  }

  return (
    <BaseDialog
      open={open}
      title={t('layout.components.serviceMigration.title')}
      okBtn={t(
        remedy === 'install'
          ? 'settings.sections.proxyControl.actions.installService'
          : remedy === 'repair'
            ? 'layout.components.serviceMigration.repair'
            : 'layout.components.serviceMigration.reinstall',
      )}
      disableOk={checking}
      loading={loading}
      onOk={() => void handleServiceAction()}
    >
      <Alert severity="warning">
        {t(
          showCheckingMessage
            ? 'layout.components.serviceMigration.checkingMessage'
            : remedy === 'reinstall'
              ? 'layout.components.serviceMigration.message'
              : 'layout.components.serviceMigration.unavailableMessage',
        )}
      </Alert>
    </BaseDialog>
  )
}
