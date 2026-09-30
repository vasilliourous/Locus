import { useLocalStorage } from 'foxact/use-local-storage'
import { useCallback, useEffect, useRef } from 'react'
import { type Message, type MihomoWebSocket } from 'tauri-plugin-mihomo-api'

import {
  removeCacheData,
  setCacheData,
  useQuery,
} from '@/services/query-client'
import { createCoalescer, type Coalescer } from '@/utils/coalescer'

const RECONNECT_DELAY_MS = 1000

interface SharedSubscriptionOwner {
  handleMessage: (data: string) => void
  onConnected?: (ws: MihomoWebSocket) => Promise<void> | void
  cleanup?: () => void
  isMounted: () => boolean
}

interface SharedSubscriptionEntry {
  refs: number
  ws: MihomoWebSocket | null
  reconnectTimer: ReturnType<typeof setTimeout> | null
  connecting: boolean
  owners: Set<SharedSubscriptionOwner>
  activeOwner: SharedSubscriptionOwner | null
  closed: boolean
  connectWs: () => Promise<void>
  scheduleReconnect: () => Promise<void>
}

const sharedSubscriptions = new Map<string, SharedSubscriptionEntry>()
const subscriptionSnapshots = new Map<string, unknown>()
const initialSubscriptionDate = Date.now()

const getSubscriptionSnapshot = <T>(key: string) =>
  subscriptionSnapshots.get(key) as T | undefined

const writeSubscriptionSnapshot = <T>(key: string, data: T) => {
  subscriptionSnapshots.set(key, data)
  void setCacheData<T>([key], data)
}

const pickActiveOwner = (entry: SharedSubscriptionEntry) => {
  if (entry.activeOwner?.isMounted()) return entry.activeOwner

  for (const owner of entry.owners) {
    if (owner.isMounted()) {
      entry.activeOwner = owner
      return owner
    }
  }

  entry.activeOwner = null
  return null
}

const closeSharedSocket = async (entry: SharedSubscriptionEntry) => {
  const ws = entry.ws
  if (!ws) return

  entry.ws = null
  await ws.close()
}

const createSharedSubscriptionEntry = (
  connect: () => Promise<MihomoWebSocket>,
): SharedSubscriptionEntry => {
  const entry: SharedSubscriptionEntry = {
    refs: 0,
    ws: null,
    reconnectTimer: null,
    connecting: false,
    owners: new Set(),
    activeOwner: null,
    closed: false,
    connectWs: async () => {},
    scheduleReconnect: async () => {},
  }

  const clearReconnectTimer = () => {
    if (entry.reconnectTimer) {
      clearTimeout(entry.reconnectTimer)
      entry.reconnectTimer = null
    }
  }

  entry.connectWs = async () => {
    if (entry.closed || entry.connecting || entry.ws) return

    entry.connecting = true
    try {
      const ws = await connect()
      if (entry.closed) {
        await ws.close()
        return
      }

      const owner = pickActiveOwner(entry)
      await owner?.onConnected?.(ws)
      if (entry.closed) {
        await ws.close()
        return
      }

      ws.addListener((msg: Message) => {
        if (msg.type !== 'Text') return
        const activeOwner = pickActiveOwner(entry)
        if (!activeOwner) return

        activeOwner.handleMessage(msg.data)
      })

      entry.ws = ws
      clearReconnectTimer()
    } catch (ignoreError) {
      if (!entry.closed && !entry.ws) {
        clearReconnectTimer()
        entry.reconnectTimer = setTimeout(entry.connectWs, RECONNECT_DELAY_MS)
      }
    } finally {
      entry.connecting = false
    }
  }

  entry.scheduleReconnect = async () => {
    if (entry.closed) return

    clearReconnectTimer()
    await closeSharedSocket(entry)
    if (!entry.closed) {
      entry.reconnectTimer = setTimeout(entry.connectWs, RECONNECT_DELAY_MS)
    }
  }

  return entry
}

/**
 * Mirrors SWR's MutatorCallback: consumers can pass either a plain value or a
 * functional updater `(current?: T) => T`.  The functional form is resolved
 * against the current subscription snapshot before updating SWR.
 */
type NextFn<T> = (
  error?: any,
  data?: T | ((current?: T) => T | undefined),
) => void

interface HandlerContext<T> {
  next: NextFn<T>
  scheduleReconnect: () => Promise<void>
  isMounted: () => boolean
}

interface HandlerResult {
  handleMessage: (data: string) => void
  onConnected?: (ws: MihomoWebSocket) => Promise<void> | void
  cleanup?: () => void
}

interface UseMihomoWsSubscriptionOptions<T> {
  storageKey: string
  buildSubscriptKey: (date: number) => string | null
  fallbackData: T
  connect: () => Promise<MihomoWebSocket>
  /**
   * When > 0, coalesce rapid WebSocket messages by wrapping the `next`
   * function passed to `setupHandlers`.  Only the most recent value is
   * flushed, at most once per `throttleMs` milliseconds.
   *
   * Uses `setTimeout` (not `requestAnimationFrame`) so it keeps working
   * when the window is backgrounded or minimized.
   */
  throttleMs?: number
  setupHandlers: (ctx: HandlerContext<T>) => HandlerResult
}

export const useMihomoWsSubscription = <T>(
  options: UseMihomoWsSubscriptionOptions<T>,
) => {
  const {
    storageKey,
    buildSubscriptKey,
    fallbackData,
    connect,
    throttleMs,
    setupHandlers,
  } = options

  const [date, setDate] = useLocalStorage(storageKey, initialSubscriptionDate)
  const subscriptKey = buildSubscriptKey(date)
  const subscriptionCacheKey = subscriptKey ? `$sub$${subscriptKey}` : null
  const lastSubscriptionCacheKeyRef = useRef<string | null>(null)
  if (subscriptionCacheKey) {
    lastSubscriptionCacheKeyRef.current = subscriptionCacheKey
  }
  const responseCacheKey =
    subscriptionCacheKey ?? lastSubscriptionCacheKeyRef.current

  const resolveNextData = useCallback(
    (
      data: T | ((current?: T) => T | undefined) | undefined,
      cacheKey: string,
    ): T => {
      if (typeof data === 'function') {
        const updater = data as (current?: T) => T | undefined
        const current = getSubscriptionSnapshot<T>(cacheKey)
        return updater(current) ?? fallbackData
      }
      return data ?? fallbackData
    },
    [fallbackData],
  )

  const response = useQuery<T>({
    queryKey: responseCacheKey ? [responseCacheKey] : ['$sub$__disabled__'],
    queryFn: () =>
      getSubscriptionSnapshot<T>(responseCacheKey!) ?? fallbackData,
    initialData: () =>
      getSubscriptionSnapshot<T>(responseCacheKey ?? '$sub$__disabled__') ??
      fallbackData,
    staleTime: Infinity,
    enabled: subscriptionCacheKey !== null,
  })

  useEffect(() => {
    if (!subscriptionCacheKey) return

    let isMounted = true
    let entry = sharedSubscriptions.get(subscriptionCacheKey)
    if (!entry) {
      entry = createSharedSubscriptionEntry(connect)
      sharedSubscriptions.set(subscriptionCacheKey, entry)
    }

    entry.refs += 1

    let throttleCleanup: (() => void) | undefined
    let wrappedNext: NextFn<T>

    const baseNext: NextFn<T> = (error, data) => {
      if (error !== undefined && error !== null) {
        return
      }
      if (data === undefined) return
      const resolved = resolveNextData(data, subscriptionCacheKey)
      writeSubscriptionSnapshot(subscriptionCacheKey, resolved)
    }

    if (throttleMs && throttleMs > 0) {
      // The scheduling rule lives in `utils/coalescer.ts` so it can be tested
      // against a fake clock — the version that was inline here was wrong in a way
      // no real-time test could see (see that module's header for the full story:
      // it re-armed its window on every message, so it bounded nothing, and the
      // value that survived was an arbitrary sample of the burst rather than the
      // newest one).
      //
      // What this wrapper adds over the coalescer is the error path: an error is
      // NEVER held. Dropping the last error in a burst would leave the UI showing
      // a stale reading with no way to learn it was wrong, and it must not be
      // reordered behind a value that was received before it.
      let coalescer: Coalescer<T | ((current?: T) => T | undefined)> | null =
        null

      wrappedNext = (
        error?: any,
        data?: T | ((current?: T) => T | undefined),
      ) => {
        if (error !== undefined && error !== null) {
          // Drop anything held rather than emitting it first: the error
          // supersedes a reading that is already out of date.
          coalescer?.cancel()
          coalescer = null
          baseNext(error, data)
          return
        }

        // `undefined` is "no payload", which upstream treats as nothing to report
        // — see `baseNext`, which returns early for it. Filtered here rather than
        // pushed, so an empty frame cannot displace a real reading that is already
        // held in the current window.
        if (data === undefined) return

        coalescer ??= createCoalescer<T | ((current?: T) => T | undefined)>(
          throttleMs,
          (value) => baseNext(undefined, value),
        )
        coalescer.push(data)
      }

      throttleCleanup = () => {
        coalescer?.cancel()
        coalescer = null
      }
    } else {
      wrappedNext = baseNext
    }

    const {
      handleMessage: handleTextMessage,
      onConnected,
      cleanup,
    } = setupHandlers({
      next: wrappedNext,
      scheduleReconnect: entry.scheduleReconnect,
      isMounted: () => isMounted,
    })

    const owner: SharedSubscriptionOwner = {
      handleMessage: handleTextMessage,
      onConnected,
      cleanup: () => {
        throttleCleanup?.()
        cleanup?.()
      },
      isMounted: () => isMounted,
    }

    entry.owners.add(owner)
    if (!entry.activeOwner) {
      entry.activeOwner = owner
    }
    void entry.connectWs()

    return () => {
      isMounted = false
      entry.owners.delete(owner)
      owner.cleanup?.()

      if (entry.activeOwner === owner) {
        entry.activeOwner = null
        const nextOwner = pickActiveOwner(entry)
        if (entry.ws && nextOwner?.onConnected) {
          void nextOwner.onConnected(entry.ws)
        }
      }

      entry.refs -= 1
      if (entry.refs <= 0) {
        entry.closed = true
        if (entry.reconnectTimer) {
          clearTimeout(entry.reconnectTimer)
          entry.reconnectTimer = null
        }
        sharedSubscriptions.delete(subscriptionCacheKey)
        void closeSharedSocket(entry)
      }
    }
    // eslint-disable-next-line react-compiler/react-compiler
    // eslint-disable-next-line react-hooks/exhaustive-deps, @eslint-react/exhaustive-deps
  }, [subscriptionCacheKey])

  const refresh = useCallback(() => {
    if (subscriptionCacheKey) {
      subscriptionSnapshots.delete(subscriptionCacheKey)
      void removeCacheData([subscriptionCacheKey])
    }
    setDate(Date.now())
  }, [subscriptionCacheKey, setDate])

  const setData = useCallback(
    (data: T) => {
      if (responseCacheKey) {
        writeSubscriptionSnapshot(responseCacheKey, data)
      }
    },
    [responseCacheKey],
  )

  return { response, refresh, setData }
}
