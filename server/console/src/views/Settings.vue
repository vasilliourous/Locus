<script setup lang="ts">
// Settings — the surfaces changed rarely.
//
// One nav item, three sections, chosen by what they configure rather than by
// what they are:
//
//   Tiers       — the connection settings every client receives on activation.
//   Releases    — which client build the fleet is offered.
//   Maintenance — the operations that exist for a bad day: import/reconcile a
//                 code set, mint a one-shot fetch link, export.
//
// WHY THESE MOVED. Tiers and Releases were top-level tabs, competing for
// attention with Codes — the page opened dozens of times a week. Neither is
// visited weekly: a tier edit is a deliberate change to a live fleet, and a
// release is a monthly-ish event. Putting them behind one Settings item keeps the
// sidebar at four destinations without hiding anything.
//
// Maintenance is genuinely new *as a place*. Its two contents already existed as
// hook actions with no UI at all, which is worse than either a page or deletion:
// `codes.import` is the remedy for a student's app deleting its own entitlement
// (see the comment block above it in admin_console.pb.js) and was reachable only
// by curl.

import { ref } from 'vue'
import { nav } from '../nav'
import TiersPanel from './settings/TiersPanel.vue'
import ReleasesPanel from './settings/ReleasesPanel.vue'
import MaintenancePanel from './settings/MaintenancePanel.vue'

const sections = ref([
  { id: 'tiers' as const, label: 'Tiers', hint: 'connection settings clients receive' },
  { id: 'releases' as const, label: 'Releases', hint: 'which build the fleet is offered' },
  { id: 'maintenance' as const, label: 'Maintenance', hint: 'reconcile, export, trigger' },
])
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Settings</h1>
      <p class="page-sub">Configuration that changes rarely, and changes the whole fleet when it does.</p>
    </div>
  </div>

  <!-- Section switcher, mirrored in the sidebar. Both exist because the sidebar
       sub-menu only appears while Settings is open — so the first arrival needs a
       way in that does not require noticing the sidebar changed. -->
  <div class="filters" style="margin-bottom: 16px">
    <div class="segmented">
      <button
        v-for="s in sections"
        :key="s.id"
        :class="{ on: nav.section === s.id }"
        @click="nav.section = s.id"
      >
        {{ s.label }}
      </button>
    </div>
    <span class="muted" style="font-size: 12px">
      {{ sections.find((s) => s.id === nav.section)?.hint }}
    </span>
  </div>

  <TiersPanel v-if="nav.section === 'tiers'" />
  <ReleasesPanel v-else-if="nav.section === 'releases'" />
  <MaintenancePanel v-else-if="nav.section === 'maintenance'" />
</template>
