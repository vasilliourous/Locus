<script setup lang="ts">
// Mint codes.
//
// WHY THIS IS A DRAWER NOW. It used to be a permanent panel above the list, two
// rows of inputs deep, which pushed the actual data below the fold on every visit
// — and the operator opens this page far more often to find a code than to mint
// one. Minting is a burst activity (a batch, then nothing for a week).
//
// The FORM is little changed, deliberately. It works, its defaults are right
// (79-day term ≈ a school term, tier pre-selected, count 10), and the copy-once
// result is the single most-used part of the console. What changed:
//
//   * the term/expiry relationship is explained where it is decided, because
//     "which one wins" was the question the old labels raised and did not answer;
//   * `labelled` is counted, since 08-distribution §8.3.1 makes the label the
//     only link between a student and their code — a batch minted without one is
//     a batch of orphans;
//   * the result list keeps its copy button and gains a CSV download, so a batch
//     can be handed on as a file.

import { computed, ref, watch } from 'vue'
import Drawer from '../../components/Drawer.vue'
import { call } from '../../api'
import { toast } from '../../toast'

const props = defineProps<{ open: boolean; tiers: string[]; middlemen: string[] }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'created'): void }>()

const tier = ref('free')
const count = ref(10)
const termDays = ref(70)
const expires = ref('')
const middleman = ref('')
const label = ref('')
const notes = ref('')
const busy = ref(false)
const created = ref<string[]>([])
const skipped = ref(0)

// The tier list comes from `tiers.list`, so it can never offer a tier the hub
// cannot serve — the failure check-consistency.sh §27(f) exists to prevent.
watch(
  () => props.tiers,
  (t) => {
    if (t.length && !t.includes(tier.value)) tier.value = t[0]
  },
  { immediate: true },
)

// Reset the one-shot result when the drawer is reopened, so a stale batch is
// never mistaken for the one just minted.
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      created.value = []
      skipped.value = 0
    }
  },
)

const countError = computed(() => {
  const n = Number(count.value)
  if (!Number.isFinite(n) || n < 1) return 'At least one code.'
  if (n > 500) return 'The hub mints at most 500 at a time.'
  return ''
})

const termError = computed(() => {
  const n = Number(termDays.value)
  if (!Number.isFinite(n) || n < 0) return 'Days, or 0 for a code that never expires.'
  return ''
})

const canCreate = computed(() => !!tier.value && !countError.value && !termError.value && !busy.value)

async function create() {
  if (!canCreate.value) return
  busy.value = true
  try {
    const res = await call<{ created: string[]; skipped: number }>('codes.generate', {
      tier: tier.value,
      count: Number(count.value),
      // Both are sent when both are set. The hub treats the term as the
      // authority AT ACTIVATION (it recomputes the expiry then); the absolute
      // date is the mint-time value a code carries until it is used. The two do
      // not fight — but the operator should know which applies, hence the hint.
      term_days: Number(termDays.value) || 0,
      expires_at: expires.value,
      middleman: middleman.value.trim(),
      label: label.value.trim(),
      notes: notes.value.trim(),
    })
    if (!res.ok || !res.data) {
      toast.err(res.message || res.transportError || 'Could not create codes')
      return
    }
    created.value = res.data.created || []
    skipped.value = res.data.skipped || 0
    toast.ok(
      `Created ${created.value.length} code(s)` + (skipped.value ? `, ${skipped.value} skipped` : ''),
    )
    emit('created')
  } finally {
    busy.value = false
  }
}

async function copyAll() {
  try {
    await navigator.clipboard.writeText(created.value.join('\n'))
    toast.ok(`Copied ${created.value.length} code(s)`)
  } catch {
    toast.warn('Could not copy automatically — select the text and copy manually')
  }
}

function downloadCsv() {
  const rows = [['code', 'tier', 'middleman', 'label', 'term_days']]
  for (const c of created.value) {
    rows.push([c, tier.value, middleman.value, label.value, String(termDays.value)])
  }
  const csv = rows.map((r) => r.map((v) => `"${String(v).replace(/"/g, '""')}"`).join(',')).join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `chrysalis-batch-${new Date().toISOString().slice(0, 10)}.csv`
  a.click()
  URL.revokeObjectURL(a.href)
}
</script>

<template>
  <Drawer :open="open" title="Create codes" subtitle="A batch is minted under one tier and one set of details." :busy="busy" @close="emit('close')">
    <!-- ── Result ── -->
    <template v-if="created.length">
      <div class="msg ok">
        <strong>Created {{ created.length }} code(s).</strong>
        <span v-if="skipped" class="muted"> {{ skipped }} skipped — the hub refused them (usually a duplicate).</span>
      </div>
      <p class="foot-note" style="margin-top: 0">
        This list is shown once. Copy it or download it now — it is the only time
        the console shows the batch as a set.
      </p>
      <div class="pre-wrap mono" style="max-height: 280px; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 10px; margin-top: 10px">{{ created.join('\n') }}</div>
      <div class="actions" style="margin-top: 12px">
        <button class="primary" @click="copyAll">Copy all</button>
        <button @click="downloadCsv">Download CSV</button>
      </div>
      <div class="section-title">Mint another batch</div>
      <button @click="created = []">Back to the form</button>
    </template>

    <!-- ── Form ── -->
    <template v-else>
      <div class="row">
        <label class="field">
          <span>Tier</span>
          <select v-model="tier">
            <option v-for="t in tiers" :key="t" :value="t">{{ t }}</option>
          </select>
        </label>
        <label class="field">
          <span>How many</span>
          <input v-model.number="count" type="number" min="1" max="500" />
        </label>
      </div>
      <p v-if="countError" class="foot-note" style="margin-top: -6px; color: var(--warn)">{{ countError }}</p>

      <div class="section-title">Term</div>
      <p class="foot-note" style="margin-top: 0">
        The term is how long <strong>one purchase</strong> lasts, counted from
        <strong>activation</strong> — so a card does not age on the shelf while it
        waits to be sold. Leave the absolute date blank.
      </p>
      <div class="row" style="margin-top: 8px">
        <label class="field">
          <span>Term (days from activation)</span>
          <input v-model.number="termDays" type="number" min="0" placeholder="70" />
        </label>
        <label class="field">
          <span>Absolute expiry <span class="muted">(override)</span></span>
          <input v-model="expires" type="date" />
        </label>
      </div>
      <p v-if="termError" class="foot-note" style="margin-top: -6px; color: var(--warn)">{{ termError }}</p>
      <p v-else-if="expires" class="foot-note" style="margin-top: -6px">
        Both are set. The <strong>term wins at activation</strong>; the date only
        applies to a code that is never activated.
      </p>

      <div class="section-title">Who it is for</div>
      <label class="field">
        <span>Middleman</span>
        <input v-model="middleman" list="create-middlemen" placeholder="e.g. Sarah" />
        <datalist id="create-middlemen">
          <option v-for="m in middlemen" :key="m" :value="m" />
        </datalist>
      </label>
      <label class="field">
        <span>Name / label <span class="muted">(optional)</span></span>
        <input v-model="label" placeholder="e.g. Macleans Year 13" />
        <span v-if="!label.trim()" class="foot-note" style="display: block">
          Without a label, this code cannot be found by a student's name later.
        </span>
      </label>
      <label class="field">
        <span>Notes <span class="muted">(optional)</span></span>
        <input v-model="notes" placeholder="free-form" />
      </label>
    </template>

    <template #foot>
      <template v-if="created.length">
        <button @click="emit('close')">Close</button>
      </template>
      <template v-else>
        <button :disabled="busy" @click="emit('close')">Cancel</button>
        <button class="primary" :disabled="!canCreate" @click="create">
          {{ busy ? 'Creating…' : `Create ${Number(count) || 0} code(s)` }}
        </button>
      </template>
    </template>
  </Drawer>
</template>
