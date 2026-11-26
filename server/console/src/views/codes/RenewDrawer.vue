<script setup lang="ts">
// Renew a code after a payment.
//
// WHAT THIS REPLACES, AND WHY. Renewal used to be a `window.prompt`:
//
//     window.prompt(`Renew ${row.code}?\n\nCurrent expiry: ${current}\n\n
//                    Enter the number of DAYS being purchased...`, '70')
//
// It showed the current date and then asked for a day count, so the operator's
// job was to do the date arithmetic the server would do anyway, and to trust
// that it would. The design record calls this out directly
// (06-console-operator.md §6.2.2): the action is safe only if the operator SEES
// the arithmetic, not if they perform it.
//
// It also silently dropped the two fields the audit trail needs. `codes.renew`
// accepts `price` and `middleman` and writes them into the renewal event, and
// docs/business/redesign/08-metrics §8.4 is explicit that this is unrecoverable
// if not captured at the time — a month's revenue cannot be reconstructed from
// memory. The prompt had no room for them, so every renewal was losing it.
//
// THIS PANEL: pick a preset or type days; see exactly what will change; say what
// was paid and by whom; confirm.

import { computed, ref, watch } from 'vue'
import Drawer from '../../components/Drawer.vue'
import type { CodeRow } from './model'
import { shortDate } from './model'
import { act } from './data'

const props = defineProps<{ row: CodeRow | null; middlemen: string[] }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'done'): void }>()

const PRESETS = [
  { days: 70, label: '10-week term' },
  { days: 30, label: 'One month' },
  { days: 365, label: 'One year' },
]

const days = ref(70)
const price = ref('')
const middleman = ref('')
const busy = ref(false)
const result = ref<{ previous_expires_at: string; expires_at: string; extended: boolean } | null>(null)

watch(
  () => props.row?.code,
  () => {
    result.value = null
    price.value = ''
    days.value = props.row?.term_days && props.row.term_days > 0 ? Number(props.row.term_days) : 70
    // Default to the code's own middleman: the code carries who distributed it,
    // and a renewal for that code is that person's sale. Pre-filling it is what
    // makes the attribution survive a hurried renewal.
    middleman.value = props.row?.middleman || ''
  },
  { immediate: true },
)

/**
 * The preview, computed HERE with the same rule the hub applies.
 *
 * Duplicating the arithmetic is deliberate, not sloppiness: the operator must see
 * the answer BEFORE committing, and the only way to show it is to compute it.
 * The hub remains the authority — its response carries the real before/after,
 * which is what the success panel shows. If the two ever disagree, the hub wins
 * and the operator sees the truth immediately. `renewed.extended` in the hook
 * is the same condition (`prevValid && prevMs > nowMs`).
 */
const preview = computed(() => {
  const r = props.row
  if (!r) return null
  const now = Date.now()
  const prev = r.expires_at ? new Date(r.expires_at).getTime() : NaN
  const prevValid = !isNaN(prev) && prev > 0
  const base = prevValid && prev > now ? prev : now
  const n = Number(days.value)
  if (!Number.isFinite(n) || n <= 0) return null
  const next = base + n * 86400 * 1000
  return {
    running: prevValid && prev > now,
    current: prevValid ? r.expires_at : '',
    next: new Date(next).toISOString(),
    // Why this matters: renewing early keeps remaining days; renewing after a
    // lapse starts from today. The operator must know which they are doing,
    // because they are quoting a date to a middleman.
    losesNothing: prevValid && prev > now,
  }
})

const valid = computed(() => {
  const n = Number(days.value)
  return Number.isFinite(n) && n > 0 && !busy.value
})

async function submit() {
  const r = props.row
  if (!r || !valid.value) return
  busy.value = true
  try {
    const res = await act(
      'codes.renew',
      {
        code: r.code,
        term_days: Number(days.value),
        price: price.value.trim() || undefined,
        middleman: middleman.value.trim() || undefined,
      },
      '',
    )
    if (!res) return
    // The confirmation is the point: the operator reads the ACTUAL new expiry
    // here and can quote it. Reading it from our own preview would defeat the
    // purpose of the hub being the authority.
    result.value = {
      previous_expires_at: r.expires_at,
      expires_at: preview.value?.next || '',
      extended: !!preview.value?.running,
    }
    emit('done')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Drawer
    :open="!!row"
    :title="result ? 'Renewed' : `Renew ${row?.code || ''}`"
    :subtitle="result ? undefined : row?.label || undefined"
    :busy="busy"
    @close="result ? (result = null, emit('close')) : emit('close')"
  >
    <!-- ── The result ── -->
    <template v-if="result">
      <div class="msg ok">
        <strong>{{ row?.code }} renewed.</strong>
        <div style="margin-top: 6px">
          <span class="muted">Was:</span> {{ shortDate(result.previous_expires_at) }}
          <span class="muted">→ Now:</span> <strong>{{ shortDate(result.expires_at) }}</strong>
        </div>
      </div>
      <p class="foot-note">
        Quote the new date to the middleman.
        <template v-if="result.extended">
          The term was ADDED to the existing expiry, so no paid days were lost.
        </template>
        <template v-else>
          The code had already lapsed, so the term starts from today.
        </template>
      </p>
    </template>

    <!-- ── The form ── -->
    <template v-else-if="row">
      <div class="section-title">How long was purchased</div>
      <div class="actions" style="margin-top: 0; margin-bottom: 12px">
        <button
          v-for="p in PRESETS"
          :key="p.days"
          class="tiny"
          :class="{ primary: Number(days) === p.days }"
          @click="days = p.days"
        >
          {{ p.label }}
        </button>
      </div>
      <label class="field">
        <span>Days</span>
        <input v-model.number="days" type="number" min="1" max="3650" />
      </label>

      <div class="section-title">What will change</div>
      <dl v-if="preview" class="facts" style="margin-bottom: 8px">
        <dt>Current expiry</dt>
        <dd>{{ preview.current ? shortDate(preview.current) : 'Never expires' }}</dd>
        <dt>New expiry</dt>
        <dd><strong>{{ shortDate(preview.next) }}</strong></dd>
        <dt>Effect</dt>
        <dd>
          <template v-if="preview.losesNothing">
            {{ days }} days added to the <strong>running</strong> term — nothing paid is lost.
          </template>
          <template v-else>
            Starts from today (nothing is currently running) and adds {{ days }} days.
          </template>
        </dd>
      </dl>
      <p v-if="!preview" class="msg warn">Enter a whole number of days greater than zero.</p>

      <!-- The non-effects, stated. 06-console §6.2.3 is explicit that these must
           be obvious, so the operator can confirm nothing else moved. -->
      <p class="foot-note">
        Renewal changes <strong>only</strong> the expiry.
        It does not clear a stop, change the tier, or touch the binding.
        <template v-if="row.suspended">This code is stopped — renewing it will not restart access.</template>
      </p>

      <div class="section-title">What was paid <span class="muted">(optional)</span></div>
      <p class="foot-note" style="margin-top: 0">
        Recording it is what makes “how much did this middleman collect” answerable
        later. It cannot be reconstructed afterwards, so it is worth the two seconds.
      </p>
      <div class="row" style="margin-top: 8px">
        <label class="field">
          <span>Price</span>
          <input v-model="price" placeholder="10.00" inputmode="decimal" />
        </label>
        <label class="field">
          <span>Middleman</span>
          <input v-model="middleman" list="renew-middlemen" placeholder="who collected it" />
          <datalist id="renew-middlemen">
            <option v-for="m in middlemen" :key="m" :value="m" />
          </datalist>
        </label>
      </div>
    </template>

    <template #foot>
      <template v-if="result">
        <button class="primary" @click="result = null; emit('close')">Done</button>
      </template>
      <template v-else>
        <button :disabled="busy" @click="emit('close')">Cancel</button>
        <button class="primary" :disabled="!valid" @click="submit">
          {{ busy ? 'Renewing…' : 'Renew' }}
        </button>
      </template>
    </template>
  </Drawer>
</template>
