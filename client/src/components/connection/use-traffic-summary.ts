import { useMemo } from 'react'

import { useConnectionSummaryData } from '@/hooks/use-connection-data'
import { useMemoryData } from '@/hooks/use-memory-data'
import { useTrafficData } from '@/hooks/use-traffic-data'
import { useVisibility } from '@/hooks/use-visibility'
import parseTraffic from '@/utils/parse-traffic'

/**
 * The traffic numbers, computed in exactly one place.
 *
 * Two screens show parts of this — Connection shows live speed, Account shows the
 * session totals — and the old component computed all of it together for a single
 * dashboard. Splitting the *rendering* is the point; splitting the *arithmetic*
 * would let the two screens disagree about the same session, which is the kind of
 * bug a user reports as "the numbers don't match" with no way for us to tell which
 * screen is lying.
 *
 * So: one hook, one subscription, one parse. The screens choose what to display.
 */
export interface TrafficSummary {
  /** Bytes per second, right now. */
  upSpeed: string
  upSpeedUnit: string
  downSpeed: string
  downSpeedUnit: string
  /**
   * The raw live speeds in bytes per second.
   *
   * Exposed so a *presentation* formatter can choose its own units without the
   * arithmetic being duplicated — see `utils/format-speed.ts`, which promotes
   * sub-kilobyte speeds to KB/s so a small live number does not read as broken.
   * `upSpeed`/`upSpeedUnit` remain the shared `parseTraffic` answer, used where
   * the graph and totals need the canonical units.
   */
  upSpeedBytes: number
  downSpeedBytes: number
  /**
   * Totals **since the core last started**, not since install.
   *
   * mihomo's counters live in the running process, so a disconnect/reconnect or
   * any core restart resets them to zero. The UI must label them accordingly —
   * see `TrafficSummaryCard`, which says "this session". Calling them "Uploaded"
   * and "Downloaded" with no qualifier is what makes a student think their usage
   * was lost.
   */
  uploaded: string
  uploadedUnit: string
  downloaded: string
  downloadedUnit: string
  /** Live count of open connections, or undefined while unknown. */
  activeConnections: number | undefined
  /**
   * The core's own memory use ("Core Usage"), or undefined while unknown.
   *
   * This is the running engine's footprint, not the machine's — it is the number
   * the original dashboard called "Core Usage" and the one that says "the engine
   * is alive and how much it is holding". Inherited from the upstream layout,
   * which shows the same figure in its status bar.
   */
  memory: string | undefined
  memoryUnit: string | undefined
}

const EMPTY: TrafficSummary = {
  upSpeed: '0',
  upSpeedUnit: 'B',
  downSpeed: '0',
  downSpeedUnit: 'B',
  upSpeedBytes: 0,
  downSpeedBytes: 0,
  uploaded: '0',
  uploadedUnit: 'B',
  downloaded: '0',
  downloadedUnit: 'B',
  activeConnections: undefined,
  memory: undefined,
  memoryUnit: undefined,
}

export const useTrafficSummary = (options?: {
  enabled?: boolean
}): TrafficSummary => {
  const enabled = options?.enabled ?? true

  // Gate on page visibility at the call site, so a backgrounded window stops
  // polling. The hook honours the same flag for the connection summary query.
  const pageVisible = useVisibility()
  const active = enabled && pageVisible

  const {
    response: { data: traffic },
  } = useTrafficData({ enabled: active })

  const {
    response: { data: connectionSummary },
  } = useConnectionSummaryData({ enabled: active })

  const {
    response: { data: memory },
  } = useMemoryData({ enabled: active })

  return useMemo(() => {
    if (!traffic) return EMPTY

    const [up, upUnit] = parseTraffic(traffic.up ?? 0)
    const [down, downUnit] = parseTraffic(traffic.down ?? 0)
    const [uploaded, uploadedUnit] = parseTraffic(traffic.upTotal ?? 0)
    const [downloaded, downloadedUnit] = parseTraffic(traffic.downTotal ?? 0)
    // `inuse` is 0 until the first memory frame arrives, which is a real reading
    // ("not yet known") rather than "0 bytes used" — so it is only reported once
    // it is a positive number, avoiding a "0 B" core-usage line on first paint.
    const hasMemory = typeof memory?.inuse === 'number' && memory.inuse > 0
    const [inuse, inuseUnit] = hasMemory
      ? parseTraffic(memory.inuse)
      : ['0', 'B']

    return {
      upSpeed: String(up),
      upSpeedUnit: upUnit,
      downSpeed: String(down),
      downSpeedUnit: downUnit,
      upSpeedBytes: traffic.up ?? 0,
      downSpeedBytes: traffic.down ?? 0,
      uploaded: String(uploaded),
      uploadedUnit,
      downloaded: String(downloaded),
      downloadedUnit,
      activeConnections: connectionSummary?.activeConnectionCount,
      memory: hasMemory ? String(inuse) : undefined,
      memoryUnit: hasMemory ? inuseUnit : undefined,
    }
  }, [traffic, connectionSummary, memory])
}
