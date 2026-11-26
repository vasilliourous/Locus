<script setup lang="ts">
// The detail panel (slide-over from the right).
//
// Replaces the old centred modal, for a specific reason: the modal covered the
// list, so "renew this one, then the next" meant close → find the row again →
// open. A slide-over leaves the list visible and clickable behind a light scrim,
// which turns a worklist into a rhythm.
//
// Keyboard: Escape closes, focus moves into the panel on open and returns to
// whatever opened it on close (otherwise a keyboard user lands back at the top
// of the document after every close).

import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    /** Widens the panel for content that needs it (a history table). */
    wide?: boolean
  }>(),
  { wide: false },
)

const emit = defineEmits<{ (e: 'close'): void }>()

const panel = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null

function onKey(e: KeyboardEvent) {
  if (!props.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('close')
    return
  }
  // A focus trap, because the list behind is scrolled out of view but still in
  // the tab order: without it, Tab walks invisibly through 200 rows.
  if (e.key !== 'Tab' || !panel.value) return
  const focusables = panel.value.querySelectorAll<HTMLElement>(
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  )
  if (!focusables.length) return
  const first = focusables[0]
  const last = focusables[focusables.length - 1]
  const active = document.activeElement as HTMLElement | null
  if (e.shiftKey && active === first) {
    e.preventDefault()
    last.focus()
  } else if (!e.shiftKey && active === last) {
    e.preventDefault()
    first.focus()
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      returnFocus = document.activeElement as HTMLElement | null
      await nextTick()
      const firstControl = panel.value?.querySelector<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled])',
      )
      ;(firstControl || panel.value)?.focus()
    } else {
      returnFocus?.focus?.()
      returnFocus = null
    }
  },
)

onMounted(() => document.addEventListener('keydown', onKey))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <template v-if="open">
      <div class="scrim" @click="emit('close')" />
      <aside
        ref="panel"
        class="slideover"
        :style="wide ? { width: 'min(760px, 100vw)' } : undefined"
        role="dialog"
        aria-modal="true"
        :aria-label="title || 'Detail'"
        tabindex="-1"
      >
        <div class="so-head">
          <div style="flex: 1; min-width: 0">
            <slot name="head" />
          </div>
          <button class="row-menu" type="button" aria-label="Close" @click="emit('close')">✕</button>
        </div>
        <div class="so-body">
          <slot />
        </div>
        <div v-if="$slots.foot" class="so-foot">
          <slot name="foot" />
        </div>
      </aside>
    </template>
  </Teleport>
</template>
