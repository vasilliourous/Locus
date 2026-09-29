<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { call } from '../api'
import { toast } from '../toast'

interface Dash {
  codes: {
    total: number
    available: number
    bound: number
    suspended: number
    expired: number
    byTier: Record<string, number>
    expiringSoon: number
  }
  attempts: number
  recent: { code: string; event: string; detail: string; created: string }[]
}

const data = ref<Dash | null>(null)
const loading = ref(true)

async function load() {
  loading.value = true
  const res = await call<Dash>('dashboard')
  if (res.ok && res.data) {
    data.value = res.data
  } else {
    toast.err(res.message || res.transportError || 'Could not load the dashboard')
  }
  loading.value = false
}

function when(iso: string): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso
  return d.toLocaleString()
}

onMounted(load)

/**
 * Open the Codes view already filtered to the renewal worklist.
 *
 * A window event rather than a router push, because this console deliberately
 * has no router (four pages, a `ref` each) and the Dashboard is not the parent
 * of Codes. Numbers match the console's own worklist buttons and the client's
 * 7-day warning threshold.
 */
function openWorklist() {
  window.dispatchEvent(new CustomEvent('locus:open-worklist', { detail: { days: 30 } }))
}
</script>

<template>
  <h1 class="page-title">Dashboard</h1>
  <p class="page-sub">Overview of issued codes and recent activity.</p>

  <div v-if="loading" class="muted">Loading…</div>

  <template v-else-if="data">
    <div class="cards">
      <div class="card">
        <div class="n">{{ data.codes.total }}</div>
        <div class="l">Total codes</div>
      </div>
      <div class="card good">
        <div class="n">{{ data.codes.available }}</div>
        <div class="l">Available (unused)</div>
      </div>
      <div class="card">
        <div class="n">{{ data.codes.bound }}</div>
        <div class="l">Activated on a device</div>
      </div>
      <div class="card bad">
        <div class="n">{{ data.codes.suspended }}</div>
        <div class="l">Suspended</div>
      </div>
      <!-- Clickable: this count is the renewal worklist's entry point.
           Renewal is an operator action and NOTHING in the system prompts one
           (the hub stores no identity, so it cannot know a payment is due).
           This card is therefore the prompt, or there is none — a renew button
           nobody is reminded to press does not collect money. -->
      <div
        class="card warn"
        role="button"
        tabindex="0"
        style="cursor: pointer"
        title="Open the renewal worklist"
        @click="openWorklist"
        @keyup.enter="openWorklist"
      >
        <div class="n">{{ data.codes.expiringSoon }}</div>
        <div class="l">Expiring in 30 days — renew →</div>
      </div>
    </div>

    <div class="panel">
      <h2>By tier</h2>
      <div v-if="Object.keys(data.codes.byTier).length === 0" class="muted">
        No codes issued yet.
      </div>
      <table v-else>
        <thead>
          <tr><th>Tier</th><th>Codes</th></tr>
        </thead>
        <tbody>
          <tr v-for="(count, tier) in data.codes.byTier" :key="tier">
            <td><strong>{{ tier }}</strong></td>
            <td>{{ count }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="panel">
      <h2>Recent activity</h2>
      <div v-if="data.recent.length === 0" class="muted">
        Nothing yet. Activity appears here as codes are generated, activated and unbound.
      </div>
      <table v-else>
        <thead>
          <tr><th>When</th><th>Code</th><th>Event</th><th>Detail</th></tr>
        </thead>
        <tbody>
          <tr v-for="(ev, i) in data.recent" :key="i">
            <td class="muted">{{ when(ev.created) }}</td>
            <td><code>{{ ev.code }}</code></td>
            <td>{{ ev.event }}</td>
            <td class="muted">{{ ev.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="panel">
      <h2>Activation attempts</h2>
      <p class="muted" style="margin: 0">
        {{ data.attempts }} recorded in the last 10 minutes window.
        A spike here usually means a mistyped code or someone guessing.
      </p>
    </div>

    <div class="actions">
      <button @click="load">Refresh</button>
    </div>
  </template>
</template>
