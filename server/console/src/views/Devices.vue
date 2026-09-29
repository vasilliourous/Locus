<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { call } from '../api'
import { toast } from '../toast'

/**
 * Devices — every code/device binding on the hub.
 *
 * # Why this page exists
 *
 * "One code per device" is enforced by a UNIQUE index on the binding table, so
 * the rule cannot be violated by an ordinary write path. This page is the
 * EVIDENCE that it holds, and the place to see the states the index cannot
 * express:
 *
 *   * a binding whose code no longer exists (`code_missing`) — the uniqueness
 *     check would still refuse a new activation on that device, so it is a
 *     state an operator must see and clear rather than a silent oddity;
 *   * a code that holds a fingerprint the index does not know about — reported
 *     on the Codes page as `unindexed`.
 *
 * It is deliberately read-only. Every mutation lives on the Codes page, where
 * it already has an audit trail; adding write actions here would give an
 * operator two routes to the same change and only one of them familiar.
 */

interface Binding {
  fingerprint: string
  code: string
  tier: string
  bound_at: string
  released: boolean
  released_at: string
  release_reason: string
  /** A live binding whose code row is gone. Needs clearing. */
  code_missing: boolean
  label: string
  expires_at: string
}

interface DevicesResponse {
  total: number
  live_count: number
  /** Should always be empty — the unique index is what prevents it. */
  duplicate_live_fingerprints: string[]
  bindings: Binding[]
}

const data = ref<DevicesResponse | null>(null)
const loading = ref(true)
/** Hide history by default: released rows accumulate and are rarely the point. */
const showReleased = ref(false)

async function load() {
  loading.value = true
  const res = await call<DevicesResponse>('devices.list')
  if (res.ok && res.data) {
    data.value = res.data
  } else {
    toast.err(res.message || res.transportError || 'Could not load device bindings')
  }
  loading.value = false
}

function when(iso: string): string {
  if (!iso) return '—'
  const d = new Date(iso)
  return isNaN(d.getTime()) ? iso : d.toLocaleString()
}

onMounted(load)
</script>

<template>
  <h1 class="page-title">Devices</h1>
  <p class="page-sub">
    Every device binding on the hub. One device holds one code — this is the
    evidence, not the control.
  </p>

  <div v-if="loading" class="muted">Loading…</div>

  <template v-else-if="data">
    <!-- An integrity failure: the unique index should make this impossible.
         Reported loudly rather than assumed away. -->
    <div v-if="data.duplicate_live_fingerprints.length" class="msg err">
      <strong>Binding index inconsistency:</strong>
      {{ data.duplicate_live_fingerprints.length }} fingerprint(s) appear more than once as live
      ({{ data.duplicate_live_fingerprints.join(', ') }}). The unique index should prevent this —
      investigate before trusting the one-code-per-device rule.
    </div>

    <div class="cards">
      <div class="card">
        <div class="n">{{ data.live_count }}</div>
        <div class="l">Live bindings</div>
      </div>
      <div class="card">
        <div class="n">{{ data.total }}</div>
        <div class="l">Including released history</div>
      </div>
      <div class="card" :class="data.bindings.some((b) => b.code_missing) ? 'bad' : 'good'">
        <div class="n">{{ data.bindings.filter((b) => b.code_missing).length }}</div>
        <div class="l">Bindings whose code is missing</div>
      </div>
    </div>

    <div class="panel">
      <div class="actions" style="margin-bottom: 10px">
        <button class="tiny" :class="{ primary: !showReleased }" @click="showReleased = false">
          Live only
        </button>
        <button class="tiny" :class="{ primary: showReleased }" @click="showReleased = true">
          Include released
        </button>
        <button class="tiny" @click="load">Refresh</button>
      </div>

      <div v-if="data.bindings.length === 0" class="muted">
        No device bindings yet. A binding is created the first time a code is activated.
      </div>

      <table v-else>
        <thead>
          <tr>
            <th>Device</th>
            <th>Code</th>
            <th>Name</th>
            <th>Tier</th>
            <th>Bound</th>
            <th>Expires</th>
            <th>State</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(b, i) in data.bindings.filter((x) => showReleased || !x.released)"
            :key="b.fingerprint + i"
            :class="{ dim: b.released }"
          >
            <td class="mono muted">{{ b.fingerprint || '—' }}</td>
            <td class="mono">{{ b.code }}</td>
            <td class="muted">{{ b.label || '—' }}</td>
            <td>{{ b.tier || '—' }}</td>
            <td class="muted">{{ when(b.bound_at) }}</td>
            <td class="muted">{{ b.expires_at ? b.expires_at.slice(0, 10) : '—' }}</td>
            <td>
              <span v-if="b.code_missing" class="badge suspended" title="The code this device is bound through no longer exists. The uniqueness check will still refuse a new activation, so unbind and re-activate the device to clear it.">
                code missing
              </span>
              <span v-else-if="b.released" class="badge" title="Released — the device is free to activate another code.">
                released
              </span>
              <span v-else class="badge bound">live</span>
            </td>
          </tr>
        </tbody>
      </table>

      <p v-if="showReleased" class="muted" style="font-size: 12px; margin-top: 10px">
        Released rows are kept rather than deleted, so "has this device ever been
        bound, and to what" stays answerable. A release reason is recorded when an
        operator unbinds or moves a code.
      </p>
    </div>
  </template>
</template>
