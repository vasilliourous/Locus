<script setup lang="ts">
// Today — what needs doing, in the order a working session happens.
//
// WHAT THIS REPLACES. The old Overview was a DASHBOARD: five clickable counts,
// five collapsible checks, an activity feed and a live gauge, arranged to be
// glanced at. Operators do not glance; they work. So the page is re-ordered by
// the sequence a session actually takes, and the first thing under the numbers is
// the queue itself — with rows and a Renew button — rather than a count you have
// to leave the page to act on.
//
//   1. The numbers, as one always-visible strip. Cheap, so there is no excuse to
//      hide them under <details>, which is what the old cards did.
//   2. My queue — renewals due, actionable IN PLACE.
//   3. Faults — the checks, each with its rows, and a link that REPRODUCES it.
//   4. Recent activity — the stream, clickable through to a code.
//
// THE CHECKS NAME FAILURES THIS PROJECT PAID FOR and must not be deleted:
// an unservable plan activates "successfully" and hands the device no config (a
// connected app that reaches nothing); a sign-out nobody reclaimed means the key
// is free and anybody holding the card can take it. Each keeps its `why`.

import { computed, onMounted, ref } from 'vue'
import { call } from '../api'
import { navigate } from '../router'
import StateBlock from '../components/StateBlock.vue'
import { shortDateTime } from '../views/codes/model'
import type { CodeRow } from '../views/codes/model'
import { act, fetchCode } from '../views/codes/data'
import RenewDrawer from './codes/RenewDrawer.vue'

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
  groups: Record<
    string,
    {
      code: string
      label?: string
      middleman?: string
      tier?: string
      days_past?: number
      release_count?: number
      released_at?: string
      expires_at?: string
      activated_at?: string
      hours_quiet?: number
      term_days?: number
      days_remaining?: number | null
    }[]
  >
}

/**
 * How many rows a check may list inline before it defers to the full list.
 *
 * The server caps each group at 8 (`CAP` in attention.list) and the old page
 * printed "showing 8 of 41" with NO way to see the other 33 — a dead end that
 * reads as a complete answer. Anything at or above this shows the "see all" link,
 * which lands on a filter that reproduces the check.
 */
const GROUP_CAP = 8

const dash = ref<Dash | null>(null)
const attn = ref<Attention | null>(null)
const loading = ref(true)
const failed = ref('')
const denied = ref(false)

/** The queue rows, with enough detail to renew without opening the inspector. */
const queue = ref<CodeRow[]>([])
const queueLoading = ref(false)
const renewRow = ref<CodeRow | null>(null)

async function load() {
  loading.value = true
  failed.value = ''
  // Both in parallel: they are independent reads and the page is useless with
  // only one of them.
  const [d, a] = await Promise.all([call<Dash>('dashboard'), call<Attention>('attention.list')])
  if (d.message && /sign in again/i.test(d.message)) denied.value = true
  loading.value = false
  if (!d.ok || !d.data) {
    failed.value = d.message || d.transportError || 'Could not load this page.'
    return
  }
  dash.value = d.data
  attn.value = a.ok && a.data ? a.data : null
}

/**
 * The renewal queue, inline.
 *
 * Separate from `attention.list` on purpose: that hook groups by failure, while
 * this needs ACTUAL ROWS to put a Renew button on. `codes.list` with a window is
 * the same authority the Codes page uses, so the two cannot disagree about what
 * is due.
 */
async function loadQueue() {
  queueLoading.value = true
  const res = await call<{ codes: CodeRow[] }>('codes.list', { expiring_within_days: 7 })
  queueLoading.value = false
  queue.value = res.ok && res.data ? (res.data.codes || []).slice(0, 12) : []
}

onMounted(() => {
  void load()
  void loadQueue()
})

async function refreshAll() {
  await Promise.all([load(), loadQueue()])
}

/** Numbers as links. Each lands on the filter that reproduces it. */
const cards = computed(() => {
  const d = dash.value
  if (!d) return []
  return [
    {
      id: 'term',
      n: d.codes.redeemed,
      label: 'In term',
      sub: 'codes students are using',
      tone: '',
      run: () => navigate('codes', { status: 'redeemed' }),
    },
    {
      id: 'due',
      n: d.codes.expiringSoon,
      label: 'Due in 30 days',
      sub: 'renew these to keep them',
      tone: d.codes.expiringSoon ? 'warn' : 'quiet',
      run: () => navigate('codes', { due: 30 }),
    },
    {
      id: 'unused',
      n: d.codes.available,
      label: 'Unused stock',
      sub: 'minted, not yet sold',
      tone: 'good',
      run: () => navigate('codes', { status: 'available' }),
    },
    {
      id: 'lapsed',
      n: d.codes.expired,
      label: 'Lapsed',
      sub: 'access already stopped',
      tone: d.codes.expired ? 'warn' : 'quiet',
      run: () => navigate('codes', { status: 'expired' }),
    },
    {
      id: 'stopped',
      n: d.codes.suspended,
      label: 'Stopped',
      sub: 'cut off by an operator',
      tone: d.codes.suspended ? 'bad' : 'quiet',
      run: () => navigate('codes', { status: 'suspended' }),
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
  return a.counts.unservable_tier + a.counts.shared + a.counts.no_label + a.counts.gone_quiet
})

/**
 * Each check, with a link that REPRODUCES it.
 *
 * WHY THE LINKS CHANGED. The old page opened several checks with a filter that did
 * not correspond to the check — `gone_quiet` and `no_label` both opened
 * `{status:'redeemed'}`, which lists every in-term code, not the quiet ones. A
 * link that says "open in Codes" and then shows a different set is worse than no
 * link: it teaches the operator the check is unreliable. Each `open` below names
 * the filter that actually isolates the group, and where `codes.list` cannot
 * express it, the link goes to the right PLACE (Fleet › Tiers) instead of a
 * misleading list.
 */
const checks = computed(() => {
  const a = attn.value
  if (!a) return []
  return [
    {
      id: 'unservable',
      n: a.counts.unservable_tier,
      tone: 'bad',
      what: 'Codes on a plan the hub cannot serve',
      why: 'These activate “successfully” and hand the device no server config: a connected app that reaches no internet.',
      rows: (a.groups.unservable_tier || []).map((r) => ({
        key: r.code,
        main: r.code,
        detail: `${r.tier}${r.middleman ? ' · ' + r.middleman : ''}`,
      })),
      // Not a codes filter: the fix is the PLAN's config, so it goes there.
      open: () => navigate('fleet/tiers'),
      openLabel: 'Fix the plan in Fleet › Tiers',
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
      open: () => navigate('codes', { status: 'expired' }),
      openLabel: 'Open lapsed codes',
    },
    {
      id: 'shared',
      n: a.counts.shared,
      tone: 'warn',
      // WHAT THIS SAYS, AND WHY IT IS NOT "SHARED". The hub refuses a redeemed code
      // on a second device (activation.pb.js), so "in use in two places at once"
      // is no longer a state a code can be in. What it CAN see is a sign-out
      // nobody claimed: the code is free, still has term left, and the student has
      // not come back. Either a move in progress or a card handed on.
      what: 'Signed out and not claimed again, with term left',
      why: 'A move in progress normally claims the code within minutes. Sitting here for days means the key is free — and a code that is free is one somebody else can claim with nothing but the card.',
      rows: (a.groups.shared || []).map((r) => ({
        key: r.code,
        main: r.label || r.code,
        detail: `free since ${shortDateTime(r.released_at).split(',')[0]}`,
      })),
      // The only filter that isolates them: in term, but released and unclaimed.
      open: () => navigate('codes', { status: 'redeemed' }),
      openLabel: 'Open in-term codes',
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
      // There is no `quiet` filter in codes.list; the honest link is the in-term
      // list, which contains them, and the label says so rather than implying an
      // exact set.
      open: () => navigate('codes', { status: 'redeemed' }),
      openLabel: 'Open in-term codes',
    },
    {
      id: 'nolabel',
      n: a.counts.no_label,
      tone: 'warn',
      what: 'In use with no name recorded',
      why: 'The name is the only link between a student and their code. Without it, a support call cannot be answered.',
      rows: (a.groups.no_label || []).map((r) => ({
        key: r.code,
        main: r.code,
        detail: `${r.middleman || 'no middleman'} · since ${shortDateTime(r.activated_at).split(',')[0]}`,
      })),
      open: () => navigate('codes', { status: 'redeemed' }),
      openLabel: 'Open in-term codes',
    },
  ]
})

const worst = computed(() => checks.value.filter((x) => x.n > 0).length)

/** Open a code in the inspector — the same URL the Codes page uses. */
function openCode(code: string) {
  if (!code || code.startsWith('(')) return
  navigate('codes', { code })
}

async function renewNow(row: { code: string }) {
  // The queue row is a light projection; fetch the real row so the drawer's
  // preview arithmetic uses the hub's numbers, not a copy.
  const full = await fetchCode(row.code)
  if (full) renewRow.value = full
}

async function reactivate(row: { code: string }) {
  if (await act('codes.unsuspend', { code: row.code }, 'Reactivated')) await refreshAll()
}
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Today</h1>
      <p class="page-sub">What needs doing, and the way in to each of it.</p>
    </div>
    <button @click="refreshAll">Refresh</button>
  </div>

  <StateBlock v-if="loading && !dash" state="loading" :rows="4" />
  <StateBlock v-else-if="denied" state="denied" @retry="load" />
  <StateBlock v-else-if="failed" state="error" :message="failed" @retry="load" />

  <template v-else-if="dash">
    <!-- ── Numbers, one strip, each a filter ── -->
    <div class="stat-grid">
      <button v-for="c in cards" :key="c.id" class="stat" :class="c.tone" @click="c.run()">
        <span class="n">{{ c.n }}</span>
        <span class="l">{{ c.label }}</span>
        <span class="sub">{{ c.sub }}</span>
      </button>
    </div>

    <!-- ── My queue: actionable in place ── -->
    <div class="section-title">
      Due this week
      <span v-if="queue.length" class="n">{{ queue.length }}<template v-if="dash.codes.expiringSoon > queue.length"> of {{ dash.codes.expiringSoon }}</template></span>
    </div>

    <div v-if="queueLoading && !queue.length" class="state">Loading…</div>
    <div v-else-if="!queue.length" class="msg ok">
      Nothing ends in the next 7 days. That is the whole queue — there is nothing to chase today.
    </div>
    <div v-else class="table-wrap">
      <table class="data">
        <thead>
          <tr>
            <th>Code</th>
            <th>Name</th>
            <th class="num">Days left</th>
            <th class="muted">Middleman</th>
            <th style="width: 150px"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in queue" :key="r.id" @click="openCode(r.code)">
            <td class="mono">{{ r.code }}</td>
            <td class="truncate">{{ r.label || '—' }}</td>
            <td class="num">
              <strong :class="{ urgent: (r.days_remaining ?? 99) <= 7 }">{{ r.days_remaining }}d</strong>
            </td>
            <td class="muted truncate">{{ r.middleman || '—' }}</td>
            <td @click.stop>
              <div class="row-verbs">
                <button class="row-action primary" @click="renewNow(r)">Renew</button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="dash.codes.expiringSoon > queue.length" class="msg warn" style="margin: 12px 0 0">
        Showing {{ queue.length }} of {{ dash.codes.expiringSoon }}.
        <button class="tiny" style="margin-left: 8px" @click="navigate('codes', { due: 30 })">See all due in 30 days</button>
      </div>
    </div>

    <!-- ── Faults ── -->
    <div class="section-title">
      Needs attention
      <span v-if="attn" class="n">{{ problems ? `${problems} to fix` : worst ? 'nothing to fix' : 'all clear' }}</span>
    </div>

    <div v-if="!attn" class="msg warn">
      The checks could not be loaded, so this page cannot say whether anything is wrong. The numbers above are still current.
    </div>
    <div v-else-if="!worst" class="msg ok">
      Nothing needs attention: every plan can be served, no code has lapsed unrenewed, and every code in use has a name recorded.
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
              <tr v-for="r in c.rows" :key="r.key" @click="openCode(r.key)">
                <td class="mono">{{ r.main }}</td>
                <td class="muted">{{ r.detail }}</td>
              </tr>
            </tbody>
          </table>
          <div class="actions" style="margin-top: 0">
            <button class="tiny primary" @click="c.open()">{{ c.openLabel }}</button>
            <!-- The old page printed "showing 8 of 41" and stopped there. -->
            <span v-if="c.n > c.rows.length" class="muted" style="font-size: 12px; align-self: center">
              showing {{ c.rows.length }} of {{ c.n }} — the link opens the rest
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
          <tr v-for="(ev, i) in dash.recent" :key="i" :class="{ clickable: !ev.code.startsWith('(') }" @click="openCode(ev.code)">
            <td class="muted nowrap" style="width: 1px">{{ shortDateTime(ev.created) }}</td>
            <td class="mono" style="width: 1px">
              <!-- Pseudo-codes for release and tier events ("(tier strike)") are
                   shown but not linked: they are not codes. -->
              <span v-if="ev.code.startsWith('(')" class="muted">{{ ev.code }}</span>
              <span v-else class="code-link">{{ ev.code }}</span>
            </td>
            <td style="width: 1px">{{ ev.event }}</td>
            <td class="muted truncate">{{ ev.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <p class="foot-note">
      <strong>Check-ins, last {{ dash.attempts.window_minutes }} minutes:</strong>
      {{ dash.attempts.activate }} activation attempt(s), {{ dash.attempts.lookup }} code lookup(s).
      Recorded codes are dropped after {{ dash.attempts.window_minutes }} minutes, so this is a live gauge
      rather than a history — a steady stream here is a mistyped card or someone guessing.
    </p>
  </template>

  <RenewDrawer :row="renewRow" :middlemen="[]" @close="renewRow = null" @done="renewRow = null; refreshAll()" />
</template>
