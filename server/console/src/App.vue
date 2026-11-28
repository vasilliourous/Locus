<script setup lang="ts">
// The shell: navigation, the palette, and the route switch.
//
// This file used to hold four flags and a conditional sub-menu for Settings. It
// now holds nothing but the chrome, because the destination lives in the URL
// (`router.ts`) and the pages read it themselves. The one piece of state left is
// whether the user is signed in, which is a fact about the browser session and
// not a route — hence it is still a ref here.
//
// THE RENEWAL BADGE. It used to ride the Codes nav item and reload on every
// destination change. Nothing else in the system tells the operator a student's
// term is about to lapse (09-console.md §9.3), so the count IS the prompt — which
// is why it stays on the sidebar, next to "Renewals due", and why zero hides it
// (a permanent "0" trains the eye to ignore that spot).

import { onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { clearToken, hasToken, call } from './api'
import { route, navigate, startRouter } from './router'
import { NAV } from './nav'
import Login from './views/Login.vue'
import Today from './views/Today.vue'
import Codes from './views/Codes.vue'
import Renewals from './views/Renewals.vue'
import Distribution from './views/Distribution.vue'
import TiersPanel from './views/settings/TiersPanel.vue'
import ReleasesPanel from './views/settings/ReleasesPanel.vue'
import MaintenancePanel from './views/settings/MaintenancePanel.vue'
import Toast from './components/ToastView.vue'
import CommandPalette from './components/CommandPalette.vue'
import HelpRail from './components/HelpRail.vue'

const signedIn = ref(hasToken())
const palette = ref<InstanceType<typeof CommandPalette> | null>(null)
const navOpen = ref(false)

const dueSoon = ref<number | null>(null)

async function loadDue() {
  const res = await call<{ codes: unknown[] }>('codes.list', { expiring_within_days: 7 })
  dueSoon.value = res.ok && res.data ? (res.data.codes || []).length : null
}

function onSignedIn() {
  signedIn.value = true
  void loadDue()
  // DO NOT navigate away from the URL that was requested. Login strips only the
  // `?token=` parameter and leaves the path, so a pasted `/admin/codes/RQ-…`
  // survives sign-in and opens that code — which is the whole point of the code
  // having an address. Only a bare `/admin/` falls through to Today, and that is
  // already what `route.name === 'today'` means.
}

function signOut() {
  clearToken()
  signedIn.value = false
  dueSoon.value = null
  navigate('today', {}, 'replace')
}

// Refresh the queue count when the operator leaves a screen that could have
// changed it. Cheap enough to keep current; too frequent to poll.
watch(
  () => route.name,
  (p) => {
    if (signedIn.value && dueSoon.value !== null) void loadDue()
    void p
  },
)

onMounted(() => {
  startRouter()
  if (signedIn.value) void loadDue()
  document.addEventListener('keydown', onHelpKey)
})
onBeforeUnmount(() => document.removeEventListener('keydown', onHelpKey))

// `?` toggles the help rail. Suppressed while typing, and while a drawer or the
// palette owns the keyboard, so a question mark in a search box stays a question
// mark.
const helpOpen = ref(false)
function onHelpKey(e: KeyboardEvent) {
  if (e.key !== '?' || e.metaKey || e.ctrlKey || e.altKey) return
  const t = e.target as HTMLElement | null
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)) return
  e.preventDefault()
  helpOpen.value = !helpOpen.value
}

// Help that describes another screen is worse than no help: close it on a move.
watch(
  () => route.name,
  () => (helpOpen.value = false),
)

// A route to a code with no session is the one case where the URL has to be
// preserved across sign-in: someone pasted a code while signed out.
const pendingPath = ref('')
watch(
  signedIn,
  (v) => {
    if (!v) pendingPath.value = window.location.pathname + window.location.search
  },
  { immediate: true },
)
</script>

<template>
  <Login v-if="!signedIn" @signed-in="onSignedIn" />

  <div v-else class="shell">
    <!-- The mobile toggle. Below 800px the sidebar becomes a drawer; without
         this there is no way to change destination on a phone at all. -->
    <button class="nav-toggle" type="button" :aria-expanded="navOpen" aria-label="Menu" @click="navOpen = !navOpen">
      ☰
    </button>

    <nav class="sidebar" :class="{ open: navOpen }" aria-label="Main">
      <div class="brand">
        Chrysalis
        <small>operator console</small>
      </div>

      <button class="search-trigger" type="button" @click="palette?.openPalette(); navOpen = false">
        <span aria-hidden="true">⌕</span>
        <span>Find a code or name…</span>
        <span class="kbd">/</span>
      </button>

      <template v-for="g in NAV" :key="g.label">
        <div class="nav-group">{{ g.label }}</div>
        <button
          v-for="item in g.items"
          :key="item.name"
          class="nav-item"
          :class="{ active: route.name === item.name }"
          @click="navigate(item.name); navOpen = false"
        >
          {{ item.label }}
          <!-- The renewal prompt rides the destination it belongs to. -->
          <span
            v-if="(item.name === 'renewals' || item.name === 'today') && dueSoon"
            class="nav-count"
            :title="`${dueSoon} code(s) end within 7 days`"
          >
            {{ dueSoon }}
          </span>
        </button>
      </template>

      <div class="spacer" />

      <button class="nav-item" :class="{ active: helpOpen }" @click="helpOpen = !helpOpen">
        Help on this screen
        <span class="kbd">?</span>
      </button>
      <button class="nav-item" @click="signOut">Sign out</button>
    </nav>

    <!-- Clicking away closes the mobile drawer. -->
    <div v-if="navOpen" class="nav-scrim" @click="navOpen = false" />

    <main class="main" :class="{ 'with-help': helpOpen }">
      <Today v-if="route.name === 'today'" />
      <Codes v-else-if="route.name === 'codes'" />
      <Renewals v-else-if="route.name === 'renewals'" />
      <Distribution v-else-if="route.name === 'distribution'" />
      <TiersPanel v-else-if="route.name === 'fleet/tiers'" />
      <ReleasesPanel v-else-if="route.name === 'fleet/releases'" />
      <MaintenancePanel v-else-if="route.name === 'fleet/maintenance'" />
    </main>

    <HelpRail :open="helpOpen" :route="route.name" @close="helpOpen = false" />
  </div>

  <CommandPalette ref="palette" />
  <Toast />
</template>
