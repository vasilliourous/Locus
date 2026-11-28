<script setup lang="ts">
// Overview — is anything wrong, and where do I go to fix it?
//
// THE PROBLEM THIS SOLVES. The old Dashboard was five count cards and three
// tables that could not be acted on. Its numbers were also partly wrong: the
// card labelled "Activated on a device" read `data.codes.bound`, and the hook
// had stopped returning `bound` when device binding was removed — so that card
// rendered blank. And `expiringSoon` was counted over 30 days while the
// clickable card opened a 30-day worklist the row buttons resolved in 7.
//
// So: every number here is a BUTTON that lands on the rows it counts, with the
// filter already applied. A number you cannot click is a number you have to
// remember and re-enter somewhere else.
//
// The bottom half is the attention list: four checks over data that already
// exists (middlemen.summary's sibling — see `attention.list` in
// admin_console.pb.js). Each names a failure this project has paid for, states it
// in a sentence, and links to the fix.

import { computed, onMounted, ref } from 'vue'
import { call } from '../api'
import { openCodes, go } from '../nav'
import StateBlock from '../components/StateBlock.vue'
import { shortDateTime } from './codes/model'

interface Dash {
  codes: {
    total: number
    available: number
    redeemed: number
    suspended: number
    expired: number
    byTier: Record<string, number>
    expiringSoon: number
  }
  attempts: { total: number; activate: number; lookup: number; window_minutes: number }
  recent: { code: string; event: string; detail: string; created: string }[]
}

interface Attention {
  total_codes: number
  tiers_served: string[]
  counts: { unservable_tier: number; lapsed: number; shared: number; no_label: number; gone_quiet: number }
  groups: Record<string, { code: string; label?: string; middleman?: string; tier?: string; days_past?: number; release_count?: number; released_at?: string; expires_at?: string; activated_at?: string }[]>
}

const dash = ref<Dash | null>(null)
const attn = ref<Attention | null>(null)
const loading = ref(true)
const failed = ref('')
const denied = ref(false)

async function load() {
  loading.value = true
  failed.value = ''
  // Both in parallel: they are independent reads and the page is useless with
  // only one of them.
  const [d, a] = await Promise.all([
    call<Dash>('dashboard'),
    call<Attention>('attention.list'),
  ])
  if (d.message && /sign in again/i.test(d.message)) denied.value = true
  loading.value = false
  if (!d.ok || !d.data) {
    failed.value = d.message || d.transportError || 'Could not load the overview.'
    return
  }
  dash.value = d.data
  attn.value = a.ok && a.data ? a.data : null
}

onMounted(load)

/** The triage cards. Each is a filter, so the numbers and the links cannot drift. */
const cards = computed(() => {
  const d = dash.value
  if (!d) return []
  const a = attn.value
  return [
    {
      id: 'term',
      n: d.codes.redeemed,
      label: 'In term',
      sub: 'codes students are using',
      tone: '',
      run: () => openCodes({ status: 'redeemed' }),
    },
    {
      id: 'due',
      n: d.codes.expiringSoon,
      label: 'Due in 30 days',
      sub: 'renew these to keep them',
      tone: d.codes.expiringSoon ? 'warn' : 'quiet',
      run: () => openCodes({ view: 'worklist', expiring_within_days: 30 }),
    },
    {
      id: 'unused',
      n: d.codes.available,
      label: 'Unused stock',
      sub: 'minted, not yet sold',
      tone: 'good',
      run: () => openCodes({ status: 'available' }),
    },
    {
      id: 'lapsed',
      n: d.codes.expired,
      label: 'Lapsed',
      sub: 'access already stopped',
      tone: d.codes.expired ? 'warn' : 'quiet',
      run: () => openCodes({ status: 'expired' }),
    },
    {
      id: 'stopped',
      n: d.codes.suspended,
      label: 'Stopped',
      sub: 'cut off by an operator',
      tone: d.codes.suspended ? 'bad' : 'quiet',
      run: () => openCodes({ status: 'suspended' }),
    },
  ]
})

/** "3 of 4 problems": the count that decides whether the page is fine. */
const problems = computed(() => {
  const a = attn.value
  if (!a) return 0
  // Deliberately NOT counting `lapsed`: an overdue renewal is the normal state of
  // a subscription business, and folding it in would leave this page saying
  // "needs attention" forever — which trains the operator to ignore the section.
  // Lapsed codes surface in their own check, below the three that are faults.
  return a.counts.unservable_tier + a.counts.shared + a.counts.no_label + a.counts.gone_quiet
})

const checks = computed(() => {
  const a = attn.value
  if (!a) return []
  return [
    {
      id: 'unservable',
      n: a.counts.unservable_tier,
      tone: 'bad',
      what: 'Codes on a tier the hub cannot serve',
      why: 'These activate “successfully” and hand the device no server config: a connected app that reaches no internet.',
      rows: (a.groups.unservable_tier || []).map((r) => ({
        key: r.code,
        main: r.code,
        detail: `${r.tier}${r.middleman ? ' · ' + r.middleman : ''}`,
      })),
      open: () => go('settings'),
      openLabel: 'Fix in Settings › Tiers',
    },
    {
      id: 'lapsed',
      n: a.counts.lapsed,
      tone: 'warn',
      what: 'Lapsed and not stopped',
      why: 'The term has ended, so the student is already cut off. These need a catch-up, not a reminder.',
      rows: (a.groups.lapsed || []).map((r) => ({
        key: r.code,
        main: r.label || r.code,
        detail: `${r.days_past}d past · ${r.middleman || 'no middleman'}`,
      })),
      open: () => openCodes({ status: 'expired' }),
      openLabel: 'Open in Codes',
    },
    {
      id: 'shared',
      n: a.counts.shared,
      tone: 'warn',
      // WHAT THIS SAYS, AND WHY IT CHANGED. The hub now refuses a redeemed code
      // on a second device (activation.pb.js), so "in use in two places at once"
      // is no longer a state a code can be in — a careless holder leaves no
      // trace of a second machine, and this card must not imply one. What it
      // CAN see is a sign-out nobody claimed: the code is free, still has term
      // left, and the student has not come back. That is either a move in
      // progress or a card handed to someone who has not activated it yet.
      what: 'Signed out and not claimed again, with term left',
      why: 'A move in progress normally claims the code within minutes. Sitting here for days means the key is free — and a code that is free is one somebody else can claim with nothing but the card.',
      rows: (a.groups.shared || []).map((r) => ({
        key: r.code,
        main: r.label || r.code,
        detail: `free since ${shortDateTime(r.released_at).split(',')[0]}`,
      })),
      open: () => openCodes({ view: 'all' }),
      openLabel: 'Open in Codes',
    },
    {
      id: 'quiet',
      n: a.counts.gone_quiet,
      tone: 'warn',
      // NOT 'bad'. A device that has stopped checking in is usually a closed
      // laptop, and calling that a fault would make this card red every night on
      // a healthy fleet. It is `warn` because the unremarkable causes and the
      // serious ones (a hub outage, a bad hook deploy, a school blocking the
      // hostname) look identical from here — and the only way to tell is to look.
      what: 'In term, but the device has stopped checking in',
      why: 'Usually a closed laptop. It is also what a hub outage, a stale hook deploy or a blocked school network look like — so once it is more than a handful, check the hub before the students (Runbook: desync).',
      rows: (a.groups.gone_quiet || []).map((r) => ({
        key: r.code,
        main: r.label || r.code,
        detail: `quiet ${r.hours_quiet}h · ${r.middleman || 'no middleman'}`,
      })),
      open: () => openCodes({ status: 'redeemed' }),
      openLabel: 'Open in Codes',
    },
    {
      id: 'nolabel',
      n: a.counts.no_label,
      tone: 'warn',
      what: 'In use with no name recorded',
      why: 'The label is the only link between a student and their code. Without it, a support call cannot be answered.',
      rows: (a.groups.no_label || []).map((r) => ({
        key: r.code,
        main: r.code,
        detail: `${r.middleman || 'no middleman'} · since ${shortDateTime(r.activated_at).split(',')[0]}`,
      })),
      open: () => openCodes({ status: 'redeemed' }),
      openLabel: 'Open in Codes',
    },
  ]
})

const worst = computed(() => checks.value.filter((x) => x.n > 0).length)

function feedLink(code: string) {
  // The activity feed carries pseudo-codes for release and tier events
  // ("(tier strike)"). Those are not codes and cannot be opened in Codes — they
  // are shown, but not linked, rather than linking to a guaranteed empty page.
  if (!code || code.startsWith('(')) return
  openCodes({ query: code })
}
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Overview</h1>
      <p class="page-sub">What needs attention, and the way in to each of it.</p>
    </div>
    <button @click="load">Refresh</button>
  </div>

  <StateBlock v-if="loading && !dash" state="loading" :rows="4" />
  <StateBlock v-else-if="denied" state="denied" @retry="load" />
  <StateBlock v-else-if="failed" state="error" :message="failed" @retry="load" />

  <template v-else-if="dash">
    <!-- ── Counts, each a filter ── -->
    <div class="stat-grid">
      <button v-for="c in cards" :key="c.id" class="stat" :class="c.tone" @click="c.run()">
        <span class="n">{{ c.n }}</span>
        <span class="l">{{ c.label }}</span>
        <span class="sub">{{ c.sub }}</span>
      </button>
    </div>

    <!-- ── Attention ── -->
    <div class="section-title">
      Needs attention
      <span v-if="attn" class="n">
        {{ problems ? `${problems} to fix` : worst ? 'nothing to fix' : 'all clear' }}
      </span>
    </div>

    <div v-if="!attn" class="msg warn">
      The checks could not be loaded, so this page cannot say whether anything is
      wrong. The counts above are still current.
    </div>

    <div v-else-if="!worst" class="msg ok">
      Nothing needs attention: every tier can be served, no code has lapsed
      unrenewed, and every code in use has a name recorded.
    </div>

    <template v-else>
      <details v-for="c in checks.filter((x) => x.n > 0)" :key="c.id" class="check">
        <summary>
          <span class="dot" :class="c.tone" />
          <span class="what">
            <strong>{{ c.what }}</strong>
            <span>{{ c.why }}</span>
          </span>
          <span class="count">{{ c.n }}</span>
        </summary>
        <div class="body">
          <table v-if="c.rows.length" class="data" style="margin: 12px 0">
            <tbody>
              <tr v-for="r in c.rows" :key="r.key">
                <td class="mono">{{ r.main }}</td>
                <td class="muted">{{ r.detail }}</td>
              </tr>
            </tbody>
          </table>
          <div class="actions" style="margin-top: 0">
            <button class="tiny primary" @click="c.open()">{{ c.openLabel }}</button>
            <span v-if="c.n > c.rows.length" class="muted" style="font-size: 12px; align-self: center">
              showing {{ c.rows.length }} of {{ c.n }}
            </span>
          </div>
        </div>
      </details>
    </template>

    <!-- ── Activity ── -->
    <div class="section-title">Recent activity</div>
    <div class="table-wrap">
      <div v-if="!dash.recent.length" class="state">
        <strong>Nothing yet</strong>
        <p>Activity appears here as codes are minted, activated, renewed and stopped.</p>
      </div>
      <table v-else class="data">
        <tbody>
          <tr v-for="(ev, i) in dash.recent" :key="i">
            <td class="muted nowrap" style="width: 1px">{{ shortDateTime(ev.created) }}</td>
            <td class="mono" style="width: 1px">
              <button
                v-if="!ev.code.startsWith('(')"
                style="border: none; background: none; padding: 0; color: var(--accent); font: inherit"
                @click="feedLink(ev.code)"
              >
                {{ ev.code }}
              </button>
              <span v-else class="muted">{{ ev.code }}</span>
            </td>
            <td style="width: 1px">{{ ev.event }}</td>
            <td class="muted truncate">{{ ev.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- ── Attempts ──
         Kept, but scoped to what it can honestly say. `activation_attempts` is
         purged by the hooks at ten minutes, so this is a live gauge, not history:
         a spike in REFUSED ACTIVATIONS is "someone cannot get in", a spike in
         LOOKUPS is "someone is guessing". They read the same as a single number,
         which is why the old panel could not be acted on. -->
    <p class="foot-note">
      <strong>Check-ins, last {{ dash.attempts.window_minutes }} minutes:</strong>
      {{ dash.attempts.activate }} activation attempt(s), {{ dash.attempts.lookup }} code lookup(s).
      Recorded codes are dropped after {{ dash.attempts.window_minutes }} minutes, so this is a live gauge
      rather than a history — a steady stream here is a mistyped card or someone guessing.
    </p>
  </template>
</template>
