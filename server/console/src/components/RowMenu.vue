<script setup lang="ts">
// The row overflow menu.
//
// WHY. The Codes table gave every row up to SEVEN buttons of equal weight —
// Details, Renew, Release, Expiry, Kick out, Suspend, Delete — laid out
// left-to-right in the last cell. Two problems, both visible in the old page:
//
//   1. "Renew" (performed dozens of times a week) and "Delete" (performed
//      almost never, and destructive) looked identical, sat side by side, and
//      on a narrow window wrapped into each other.
//   2. A column of buttons is noise the eye cannot skip past while scanning a
//      code and a date. It is the reason the table needed eleven columns and
//      still felt cramped.
//
// So: one primary action stays in the row, everything else goes here. The menu
// is positioned in a fixed layer because the table is inside an `overflow:auto`
// scroller (a sticky header needs it), and an absolutely-positioned menu would
// be clipped by it at the last row.

import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { MenuItem } from './menu'

const props = defineProps<{ items: MenuItem[]; label?: string }>()

const open = ref(false)
const btn = ref<HTMLButtonElement | null>(null)
const style = ref<Record<string, string>>({})

const MENU_W = 200

function position() {
  const el = btn.value
  if (!el) return
  const r = el.getBoundingClientRect()
  // Open to the left of the trigger and keep it on-screen: the trigger is in
  // the last column, so a menu opening rightwards would run off the edge.
  let left = r.right - MENU_W
  if (left < 8) left = 8
  let top = r.bottom + 4
  // If there is no room below, flip above rather than letting it be clipped.
  const estH = props.items.length * 32 + 12
  if (top + estH > window.innerHeight - 8) top = Math.max(8, r.top - estH - 4)
  style.value = { left: `${left}px`, top: `${top}px` }
}

function toggle() {
  if (open.value) {
    open.value = false
    return
  }
  position()
  open.value = true
}

function pick(item: MenuItem) {
  if (item.disabled) return
  open.value = false
  item.run()
}

function onDocClick(e: MouseEvent) {
  if (!open.value) return
  const t = e.target as Node
  if (btn.value?.contains(t)) return
  const menu = document.getElementById('row-menu-layer')
  if (menu?.contains(t)) return
  open.value = false
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') open.value = false
}

onMounted(() => {
  document.addEventListener('click', onDocClick, true)
  document.addEventListener('keydown', onKey)
  window.addEventListener('resize', () => (open.value = false))
  window.addEventListener('scroll', () => (open.value = false), true)
})
onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick, true)
  document.removeEventListener('keydown', onKey)
})
</script>

<template>
  <button
    ref="btn"
    class="row-menu"
    type="button"
    :aria-label="label || 'More actions'"
    aria-haspopup="menu"
    :aria-expanded="open"
    @click.stop="toggle"
  >
    ⋯
  </button>

  <!-- Teleported so the table's scroll container cannot clip it. -->
  <Teleport to="body">
    <div v-if="open" id="row-menu-layer">
      <div class="menu" role="menu" :style="style">
        <template v-for="(item, i) in items" :key="i">
          <div v-if="item.sep" class="sep" />
          <button
            type="button"
            role="menuitem"
            :class="{ danger: item.danger }"
            :disabled="item.disabled"
            @click.stop="pick(item)"
          >
            <span>{{ item.label }}</span>
            <span v-if="item.hint" class="hint">{{ item.hint }}</span>
          </button>
        </template>
      </div>
    </div>
  </Teleport>
</template>
