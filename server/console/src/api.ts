// API client for the Chrysalis admin console.
//
// The admin token is kept in sessionStorage, NOT localStorage: it should not
// outlive the browser session on a shared machine, and it is never baked into
// the built bundle (which is served publicly as a static file).

const TOKEN_KEY = 'chrysalis_admin_token'

export function getToken(): string {
  return sessionStorage.getItem(TOKEN_KEY) || ''
}

export function setToken(token: string) {
  sessionStorage.setItem(TOKEN_KEY, token)
}

export function clearToken() {
  sessionStorage.removeItem(TOKEN_KEY)
}

export function hasToken(): boolean {
  return !!getToken()
}

export interface ApiResult<T = Record<string, unknown>> {
  ok: boolean
  message?: string
  /** Present when the request itself failed (network / non-JSON response). */
  transportError?: string
  data?: T
}

/**
 * Call the admin API.
 *
 * Every console action funnels through here so auth handling, error shaping and
 * timeout behaviour stay in one place. The endpoint is deliberately a single
 * PB route with an `action` discriminator — one auth check, one place to audit.
 */
export async function call<T = Record<string, unknown>>(
  action: string,
  payload: Record<string, unknown> = {},
  timeoutMs = 30000,
): Promise<ApiResult<T>> {
  const token = getToken()
  if (!token) return { ok: false, message: 'Not signed in' }

  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const resp = await fetch('/api/admin/console', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        // Sent as a header so it never lands in a URL / access log.
        'X-Admin-Token': token,
      },
      body: JSON.stringify({ action, admin_token: token, ...payload }),
      signal: controller.signal,
    })

    const text = await resp.text()
    let parsed: Record<string, unknown> = {}
    try {
      parsed = text ? JSON.parse(text) : {}
    } catch {
      return {
        ok: false,
        transportError: `Server returned a non-JSON response (HTTP ${resp.status})`,
      }
    }

    if (resp.status === 403) {
      clearToken()
      return { ok: false, message: 'Your session is no longer valid — please sign in again.' }
    }
    if (!parsed.ok) {
      return { ok: false, message: (parsed.message as string) || `Request failed (HTTP ${resp.status})` }
    }
    return { ok: true, data: parsed as T }
  } catch (err) {
    const e = err as Error
    if (e.name === 'AbortError') {
      return { ok: false, transportError: 'The server took too long to respond.' }
    }
    return { ok: false, transportError: e.message || 'Network error' }
  } finally {
    clearTimeout(timer)
  }
}

/** Verify a token by making the cheapest possible authenticated call. */
export async function verifyToken(token: string): Promise<boolean> {
  try {
    const resp = await fetch('/api/admin/console', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Admin-Token': token },
      body: JSON.stringify({ action: 'dashboard', admin_token: token }),
    })
    if (!resp.ok) return false
    const json = await resp.json().catch(() => null)
    return !!json && json.ok === true
  } catch {
    return false
  }
}

export interface FetchArtifact {
  filename: string
  sha256: string
  bytes: number
  skipped?: boolean
  format?: string
}

export interface FetchResult {
  version: string
  dir: string
  elapsed?: number
  artifacts: Record<string, FetchArtifact>
}

/**
 * Ask the hub to pull a release straight from its GitHub Release.
 *
 * This replaces the old browser upload. The artifacts are fetched BY VERSION
 * from GitHub by the hub itself, so nothing large travels from this browser and
 * there is no directory for the operator to point at wrongly. The service
 * resolves assets by name, requires every platform, verifies each file's format
 * and hash, and publishes atomically — so a partial release cannot be produced
 * here.
 *
 * Uses a plain fetch (not XHR) because there is no upload to report progress
 * for; the response only arrives once every artifact is on disk and verified.
 */
export async function fetchRelease(version: string): Promise<ApiResult<FetchResult>> {
  const token = getToken()
  if (!token) return { ok: false, message: 'Not signed in' }

  try {
    const resp = await fetch('/api/admin/fetch-release', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Admin-Token': token },
      body: JSON.stringify({ version }),
    })
    const text = await resp.text()
    let parsed: Record<string, unknown> = {}
    try {
      parsed = text ? JSON.parse(text) : {}
    } catch {
      return { ok: false, transportError: `Fetch service returned a non-JSON response (HTTP ${resp.status})` }
    }
    if (!resp.ok || !parsed.ok) {
      return { ok: false, message: (parsed.message as string) || `Fetch failed (HTTP ${resp.status})` }
    }
    return { ok: true, data: parsed as unknown as FetchResult }
  } catch (err) {
    return { ok: false, transportError: (err as Error).message || 'Network error' }
  }
}

/**
 * Mint a fresh one-shot trigger link for a version.
 *
 * The link is single-use and expires. It exists so a fetch can be triggered
 * without the admin token — from a phone or a CI job — and it must be re-minted
 * for every run, which is what makes leaking one harmless.
 */
export function fetchLink(version: string): Promise<ApiResult<{ link: string; expires_in: number }>> {
  const url = `/api/admin/fetch-link?version=${encodeURIComponent(version)}`
  return fetch(url, { headers: { 'X-Admin-Token': getToken() } })
    .then(async (resp) => {
      const parsed = await resp.json().catch(() => ({}))
      if (!resp.ok || !parsed.ok) {
        return { ok: false, message: parsed.message || `Could not mint a link (HTTP ${resp.status})` } as ApiResult<{ link: string; expires_in: number }>
      }
      return { ok: true, data: parsed } as ApiResult<{ link: string; expires_in: number }>
    })
    .catch((err) => ({ ok: false, transportError: err.message || 'Network error' }))
}

/**
 * The hub's session probe, run by hand from the console.
 *
 * WHY THIS CALLS THE PUBLIC ENDPOINT INSTEAD OF ADDING AN ADMIN ACTION. The probe
 * already exists (`server/pb_hooks/session_probe.pb.js`) and is the one piece of
 * code the runbook tells an operator to trust. Re-implementing it behind
 * `/api/admin/console` would create a second implementation of the entitlement ×
 * liveness rule — the exact class of drift this repository has paid for. So the
 * console asks the SAME endpoint a client does, using the code and the device id
 * the row already carries.
 *
 * The probe is rate-limited (10 per 10 minutes per IP) and answers 404 to a
 * caller that cannot present the machine, 409 to an unsupported `probe_version`,
 * and 400 without one. All of those are returned as a legible message rather
 * than thrown, because "the probe said no" is an answer the operator asked for.
 */
export interface ProbeResponse {
  probe_version: number
  server_time: string
  /** active | released-unclaimed | suspended | expired | never-activated */
  entitlement: string
  /** live | stale | absent | unknown — `unknown` is NOT `absent`. */
  liveness: string
  hub_view: {
    activated_at: string | null
    released_at: string | null
    released_pending: boolean
    suspended: boolean
    suspended_reason: string | null
    expires_at: string | null
    tier: string
    last_seen_at: string | null
    last_seen_age_s: number | null
    first_seen_fingerprint: string
  }
  client_view: Record<string, unknown> | null
  lease_expires_at: string | null
  reason_code: string
  divergence: string[]
  /**
   * The operator's decision when one is in force. `null` is the ordinary case.
   * This is R3's probe-readable marker: an override that nothing could observe
   * would be indistinguishable from an intention.
   */
  override: {
    force_state: string | null
    override_id: string | null
    evicted: boolean
    evicted_at: string | null
    evicted_reason: string | null
  } | null
}

export async function runSessionProbe(code: string, deviceId: string): Promise<ApiResult<ProbeResponse>> {
  const controller = new AbortController()
  // Longer than the admin call's default: the probe may start the maintenance
  // engine on a network that blocks the hub by name (see probe.rs).
  const timer = setTimeout(() => controller.abort(), 45000)
  try {
    const resp = await fetch('/api/session-probe', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ probe_version: 1, code, device_id: deviceId }),
      signal: controller.signal,
    })
    const text = await resp.text()
    let parsed: Record<string, unknown> = {}
    try {
      parsed = text ? JSON.parse(text) : {}
    } catch {
      return { ok: false, transportError: `The probe returned a non-JSON response (HTTP ${resp.status})` }
    }
    if (!resp.ok) {
      return {
        ok: false,
        message: (parsed.message as string) || `The probe answered HTTP ${resp.status}`,
      }
    }
    return { ok: true, data: parsed as unknown as ProbeResponse }
  } catch (err) {
    const e = err as Error
    if (e.name === 'AbortError') return { ok: false, transportError: 'The probe took too long to answer.' }
    return { ok: false, transportError: e.message || 'Network error' }
  } finally {
    clearTimeout(timer)
  }
}
