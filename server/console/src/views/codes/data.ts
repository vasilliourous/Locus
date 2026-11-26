// Codes: the data layer.
//
// One composable owns every request the page makes, so the view can be about
// layout and the request/error/retry handling exists once rather than in a
// dozen handlers. The previous page called `load()` from ten places with no
// shared notion of "is a load in flight", which is how a stale response could
// land after a filter change and silently show the wrong rows.

import { ref } from 'vue'
import { call } from '../../api'
import { toast } from '../../toast'
import type { CodeRow, Facets, HistoryEvent } from './model'

export interface CodesQuery {
  query: string
  tier: string
  status: string
  middleman: string
  expiring_within_days: number
}

export function useCodes() {
  const rows = ref<CodeRow[]>([])
  const facets = ref<Facets>({ status: {}, tier: {} })
  const loading = ref(false)
  const failed = ref('')
  const denied = ref(false)
  /** Total in the CURRENT filter, which is what "200 of 214" needs. */
  const total = ref(0)
  let seq = 0

  async function load(q: CodesQuery) {
    const mine = ++seq
    loading.value = true
    failed.value = ''
    const res = await call<{
      codes: CodeRow[]
      total: number
      facets?: Facets
      expiring_within_days: number
    }>('codes.list', { ...q })
    // A response from an earlier query must not overwrite a newer one. Without
    // this, two quick filter clicks can show the first filter's rows under the
    // second filter's label.
    if (mine !== seq) return
    loading.value = false
    if (res.ok && res.data) {
      rows.value = res.data.codes || []
      total.value = res.data.total ?? rows.value.length
      if (res.data.facets) facets.value = res.data.facets
      return
    }
    if (res.message && /sign in again/i.test(res.message)) denied.value = true
    failed.value = res.message || res.transportError || 'The hub did not answer.'
    rows.value = []
    total.value = 0
  }

  async function history(code: string): Promise<HistoryEvent[]> {
    const res = await call<{ events: HistoryEvent[] }>('codes.history', { code })
    if (!res.ok || !res.data) {
      toast.err(res.message || res.transportError || 'Could not load the history')
      return []
    }
    return res.data.events || []
  }

  return { rows, facets, loading, failed, denied, total, load, history }
}

/**
 * Run a mutating action, toast the outcome, and report success.
 *
 * Every action went through this shape already (call → toast → reload) but
 * hand-rolled at each site, so the error text varied and one of them forgot to
 * reload. Centralising it means a new action cannot forget the reload.
 */
export async function act(
  action: string,
  payload: Record<string, unknown>,
  okText: string,
): Promise<boolean> {
  const res = await call(action, payload)
  if (res.ok) {
    if (okText) toast.ok(okText)
    return true
  }
  toast.err(res.message || res.transportError || 'That did not work')
  return false
}
