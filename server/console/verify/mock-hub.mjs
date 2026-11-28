// A local stand-in for /api/admin/console, shaped exactly like the hook.
//
// WHY THIS EXISTS. PocketBase could not be installed here (downloading a binary
// is blocked in this environment), so the console's real backend is unavailable.
// The alternative was "assert the UI is fine without ever running it", which is
// the thing this exercise exists to avoid.
//
// Written from `admin_console.pb.js`: same action names, same field names, same
// refusal cases. The SPA therefore exercises its REAL code paths — real parsing,
// real empty states, real error branches, real arithmetic in the renew drawer.
// What it does NOT prove is the hook itself; that is check-consistency.sh and the
// live hub's job.
//
// Deterministic: fixed fixture, relative clock offsets, so today's screenshot
// matches tomorrow's.

import http from 'node:http'

const DAY = 24 * 60 * 60 * 1000
const now = Date.now()
const iso = (ms) => new Date(ms).toISOString()

function mkCode({ code, tier, label = '', middleman = '', activated = null, expires = null, term = 70, suspended = false, released = null, lastSeen = null, notes = '', fingerprint = '' }) {
  const redeemed = !!activated
  const isExp = expires !== null && expires < now
  const status = suspended ? 'suspended' : isExp ? 'expired' : redeemed ? 'redeemed' : 'available'
  const releasedPending = status === 'redeemed' && released !== null && released > activated
  return {
    id: code,
    code,
    tier,
    status,
    redeemed,
    suspended,
    expires_at: expires === null ? '' : iso(expires),
    activated_at: activated === null ? '' : iso(activated),
    last_seen_at: lastSeen === null ? '' : iso(lastSeen),
    middleman,
    label,
    notes,
    term_days: term,
    term_kind: 'term',
    days_remaining: expires === null ? null : Math.max(0, Math.ceil((expires - now) / DAY)),
    first_seen_at: activated === null ? '' : iso(activated),
    first_seen_fingerprint: fingerprint,
    released_at: released === null ? '' : iso(released),
    release_count: released === null ? 0 : 1,
    released_pending: releasedPending,
  }
}

const codes = [
  mkCode({ code: 'RQ-ABCD-EFGH-JKMN-T', tier: 'strike', label: 'Macleans Y13 — Tom R', middleman: 'Sarah', activated: now - 40 * DAY, expires: now + 30 * DAY, lastSeen: now - 2 * 60 * 1000, fingerprint: 'a1b2c3d4e5f6a7b8' }),
  mkCode({ code: 'RQ-BCDE-FGHJ-KMNP-2', tier: 'strike', label: 'Macleans Y13 — Priya S', middleman: 'Sarah', activated: now - 60 * DAY, expires: now + 3 * DAY, lastSeen: now - 6 * 60 * 1000, fingerprint: 'b2c3d4e5f6a7b8c9' }),
  mkCode({ code: 'RQ-CDEF-GHJK-MNPQ-3', tier: 'budget', label: 'Auckland Grammar — Wiremu K', middleman: 'Dave', activated: now - 68 * DAY, expires: now + 2 * DAY, lastSeen: now - 3 * 60 * 60 * 1000 }),
  mkCode({ code: 'RQ-DEFG-HJKM-NPQR-4', tier: 'strike', label: 'Westlake Girls — Ana M', middleman: 'Sarah', activated: now - 100 * DAY, expires: now - 3 * DAY, lastSeen: now - 5 * DAY }),
  mkCode({ code: 'RQ-EFGH-JKMN-PQRS-5', tier: 'free', label: '', middleman: '', activated: null, expires: null, term: 0 }),
  mkCode({ code: 'RQ-FGHJ-KMNP-QRST-6', tier: 'strike', label: 'Rangitoto — Chris L', middleman: 'Dave', activated: now - 20 * DAY, expires: now + 50 * DAY, lastSeen: now - 9 * DAY, released: now - 4 * DAY }),
  mkCode({ code: 'RQ-GHJK-MNPQ-RSTV-7', tier: 'strike', label: 'St Kentigern — Sam T', middleman: 'Sarah', activated: now - 10 * DAY, expires: now + 60 * DAY, suspended: true, lastSeen: now - 30 * 60 * 1000 }),
  mkCode({ code: 'RQ-HJKM-NPQR-STVW-8', tier: 'free', label: 'Kings College — Jess W', middleman: '', activated: now - 15 * DAY, expires: null, term: 0, lastSeen: now - 20 * 60 * 1000 }),
  mkCode({ code: 'RQ-JKMN-PQRS-TVWX-9', tier: 'budget', label: 'Macleans Y12 — Liam O', middleman: 'Dave', activated: now - 55 * DAY, expires: now + 15 * DAY, lastSeen: now - 4 * 60 * 1000 }),
  mkCode({ code: 'RQ-KMNP-QRST-VWXY-2', tier: 'strike', label: 'Botany Downs — Nia H', middleman: 'Sarah', activated: now - 70 * DAY, expires: now + 1 * DAY, lastSeen: now - 90 * 60 * 1000 }),
  mkCode({ code: 'RQ-MNPQ-RSTV-WXYZ-3', tier: 'free', label: '', middleman: 'Dave', activated: null, expires: null, term: 0 }),
  mkCode({ code: 'RQ-NPQR-STVW-XYZ2-4', tier: 'free', label: '', middleman: 'Dave', activated: null, expires: null, term: 0 }),
  mkCode({ code: 'RQ-PQRS-TVWX-YZ23-5', tier: 'free', label: '', middleman: 'Sarah', activated: null, expires: null, term: 0 }),
  mkCode({ code: 'RQ-QRST-VWXY-Z234-6', tier: 'strike', label: 'Whangaparaoa — Hemi P', middleman: 'Sarah', activated: now - 5 * DAY, expires: now + 65 * DAY, lastSeen: now - 60 * 1000 }),
  mkCode({ code: 'RQ-RSTV-WXYZ-2345-7', tier: 'budget', label: 'Long Bay — Tessa N', middleman: 'Dave', activated: now - 30 * DAY, expires: now + 40 * DAY, lastSeen: now - 12 * 60 * 1000 }),
]

const tiers = [
  { id: 't1', tier: 'free', active: true, udp_relay: false, server: 'hub.example.org', server_port: 8443, method: '2022-blake3-aes-128-gcm', uot_port: 0, hub_alias: '' },
  { id: 't2', tier: 'budget', active: true, udp_relay: true, server: 'hub.example.org', server_port: 8444, method: '2022-blake3-aes-128-gcm', uot_port: 8444, hub_alias: '' },
  { id: 't3', tier: 'strike', active: true, udp_relay: true, server: 'hub.example.org', server_port: 8445, method: '2022-blake3-aes-256-gcm', uot_port: 8445, hub_alias: 'https://hub2.example.org' },
]

const middlemen = ['Dave', 'Sarah']

const events = [
  { code: 'RQ-ABCD-EFGH-JKMN-T', event: 'renewed', detail: 'expiry 2026-12-01 → 2027-02-01 (price 10.00, via Sarah)', created: iso(now - 2 * DAY) },
  { code: 'RQ-BCDE-FGHJ-KMNP-2', event: 'activated', detail: 'first activation', created: iso(now - 60 * DAY) },
  { code: 'RQ-CDEF-GHJK-MNPQ-3', event: 'renewed', detail: 'expiry 2026-11-20 → 2027-01-20', created: iso(now - 5 * DAY) },
  { code: '(tier strike)', event: 'tier.updated', detail: 'port 8445 → 8446', created: iso(now - 6 * DAY) },
  { code: 'RQ-EFGH-JKMN-PQRS-5', event: 'generated', detail: 'batch of 5 for Dave', created: iso(now - 8 * DAY) },
  { code: 'RQ-GHJK-MNPQ-RSTV-7', event: 'suspended', detail: 'reason: chargeback', created: iso(now - 9 * DAY) },
  { code: 'RQ-ABCD-EFGH-JKMN-T', event: 'activated', detail: 'first activation', created: iso(now - 40 * DAY) },
]

function dashboard() {
  const redeemed = codes.filter((c) => c.redeemed && !c.suspended && c.status !== 'expired')
  const expiringSoon = codes.filter((c) => c.expires_at && new Date(c.expires_at) > now && new Date(c.expires_at) <= now + 30 * DAY && !c.suspended && c.redeemed).length
  const byTier = {}
  for (const c of codes) byTier[c.tier] = (byTier[c.tier] || 0) + 1
  return {
    codes: {
      total: codes.length,
      available: codes.filter((c) => c.status === 'available').length,
      redeemed: redeemed.length,
      suspended: codes.filter((c) => c.suspended).length,
      expired: codes.filter((c) => c.status === 'expired').length,
      byTier,
      expiringSoon,
    },
    attempts: { total: 4, activate: 3, lookup: 1, window_minutes: 10 },
    recent: events.slice(0, 15),
  }
}

function attention() {
  const groups = { unservable_tier: [], lapsed: [], shared: [], no_label: [], gone_quiet: [] }
  const counts = { unservable_tier: 0, lapsed: 0, shared: 0, no_label: 0, gone_quiet: 0 }
  const served = new Set(tiers.filter((t) => t.active && t.server && t.server_port).map((t) => t.tier))
  for (const c of codes) {
    if (!served.has(c.tier)) {
      counts.unservable_tier++
      groups.unservable_tier.push({ code: c.code, tier: c.tier, middleman: c.middleman })
    }
    if (c.status === 'expired') {
      counts.lapsed++
      groups.lapsed.push({ code: c.code, label: c.label, days_past: 3, middleman: c.middleman })
    }
    if (c.released_pending && !c.suspended) {
      counts.shared++
      groups.shared.push({ code: c.code, label: c.label, released_at: c.released_at })
    }
    if (c.redeemed && !c.label && !c.suspended) {
      counts.no_label++
      groups.no_label.push({ code: c.code, middleman: c.middleman, activated_at: c.activated_at })
    }
    if (c.redeemed && !c.suspended && c.last_seen_at && now - new Date(c.last_seen_at).getTime() > (2 * 60 + 14) * 60 * 1000) {
      counts.gone_quiet++
      groups.gone_quiet.push({ code: c.code, label: c.label, hours_quiet: Math.round((now - new Date(c.last_seen_at).getTime()) / 3600000), middleman: c.middleman })
    }
  }
  return { total_codes: codes.length, tiers_served: [...served], counts, groups }
}

function codesList(body) {
  const q = String(body.query || '').trim().toUpperCase().replace(/-/g, '')
  const tier = String(body.tier || '').trim()
  const status = String(body.status || '').trim()
  const mm = String(body.middleman || '').trim().toLowerCase()
  const win = Number(body.expiring_within_days) || 0

  const facetStatus = { available: 0, redeemed: 0, expired: 0, suspended: 0 }
  const facetTier = {}
  for (const c of codes) {
    if (win > 0) {
      const e = c.expires_at ? new Date(c.expires_at).getTime() : 0
      if (c.suspended || !e || e <= now || e > now + win * DAY) continue
    }
    facetStatus[c.status] = (facetStatus[c.status] || 0) + 1
    facetTier[c.tier] = (facetTier[c.tier] || 0) + 1
  }

  let out = codes.filter((c) => {
    if (win > 0) {
      const e = c.expires_at ? new Date(c.expires_at).getTime() : 0
      if (c.suspended || !e || e <= now || e > now + win * DAY) return false
    }
    if (tier && c.tier !== tier) return false
    if (status && c.status !== status) return false
    if (mm && String(c.middleman).toLowerCase() !== mm) return false
    if (q) {
      const hay = [c.code.replace(/-/g, ''), c.label, c.notes, c.middleman].join(' ').toUpperCase()
      if (!hay.includes(q)) return false
    }
    return true
  })
  out = out.slice().sort((a, b) => {
    if (win > 0) {
      const da = a.days_remaining === null ? 1e9 : a.days_remaining
      const db = b.days_remaining === null ? 1e9 : b.days_remaining
      return da - db
    }
    return (b.activated_at || '').localeCompare(a.activated_at || '')
  })
  return { total: out.length, codes: out, expiring_within_days: win, facets: { status: facetStatus, tier: facetTier } }
}

function middlemanSummary(body) {
  const win = Number(body.window_days) || 30
  const since = now - win * DAY
  const out = middlemen.map((name) => {
    const mine = codes.filter((c) => c.middleman === name)
    const renewals = events.filter((e) => /renew/.test(e.event) && mine.some((c) => c.code === e.code))
    const priced = renewals.filter((e) => /price/.test(e.detail))
    const collected = priced.reduce((n, e) => {
      const m = e.detail.match(/price ([\d.]+)/)
      return n + (m ? Number(m[1]) : 0)
    }, 0)
    return {
      name,
      codes: mine.length,
      available: mine.filter((c) => c.status === 'available').length,
      in_term: mine.filter((c) => c.redeemed && !c.suspended && c.status !== 'expired').length,
      due_7d: mine.filter((c) => c.redeemed && !c.suspended && c.expires_at && new Date(c.expires_at) > now && new Date(c.expires_at) <= now + 7 * DAY).length,
      suspended: mine.filter((c) => c.suspended).length,
      expired: mine.filter((c) => c.status === 'expired').length,
      released_once: mine.filter((c) => c.released_at).length,
      free_codes: mine.filter((c) => c.tier === 'free').length,
      paid_codes: mine.filter((c) => c.tier !== 'free').length,
      renewals: renewals.length,
      collected,
      priced_renewals: priced.length,
      collected_complete: renewals.length > 0 && priced.length === renewals.length,
    }
  })
  return { middlemen: out, window_days: win, since: iso(since) }
}

const release = {
  version: '2.2.0',
  active: true,
  platforms: {
    linux: { url: 'chrysalis-linux-amd64', sha256: 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2', size: 48234112 },
    windows: { url: 'installer-chrysalis-windows-amd64.exe', sha256: 'b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3', size: 52000000 },
    macos_intel: { url: 'chrysalis-darwin-amd64', sha256: 'c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4', size: 61000000 },
    macos_arm: { url: 'chrysalis-darwin-arm64', sha256: 'd4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5', size: 59000000 },
  },
}

const server = http.createServer((req, res) => {
  res.setHeader('Access-Control-Allow-Origin', '*')
  res.setHeader('Access-Control-Allow-Headers', '*')
  if (req.method === 'OPTIONS') {
    res.writeHead(204)
    return res.end()
  }
  const route = String(req.url || '')
  let raw = ''
  req.on('data', (c) => (raw += c))
  req.on('end', () => {
    let body = {}
    try {
      body = raw ? JSON.parse(raw) : {}
    } catch {
      res.writeHead(400)
      return res.end('{}')
    }
    const send = (obj) => {
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ ok: true, ...obj }))
    }

    // ── /api/session-probe — the read-only diagnostic the console now runs ──
    //
    // Shaped like `session_probe.pb.js`: the machine check runs FIRST and answers
    // an opaque 404; the two axes are reported separately; the override block is
    // present only when force_state/evict_requested_at is set. Enough to drive
    // R2's visible result and R3's "the next probe alerts on the override".
    if (route.startsWith('/api/session-probe')) {
      if (Number(body.probe_version) !== 1) {
        res.writeHead(400, { 'Content-Type': 'application/json' })
        return res.end(JSON.stringify({ code: 400, message: 'Missing probe_version' }))
      }
      const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
      const presented = String(body.device_id || '').trim().toLowerCase()
      const stored = String((c && c.first_seen_fingerprint) || '').trim().toLowerCase()
      const matches = !!c && presented.length >= 12 && stored.length >= 12 && (presented === stored || stored.startsWith(presented) || presented.startsWith(stored))
      if (!matches) {
        res.writeHead(404, { 'Content-Type': 'application/json' })
        return res.end(JSON.stringify({ code: 404, message: 'Code not found' }))
      }
      const nowMs = Date.now()
      const exp = c.expires_at ? new Date(c.expires_at).getTime() : 0
      const releasedPending = !!c.redeemed && !!c.released_at && new Date(c.released_at).getTime() > new Date(c.activated_at || 0).getTime()
      const entitlement = c.suspended
        ? 'suspended'
        : exp && exp < nowMs
          ? 'expired'
          : !c.activated_at
            ? 'never-activated'
            : releasedPending
              ? 'released-unclaimed'
              : 'active'
      const seenMs = c.last_seen_at ? new Date(c.last_seen_at).getTime() : 0
      const ageS = seenMs ? Math.floor((nowMs - seenMs) / 1000) : null
      const liveness = ageS === null ? 'unknown' : ageS <= 8040 ? 'live' : ageS <= 7 * 86400 ? 'stale' : 'absent'
      const hasOverride = !!c.force_state || !!c.evict_requested_at
      const divergence = []
      if (hasOverride) divergence.push('operator_override')
      res.writeHead(200, { 'Content-Type': 'application/json' })
      return res.end(
        JSON.stringify({
          probe_version: 1,
          server_time: iso(nowMs),
          entitlement,
          liveness,
          hub_view: {
            activated_at: c.activated_at || null,
            released_at: c.released_at || null,
            released_pending: releasedPending,
            suspended: !!c.suspended,
            suspended_reason: c.suspended ? 'verify mock' : null,
            expires_at: c.expires_at || null,
            tier: c.tier,
            last_seen_at: c.last_seen_at || null,
            last_seen_age_s: ageS,
            first_seen_fingerprint: c.first_seen_fingerprint || '',
          },
          client_view: null,
          lease_expires_at: seenMs ? iso(seenMs + 8040 * 1000) : null,
          reason_code: liveness === 'unknown' ? 'unknown' : 'client_beating_hub_quiet',
          divergence,
          override: hasOverride
            ? {
                force_state: c.force_state || null,
                override_id: c.override_id || null,
                evicted: !!c.evict_requested_at,
                evicted_at: c.evict_requested_at || null,
                evicted_reason: c.evict_reason || null,
              }
            : null,
        }),
      )
    }

    switch (body.action) {
      case 'dashboard':
        return send(dashboard())
      case 'attention.list':
        return send(attention())
      case 'codes.list':
        return send(codesList(body))
      case 'codes.history': {
        const code = String(body.code || '').toUpperCase()
        return send({ code, events: events.filter((e) => e.code === code) })
      }
      case 'middlemen.list':
        return send({ middlemen: middlemen.map((name) => ({ name, codes: codes.filter((c) => c.middleman === name).length })) })
      case 'middlemen.summary':
        return send(middlemanSummary(body))
      case 'tiers.list':
        return send({ tiers })
      case 'releases.get':
        return send({ release })
      case 'codes.renew': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (!c) {
          res.writeHead(200, { 'Content-Type': 'application/json' })
          return res.end(JSON.stringify({ ok: false, message: 'Code not found' }))
        }
        const days = Number(body.days) || c.term_days || 70
        const base = Math.max(now, c.expires_at ? new Date(c.expires_at).getTime() : now)
        c.expires_at = iso(base + days * DAY)
        c.days_remaining = Math.ceil((new Date(c.expires_at).getTime() - now) / DAY)
        if (c.status === 'expired') c.status = c.redeemed ? 'redeemed' : 'available'
        return send({ code: c.code, expires_at: c.expires_at })
      }
      case 'codes.unsuspend':
      case 'codes.suspend': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        const want = body.action === 'codes.suspend'
        if (c) c.suspended = want
        return send({ code: body.code, suspended: want })
      }
      case 'codes.update': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (c) {
          if ('label' in body) c.label = body.label
          if ('middleman' in body) c.middleman = body.middleman
          if ('notes' in body) c.notes = body.notes
          if ('tier' in body) c.tier = body.tier
        }
        return send({ code: body.code })
      }
      case 'codes.set-term': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (c) c.term_days = Number(body.term_days) || 0
        return send({ code: body.code })
      }
      case 'codes.expire': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (c) {
          c.expires_at = body.expires_at || ''
          c.days_remaining = c.expires_at ? Math.max(0, Math.ceil((new Date(c.expires_at).getTime() - now) / DAY)) : null
        }
        return send({ code: body.code, expires_at: c ? c.expires_at : '' })
      }
      case 'codes.release':
      case 'codes.unbind': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (!c) return send({ ok: false, message: 'Code not found' }, 404)
        // Mirror the real handler's branch so the drawer's preview and its commit
        // take the same path. `live` exercises the typed-code confirmation.
        const observed = c.last_seen_at ? 'live' : 'unknown'
        const willEvict = observed === 'live'
        if (body.dry_run === true) {
          return send({
            code: c.code,
            dry_run: true,
            observed_state: observed,
            will_evict: willEvict,
            severity: willEvict ? 'evicts-live-client' : 'release-only',
            consequences: [
              'The single-use claim is cleared: this code becomes available to a new holder.',
              'Its tier and expiry are NOT changed.',
              willEvict
                ? 'That app will be cut off at its next check-in — within about five minutes while connected.'
                : 'No eviction: if the device reappears later it will NOT be refused by this release.',
            ],
          })
        }
        // Replay safety: the same override_id must not do the work twice.
        if (c.override_id && c.override_id === body.override_id) {
          return send({ code: c.code, replay: true, observed_state: observed })
        }
        c.override_id = String(body.override_id || '')
        c.redeemed = false
        c.activated_at = ''
        c.status = 'released'
        c.force_state = 'released'
        if (willEvict) {
          c.evict_requested_at = iso(now)
          c.evict_reason = String(body.reason || '')
        }
        return send({ code: c.code, evicted: willEvict, outcome: willEvict ? 'applied_with_eviction' : 'applied' })
      }
      case 'codes.override': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (!c) return send({ ok: false, message: 'Code not found' }, 404)
        if (String(body.to_state || '') !== 'in_term') return send({ ok: false, message: 'to_state must be in_term' }, 400)
        const pending = !!c.released_at && new Date(c.released_at).getTime() > new Date(c.activated_at || 0).getTime()
        if (!pending) return send({ code: c.code, noop: true, state: 'in_term' })
        c.released_at = ''
        c.released_pending = false
        c.override_id = String(body.override_id || '')
        return send({ code: c.code, state: 'in_term', cleared_sign_out: true })
      }
      case 'codes.reactivate': {
        const c = codes.find((x) => x.code === String(body.code || '').toUpperCase())
        if (!c) return send({ ok: false, message: 'Code not found' }, 404)
        if (!c.force_state && !c.evict_requested_at) return send({ ok: false, message: 'no override' }, 400)
        c.force_state = ''
        c.evict_requested_at = ''
        c.evict_reason = ''
        c.override_id = ''
        c.status = 'available'
        return send({ code: c.code })
      }
      case 'codes.delete': {
        const i = codes.findIndex((x) => x.code === String(body.code || '').toUpperCase())
        if (i >= 0) codes.splice(i, 1)
        return send({ code: body.code, deleted: true })
      }
      case 'codes.generate': {
        const n = Math.min(Number(body.count) || 1, 5)
        const made = []
        for (let i = 0; i < n; i++) {
          const code = `RQ-TEST-100${i}-ABCD-${i}`
          codes.unshift(mkCode({ code, tier: body.tier || 'free', label: body.label || '', middleman: body.middleman || '', term: Number(body.term_days) || 0 }))
          made.push(code)
        }
        return send({ created: made, skipped: [], tier: body.tier, term_days: body.term_days })
      }
      case 'releases.fetchLink':
        return send({ link: 'https://hub.example.org/api/admin/fetch-release?t=onetime-abc123', expires_in: 600 })
      default:
        return send({})
    }
  })
})

server.listen(8777, () => console.log('mock hub on http://127.0.0.1:8777'))
