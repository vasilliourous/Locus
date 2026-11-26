<script setup lang="ts">
// The shell: four destinations, one search, one sign-out.
//
// WHAT CHANGED AND WHY. There were five nav items, and two of them ("Codes &
// Clients" and "Used codes") listed the same rows from the same collection — one
// read-only, both re-fetching `codes.list`. The tabs were organised by how the
// data happened to be shaped rather than by what an operator does.
//
// Now the four are jobs:
//   Overview      — is anything wrong, and where do I go to fix it?
//   Codes         — find one, act on it. The 95% case.
//   Distribution  — who sold what, and what do they owe. The only surface with a
//                   money column, and the reason `middlemen.list` finally has a
//                   view (it was fetched and thrown away before).
//   Settings      — things changed rarely: tiers, releases, maintenance.
//
// Settings holds three surfaces behind one nav item, which is what keeps the
// top-level count at four: a settings page that lists its sections is cheaper to
// scan than four more tabs nobody visits weekly.

import { computed, onMounted, ref, watch } from 'vue'
import { clearToken, hasToken, call } from './api'
import { nav, go, openCodes, openSettings } from './nav'
import Login from './views/Login.vue'
import Overview from './views/Overview.vue'
import Codes from './views/Codes.vue'
import Distribution from './views/Distribution.vue'
import Settings from './views/Settings.vue'
import Toast from './components/ToastView.vue'
import CommandPalette from './components/CommandPalette.vue'

const signedIn = ref(hasToken())
const palette = ref<InstanceType<typeof CommandPalette> | null>(null)

const pages = computed(() => [
  { id: 'overview' as const, label: 'Overview' },
  { id: 'codes' as const, label: 'Codes' },
  { id: 'distribution' as const, label: 'Distribution' },
  { id: 'settings' as const, label: 'Settings' },
])

// The renewal queue count rides the Codes nav item. It is the one number the
// console owes the operator unprompted: nothing else in the system tells them a
// student's term is about to lapse (docs/business/09-console.md §9.3), so a
// visible count IS the prompt. Zero hides it — a permanent "0" trains the eye to
// ignore that spot.
const dueSoon = ref<number | null>(null)

async function loadDue() {
  const res = await call<{ codes: unknown[] }>('codes.list', { expiring_within_days: 7 })
  dueSoon.value = res.ok && res.data ? (res.data.codes || []).length : null
}

function onSignedIn() {
  signedIn.value = true
  go('overview')
  void loadDue()
}

function signOut() {
  clearToken()
  signedIn.value = false
  dueSoon.value = null
}

// Re-read the queue count whenever the destination changes: a renewal performed
// on the Codes page drops the badge on the way back without a full reload.
watch(
  () => nav.page,
  (p) => {
    if (signedIn.value && (p === 'overview' || p === 'codes')) void loadDue()
  },
)

onMounted(() => {
  if (signedIn.value) void loadDue()
})
</script>

<template>
  <Login v-if="!signedIn" @signed-in="onSignedIn" />

  <div v-else class="shell">
    <nav class="sidebar" aria-label="Main">
      <div class="brand">
        Chrysalis
        <small>operator console</small>
      </div>

      <button class="search-trigger" type="button" @click="palette?.openPalette()">
        <span aria-hidden="true">⌕</span>
        <span>Find a code or name…</span>
        <span class="kbd">⌘K</span>
      </button>

      <button
        v-for="p in pages"
        :key="p.id"
        class="nav-item"
        :class="{ active: nav.page === p.id }"
        @click="go(p.id)"
      >
        {{ p.label }}
        <span
          v-if="p.id === 'codes' && dueSoon"
          class="nav-count"
          :title="`${dueSoon} code(s) end within 7 days`"
        >
          {{ dueSoon }}
        </span>
      </button>

      <!-- Settings' sections are only shown while Settings is open: a permanent
           sub-menu for three pages nobody visits weekly is noise in the 95% of
           sessions that are about codes. -->
      <template v-if="nav.page === 'settings'">
        <div class="nav-group">Settings</div>
        <button
          class="nav-item"
          :class="{ active: nav.section === 'tiers' }"
          @click="openSettings('tiers')"
        >
          Tiers
        </button>
        <button
          class="nav-item"
          :class="{ active: nav.section === 'releases' }"
          @click="openSettings('releases')"
        >
          Releases
        </button>
        <button
          class="nav-item"
          :class="{ active: nav.section === 'maintenance' }"
          @click="openSettings('maintenance')"
        >
          Maintenance
        </button>
      </template>

      <div class="spacer" />

      <button
        class="nav-item"
        title="Codes ending within 7 days"
        @click="openCodes({ view: 'worklist', expiring_within_days: 7 })"
      >
        Renewal queue
        <span v-if="dueSoon" class="nav-count">{{ dueSoon }}</span>
      </button>
      <button class="nav-item" @click="signOut">Sign out</button>
    </nav>

    <main class="main">
      <Overview v-if="nav.page === 'overview'" />
      <Codes v-else-if="nav.page === 'codes'" />
      <Distribution v-else-if="nav.page === 'distribution'" />
      <Settings v-else-if="nav.page === 'settings'" />
    </main>
  </div>

  <CommandPalette ref="palette" />
  <Toast />
</template>
