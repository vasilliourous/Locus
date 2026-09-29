<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { hasToken, clearToken } from './api'
import Login from './views/Login.vue'
import Dashboard from './views/Dashboard.vue'
import Codes from './views/Codes.vue'
import Devices from './views/Devices.vue'
import Releases from './views/Releases.vue'
import Tiers from './views/Tiers.vue'
import Toast from './components/ToastView.vue'

type Page = 'dashboard' | 'codes' | 'devices' | 'releases' | 'tiers'

const signedIn = ref(hasToken())
// Simple hash-less view switching — this is a four-page internal tool, a router
// would be more moving parts than value. Refresh keeps you signed in (token is
// in sessionStorage) and returns to the dashboard.
const page = ref<Page>('dashboard')

// A page can ask to open another page with a mode attached — currently only the
// Dashboard's "expiring" card, which opens the Codes view already filtered to
// the renewal worklist.
//
// A tiny event rather than a prop or a router: the Dashboard is not the parent
// of Codes, and threading a callback through would couple two pages that should
// not know about each other. The window event is the least machinery that
// survives without a router.
const codesWorklistDays = ref(0)
function openWorklist(days: number) {
  codesWorklistDays.value = days
  page.value = 'codes'
}
onMounted(() => {
  const handler = (e: Event) => {
    const detail = (e as CustomEvent).detail
    if (detail && typeof detail.days === 'number') openWorklist(detail.days)
  }
  window.addEventListener('locus:open-worklist', handler)
  onUnmounted(() => window.removeEventListener('locus:open-worklist', handler))
})

const pages: { id: Page; label: string }[] = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'codes', label: 'Codes & Clients' },
  { id: 'devices', label: 'Devices' },
  { id: 'releases', label: 'Releases' },
  { id: 'tiers', label: 'Tiers' },
]

const currentLabel = computed(() => pages.find((p) => p.id === page.value)?.label || '')

function onSignedIn() {
  signedIn.value = true
  page.value = 'dashboard'
}

function signOut() {
  clearToken()
  signedIn.value = false
}
</script>

<template>
  <Login v-if="!signedIn" @signed-in="onSignedIn" />

  <div v-else class="shell">
    <nav class="sidebar">
      <div class="brand">
        Locus Console
        <small>networkingguides.duckdns.org</small>
      </div>
      <button
        v-for="p in pages"
        :key="p.id"
        class="nav-item"
        :class="{ active: page === p.id }"
        @click="page = p.id"
      >
        {{ p.label }}
      </button>
      <div class="spacer" />
      <button class="nav-item" @click="signOut">Sign out</button>
    </nav>

    <main class="main">
      <Dashboard v-if="page === 'dashboard'" />
      <Codes v-else-if="page === 'codes'" :worklist-days="codesWorklistDays" />
      <Devices v-else-if="page === 'devices'" />
      <Releases v-else-if="page === 'releases'" />
      <Tiers v-else-if="page === 'tiers'" :key="currentLabel" />
    </main>
  </div>

  <Toast />
</template>
