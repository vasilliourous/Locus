<script setup lang="ts">
// The command palette (⌘K / Ctrl-K / "/").
//
// WHY THIS EXISTS RATHER THAN A FIFTH TAB.
//
// The old console had five destinations and still could not answer the question
// that starts every support call: "a student called, their name is X, which
// code is theirs?" That required guessing the right page, then typing into one
// specific input on it. Adding a sixth page for search would have been the
// wrong shape — search is not a place you go, it is a thing you do from
// anywhere.
//
// `codes.list`'s query ALREADY matches label, notes and middleman as well as
// the code (the redesign's highest-leverage server change; see
// docs/business/redesign/06-console-and-operator.md §6.4). So this is entirely a
// front-end: one debounced `codes.list` call, rendered as results.
//
// Deliberately NOT offering every action. The palette finds things; acting on a
// found code opens its detail panel, where the actions live with their
// consequences shown. A palette that could also renew a code would be a
// one-keystroke way to extend the wrong student's term after a mis-typed name.

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { call } from '../api'
import { navigate } from '../router'
import { NAV_FLAT } from '../nav'

export interface PaletteCode {
  id: string
  code: string
  tier: string
  status: 'available' | 'redeemed' | 'suspended' | 'expired'
  label?: string
  middleman?: string
  expires_at?: string
  days_remaining?: number | null
}

const open = ref(false)
const q = ref('')
const busy = ref(false)
const results = ref<PaletteCode[]>([])
const active = ref(0)
const inputEl = ref<HTMLInputElement | null>(null)
let debounce: number | undefined
let seq = 0

// Static destinations, generated from the one nav model so a new page cannot be
// missing from the palette. Short and fixed on purpose: a palette is for the two
// kinds of navigation an operator needs mid-task — "take me to the screen I know
// exists" and "find this code".
const DESTINATIONS = NAV_FLAT.map((n) => ({
  kind: 'Page',
  what: n.label,
  hint: n.hint,
  run: () => navigate(n.name),
}))

const pages = computed(() => {
  const needle = q.value.trim().toLowerCase()
  if (!needle) return DESTINATIONS
  return DESTINATIONS.filter((d) => (d.what + ' ' + d.hint).toLowerCase().includes(needle))
})

/** Codes plus destinations, in one flat list so arrow keys do not care which kind. */
const rows = computed(() => {
  const out: { kind: string; what: string; hint: string; run: () => void }[] = []
  for (const p of pages.value) out.push({ kind: p.kind, what: p.what, hint: p.hint, run: p.run })
  for (const c of results.value) {
    out.push({
      kind: 'Code',
      what: c.code,
      hint: [c.label, c.middleman, c.tier, c.status].filter(Boolean).join(' · '),
      // Open the code's INSPECTOR, not a one-row list. This was the palette's
      // one unfinished step: it found the code, then put a list in front of it
      // that the operator had to click through.
      run: () => navigate('codes', { code: c.code }),
    })
  }
  return out
})

function openPalette() {
  open.value = true
  q.value = ''
  results.value = []
  active.value = 0
  nextTick(() => inputEl.value?.focus())
}

function close() {
  open.value = false
}

function move(delta: number) {
  if (!rows.value.length) return
  active.value = (active.value + delta + rows.value.length) % rows.value.length
}

function pick(i = active.value) {
  const row = rows.value[i]
  if (!row) return
  close()
  row.run()
}

// Debounced so typing a name does not fire a request per keystroke. 180ms is
// below the threshold where a result feels detached from the input.
watch(q, (value) => {
  window.clearTimeout(debounce)
  const term = value.trim()
  if (term.length < 2) {
    results.value = []
    return
  }
  // A code needs fewer characters to be unambiguous than a name does, but two
  // is the floor for a name search to return anything useful.
  busy.value = true
  debounce = window.setTimeout(async () => {
    const mine = ++seq
    const res = await call<{ codes: PaletteCode[] }>('codes.list', { query: term })
    // Drop a response that a later keystroke has already superseded, or the
    // list flickers back to stale results.
    if (mine !== seq) return
    busy.value = false
    results.value = res.ok && res.data ? (res.data.codes || []).slice(0, 12) : []
    active.value = 0
  }, 180)
})

function onKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement | null
  const typing = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT' || target.isContentEditable)

  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    if (open.value) close()
    else openPalette()
    return
  }
  // "/" as a shortcut only when not already typing — otherwise it is a
  // character the operator meant to type into a search box.
  if (e.key === '/' && !typing && !open.value) {
    e.preventDefault()
    openPalette()
    return
  }
  if (!open.value) return
  if (e.key === 'Escape') {
    e.preventDefault()
    close()
  } else if (e.key === 'ArrowDown') {
    e.preventDefault()
    move(1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    move(-1)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    pick()
  }
}

onMounted(() => document.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))

defineExpose({ openPalette })
</script>

<template>
  <div v-if="open" class="palette-wrap" @click.self="close">
    <div class="palette" role="dialog" aria-modal="true" aria-label="Search and jump">
      <input
        ref="inputEl"
        v-model="q"
        type="text"
        placeholder="Search a code, a student's name, or a middleman — or jump to a page"
        autocomplete="off"
        spellcheck="false"
        aria-controls="palette-results"
        :aria-activedescendant="rows.length ? `pal-${active}` : undefined"
      />

      <div id="palette-results" class="results" role="listbox">
        <template v-if="rows.length">
          <button
            v-for="(row, i) in rows"
            :id="`pal-${i}`"
            :key="`${row.kind}-${row.what}-${i}`"
            type="button"
            role="option"
            :aria-selected="i === active"
            class="item"
            :class="{ active: i === active }"
            @mouseenter="active = i"
            @click="pick(i)"
          >
            <span class="kind">{{ row.kind }}</span>
            <span class="what">
              <span :class="{ mono: row.kind === 'Code' }">{{ row.what }}</span>
              <small v-if="row.hint">{{ row.hint }}</small>
            </span>
          </button>
        </template>
        <div v-else-if="busy" class="state" style="padding: 20px">Searching…</div>
        <div v-else-if="q.trim().length >= 2" class="state" style="padding: 20px">
          No code or page matches “{{ q.trim() }}”.
          <p class="hint">
            The search covers the code, the student's name (label), notes and the
            middleman. A code that was never labelled cannot be found by name.
          </p>
        </div>
        <div v-else class="state" style="padding: 20px">
          Type at least two characters to search codes.
        </div>
      </div>

      <div class="foot">
        <span><span class="kbd">↑↓</span> move</span>
        <span><span class="kbd">↵</span> open</span>
        <span><span class="kbd">esc</span> close</span>
      </div>
    </div>
  </div>
</template>
