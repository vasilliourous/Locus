import { useCallback } from 'react'

import { uninstallService } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

import { useSystemState } from './use-system-state'

/**
 * Removes the service and re-reads the run state.
 *
 * Named for the uninstall alone: it used to also bring a sidecar up in the same
 * step, and there is no longer a second run mode to fall back to. Removing the
 * service now stops the Core, which is the honest consequence of the service
 * being the only supported way to run it.
 */
export const useServiceUninstaller = () => {
  const { mutateSystemState } = useSystemState()

  const uninstallServiceAndRefresh = useCallback(async () => {
    let uninstallError: unknown
    showNotice.info('settings.statuses.clashService.uninstalling')
    try {
      await uninstallService()
      showNotice.success(
        'settings.feedback.notifications.clashService.uninstallSuccess',
      )
    } catch (error) {
      uninstallError = error
    }

    try {
      await mutateSystemState()
    } catch (error) {
      if (!uninstallError) throw error
    }

    if (uninstallError) throw uninstallError
  }, [mutateSystemState])

  return { uninstallServiceAndRefresh }
}
