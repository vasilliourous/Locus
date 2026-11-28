<script setup lang="ts">
// Maintenance — the operations for a bad day.
//
// WHY THIS PAGE EXISTS AT ALL. Both halves already existed as hook actions and
// neither had any UI:
//
//   codes.import          — add codes the hub is MISSING. This is not cosmetic:
//                           heartbeat.pb.js answers 404 "Code not found", the
//                           client files that as a REFUSAL, and handle_refusal()
//                           stops the tunnel and DELETES the code from the
//                           device. The student is left logged in with no tunnel
//                           and no error, because the state that would have
//                           explained it was erased a beat earlier. So a
//                           reconciliation path is the remedy for the worst
//                           failure in the system — and it was reachable only by
//                           writing curl by hand.
//
//   releases.fetchLink    — mint a single-use, expiring link that triggers a
//                           fetch WITHOUT the admin token, so a release can be
//                           pulled from a phone or a CI job.
//
// A remedy nobody can reach is not a remedy, which is why these are here rather
// than deleted.

import { ref } from 'vue'
import { call, fetchLink } from '../../api'
import { toast } from '../../toast'

// ── Import ──
const importText = ref('')
const importTier = ref('')
const remapTo = ref('')
const importing = ref(false)
const importResult = ref<{
  created: string[]
  skipped: string[]
  remapped: string[]
  rejected: { code: string; reason: string }[]
} | null>(null)

interface TierOption {
  tier: string
  active: boolean
}

const tierOptions = ref<TierOption[]>([])

async function loadTiers() {
  const res = await call<{ tiers: TierOption[] }>('tiers.list')
  if (res.ok && res.data) tierOptions.value = res.data.tiers
}
void loadTiers()

/** Split whatever was pasted into candidate codes, one per line or comma. */
function parseCodes(): string[] {
  return importText.value
    .split(/[\n,;]+/)
    .map((s) => s.trim())
    .filter(Boolean)
}

async function runImport() {
  const codes = parseCodes()
  if (!codes.length) {
    toast.err('Paste at least one code.')
    return
  }
  if (!importTier.value.trim()) {
    toast.err('Name the tier these codes belong to.')
    return
  }
  importing.value = true
  importResult.value = null
  try {
    const res = await call<NonNullable<typeof importResult.value>>('codes.import', {
      codes,
      tier: importTier.value.trim(),
      remap_to: remapTo.value.trim() || undefined,
      // Duplicates are skipped, never errors: an import must never overwrite a
      // row that may already carry real history (activation, renewals).
      on_duplicate: 'skip',
    })
    if (!res.ok || !res.data) {
      toast.err(res.message || res.transportError || 'Import failed')
      return
    }
    importResult.value = res.data
    const made = res.data.created.length + res.data.remapped.length
    toast.ok(`Imported ${made}; ${res.data.skipped.length} already present; ${res.data.rejected.length} refused`)
  } finally {
    importing.value = false
  }
}

// ── One-shot fetch link ──
const linkVersion = ref('')
const link = ref('')
const linkBusy = ref(false)

async function mintLink() {
  const v = linkVersion.value.trim().replace(/^v/, '')
  if (!v) {
    toast.err('Enter a version first — the link is bound to one version.')
    return
  }
  linkBusy.value = true
  try {
    const res = await fetchLink(v)
    if (res.ok && res.data) {
      link.value = res.data.link
      toast.ok('Link minted — single use, and it expires')
    } else {
      toast.err(res.message || res.transportError || 'Could not mint a link')
    }
  } finally {
    linkBusy.value = false
  }
}

async function copyLink() {
  try {
    await navigator.clipboard.writeText(link.value)
    toast.ok('Copied')
  } catch {
    toast.warn('Could not copy — select the text and copy manually')
  }
}
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Maintenance</h1>
      <p class="page-sub">The operations that exist for a bad day: reconcile, trigger, export.</p>
    </div>
  </div>

  <!-- ── Import ── -->
  <div class="panel">
    <h2>Add codes the hub is missing</h2>
    <p class="page-sub" style="margin-bottom: 12px">
      For when the code set and the hub have drifted apart. A code the hub does not
      hold is not a cosmetic gap: the client treats the 404 as a refusal and
      <strong>deletes the code from the student's machine</strong>, with no error
      shown. This is the way to put it back.
    </p>

    <div class="row">
      <label class="field" style="flex: 2 1 320px">
        <span>Codes (one per line, or comma-separated)</span>
        <textarea v-model="importText" class="mono" placeholder="RQ-ABCD-EFGH-JKMN-T&#10;RQ-ABCD-EFGH-JKMP-2" spellcheck="false" />
      </label>
    </div>
    <div class="row">
      <label class="field">
        <span>Tier these codes belong to</span>
        <input v-model="importTier" list="import-tiers" placeholder="free" spellcheck="false" />
        <datalist id="import-tiers">
          <option v-for="t in tierOptions" :key="t.tier" :value="t.tier" />
        </datalist>
      </label>
      <label class="field">
        <span>Remap to <span class="muted">(optional)</span></span>
        <input v-model="remapTo" placeholder="only if the tier above has no config" spellcheck="false" />
      </label>
      <div class="shrink">
        <button class="primary" :disabled="importing" @click="runImport">
          {{ importing ? 'Importing…' : `Import ${parseCodes().length || 0} code(s)` }}
        </button>
      </div>
    </div>

    <p class="foot-note">
      A code already on the hub is <strong>skipped</strong>, never overwritten — it
      may already carry a student's activation and renewal history. Use Renew or
      the detail panel to change an existing code. A code whose tier has no active
      config row is <strong>refused</strong> rather than written, because it would
      activate into a tunnel with no server settings.
    </p>

    <template v-if="importResult">
      <div class="section-title">Result</div>
      <div v-if="importResult.created.length" class="msg ok">
        <strong>{{ importResult.created.length }} created.</strong>
        <div class="mono pre-wrap" style="margin-top: 6px; max-height: 140px; overflow: auto">{{ importResult.created.join('\n') }}</div>
      </div>
      <div v-if="importResult.remapped.length" class="msg warn">
        <strong>{{ importResult.remapped.length }} created under the remapped tier.</strong>
        <div class="mono pre-wrap" style="margin-top: 6px">{{ importResult.remapped.join('\n') }}</div>
      </div>
      <div v-if="importResult.rejected.length" class="msg err">
        <strong>{{ importResult.rejected.length }} refused — not written.</strong>
        <table class="data" style="margin-top: 6px">
          <tbody>
            <tr v-for="r in importResult.rejected" :key="r.code">
              <td class="mono" style="width: 1px">{{ r.code }}</td>
              <td class="muted">{{ r.reason }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div v-if="importResult.skipped.length" class="msg warn">
        {{ importResult.skipped.length }} already on the hub and left untouched.
      </div>
    </template>
  </div>

  <!-- ── Trigger link ── -->
  <div class="panel">
    <h2>One-shot fetch link</h2>
    <p class="page-sub" style="margin-bottom: 12px">
      Triggers a GitHub fetch for one version <strong>without the admin token</strong>
      — usable from a phone or a CI job. Every link is single-use and expires; a used
      or expired one is refused. Mint a fresh link every time.
    </p>
    <div class="row">
      <label class="field">
        <span>Version</span>
        <input v-model="linkVersion" placeholder="2.2.1" spellcheck="false" />
      </label>
      <div class="shrink">
        <button :disabled="linkBusy" @click="mintLink">{{ linkBusy ? 'Minting…' : 'New link' }}</button>
      </div>
      <div v-if="link" class="shrink">
        <button @click="copyLink">Copy</button>
      </div>
    </div>
    <p v-if="link" class="mono breakall">{{ link }}</p>
  </div>

  <!-- ── Export ── -->
  <div class="panel">
    <h2>Export</h2>
    <p class="page-sub" style="margin-bottom: 0">
      The Codes page exports whatever is currently filtered, from its row menu. There
      is no separate export here, deliberately — a second export path is how two
      “export” buttons end up producing different columns.
    </p>
  </div>
</template>
