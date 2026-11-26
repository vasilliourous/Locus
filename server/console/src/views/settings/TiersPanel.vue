<script setup lang="ts">
// Tiers — the connection settings a client receives when it activates.
//
// SAME DATA AND SAME GUARDRAILS as the old Tiers tab, which was not useless (it
// edits live fleet settings and carries the `hub_alias` escape hatch) but was
// laid out so nothing could be compared:
//
//   * one full-width panel per tier, stacked, each with its own Save button —
//     so comparing two tiers' ports meant scrolling;
//   * ~350 words of prose between the checkboxes and the Save, every time;
//   * no indication of which tier is actually being served to anybody.
//
// Now: one table you can read down, an expander per row for the editable fields,
// and the prose behind a disclosure. The WARNINGS are the exception — they stay
// inline, because they describe a setting whose effect the operator cannot
// otherwise observe (`udp_relay` without `uot_port` is a switch that does
// nothing; that fact cost a fleet-wide UDP outage, FIXES.md 29).

import { computed, onMounted, ref } from 'vue'
import { call } from '../../api'
import { toast } from '../../toast'
import StateBlock from '../../components/StateBlock.vue'

interface TierRow {
  id: string
  tier: string
  active: boolean
  udp_relay: boolean
  server: string
  server_port: number
  method: string
  /** The UDP-over-TCP endpoint port, or 0 when the tier has none. */
  uot_port: number
  /** A hub base URL to offer this fleet instead of the compiled one, or ''. */
  hub_alias: string
}

const rows = ref<TierRow[]>([])
const loading = ref(true)
const failed = ref('')
const saving = ref('')
/** The row currently expanded for editing, by tier name. One at a time. */
const editing = ref('')

async function load() {
  loading.value = true
  failed.value = ''
  const res = await call<{ tiers: TierRow[] }>('tiers.list')
  loading.value = false
  if (!res.ok || !res.data) {
    failed.value = res.message || res.transportError || 'Could not load the tiers.'
    return
  }
  rows.value = res.data.tiers || []
}

onMounted(load)

async function save(row: TierRow) {
  saving.value = row.tier
  try {
    const res = await call('tiers.update', {
      tier: row.tier,
      server: row.server,
      server_port: row.server_port,
      method: row.method,
      active: row.active,
      udp_relay: row.udp_relay,
      uot_port: row.uot_port,
      hub_alias: row.hub_alias,
    })
    if (!res.ok) {
      toast.err(res.message || res.transportError || 'Save failed')
      return
    }
    toast.ok(`${row.tier} saved — clients pick this up on their next check-in`)
    editing.value = ''
    await load()
  } finally {
    saving.value = ''
  }
}

/**
 * The two halves of the UDP switch disagreeing.
 *
 * Both states are harmless and both are misleading, which is exactly why they are
 * shown rather than prevented: the operator may be mid-edit. The cost of not
 * showing them is the failure that made UDP dead fleet-wide while every setting
 * looked configured.
 */
function udpProblem(row: TierRow): string | null {
  if (row.udp_relay && !row.uot_port) {
    return 'UDP relay is on but no UoT port is set — the client only builds the UDP-over-TCP outbound when both are present, so UDP stays raw.'
  }
  if (!row.udp_relay && row.uot_port) {
    return `A UoT port (${row.uot_port}) is set but UDP relay is off — the endpoint exists and no client is told to use it.`
  }
  return null
}

const served = computed(() => rows.value.filter((r) => r.active && !tierMisconfigured(r)))
function tierMisconfigured(row: TierRow): boolean {
  // A tier missing its server or port cannot produce a working config; a client
  // that activates against it connects and reaches nothing.
  return !row.server || !row.server_port
}
</script>

<template>
  <StateBlock v-if="loading" state="loading" :rows="3" />
  <StateBlock
    v-else-if="failed"
    state="error"
    :message="failed"
    hint="Tier settings live in the hub's tier_configs collection."
    @retry="load"
  />

  <template v-else>
    <div class="msg warn">
      <strong>The tier password is deliberately not editable here.</strong>
      Changing it breaks every client already using that tier, with no way for an
      existing install to recover on its own. It stays a documented operation
      (OPS.md).
    </div>

    <div class="table-wrap">
      <table class="data">
        <thead>
          <tr>
            <th>Tier</th>
            <th>Server</th>
            <th class="num">Port</th>
            <th>Method</th>
            <th>UDP</th>
            <th>Status</th>
            <th style="width: 90px"></th>
          </tr>
        </thead>
        <tbody>
          <template v-for="row in rows" :key="row.id">
            <tr :class="{ dim: !row.active }">
              <td>
                <strong style="text-transform: capitalize">{{ row.tier }}</strong>
                <div v-if="row.hub_alias" class="muted" style="font-size: 11px">
                  offers {{ row.hub_alias }}
                </div>
              </td>
              <td class="mono">{{ row.server || '— not set —' }}</td>
              <td class="num mono">{{ row.server_port || '—' }}</td>
              <td class="mono">{{ row.method || '—' }}</td>
              <td>
                <span v-if="!row.udp_relay && !row.uot_port" class="muted">off</span>
                <span v-else-if="row.udp_relay && row.uot_port" class="badge bound">
                  on · :{{ row.uot_port }}
                </span>
                <span v-else class="badge issued">mismatched</span>
              </td>
              <td>
                <span v-if="tierMisconfigured(row)" class="badge suspended">unservable</span>
                <span v-else-if="row.active" class="badge available">served</span>
                <span v-else class="muted" style="font-size: 12px">inactive</span>
              </td>
              <td>
                <button class="row-action" @click="editing = editing === row.tier ? '' : row.tier">
                  {{ editing === row.tier ? 'Close' : 'Edit' }}
                </button>
              </td>
            </tr>

            <!-- The editing row. Only the fields for THIS tier, so a change can
                 never be applied to the wrong row (the old page's Save buttons
                 were identical and stacked). -->
            <tr v-if="editing === row.tier" :key="`${row.id}-edit`">
              <td colspan="7" style="height: auto; padding: 16px; background: var(--panel-2)">
                <div class="form-row">
                  <label class="lbl">Server hostname</label>
                  <div class="ctl">
                    <input v-model="row.server" spellcheck="false" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">Port</label>
                  <div class="ctl">
                    <input v-model.number="row.server_port" type="number" min="1" max="65535" style="max-width: 140px" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">Encryption method</label>
                  <div class="ctl">
                    <input v-model="row.method" spellcheck="false" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">
                    Served
                    <small>inactive tiers are not handed to clients</small>
                  </label>
                  <div class="ctl">
                    <input v-model="row.active" type="checkbox" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">
                    UDP relay
                    <small>requires a UoT port below</small>
                  </label>
                  <div class="ctl">
                    <input v-model="row.udp_relay" type="checkbox" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">
                    UoT port
                    <small>0 = none</small>
                  </label>
                  <div class="ctl">
                    <input v-model.number="row.uot_port" type="number" min="0" max="65535" style="max-width: 140px" />
                  </div>
                </div>
                <div class="form-row">
                  <label class="lbl">
                    Hub address to offer
                    <small>blank = use the compiled address</small>
                  </label>
                  <div class="ctl">
                    <input v-model="row.hub_alias" placeholder="https://hub.example.org" spellcheck="false" />
                  </div>
                </div>

                <p v-if="udpProblem(row)" class="msg warn" style="margin-top: 12px; font-size: 12px">
                  {{ udpProblem(row) }}
                </p>

                <div class="actions" style="margin-top: 12px">
                  <button class="primary" :disabled="saving === row.tier" @click="save(row)">
                    {{ saving === row.tier ? 'Saving…' : `Save ${row.tier}` }}
                  </button>
                  <button :disabled="saving === row.tier" @click="editing = ''; load()">Cancel</button>
                </div>
              </td>
            </tr>

            <tr v-if="udpProblem(row) && editing !== row.tier" :key="`${row.id}-warn`">
              <td colspan="7" style="height: auto; padding: 6px 12px 10px">
                <span class="msg warn" style="display: inline-block; margin: 0; font-size: 12px">
                  {{ udpProblem(row) }}
                </span>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>

    <!-- The reference prose. It is institutional memory — the endpoint table, the
         warning about pointing UoT at a shadowsocks-rust port, the Caddy alias
         prerequisite — so it is kept in full, behind a disclosure rather than
         between the operator and the Save button. -->
    <details class="help">
      <summary>How these settings reach a client, and how they can break it</summary>
      <div class="body">
        <p>
          <strong>UoT port</strong> is the UDP-over-TCP endpoint the client tunnels
          UDP through. Both switches are required: the client builds the UoT
          outbound only when <strong>UDP relay</strong> is on <em>and</em> a
          <code>uot_port</code> is set.
        </p>
        <p>
          Use it only for a <strong>paid tier</strong> — the <code>strike</code>
          row (shown as <strong>Full</strong>) or the <code>budget</code> row — and
          only when a sing-box UoT listener is running for it. Install it with
          <code>enable-uot.sh</code>; the live endpoint table is
          <code>docs/business/04-tiers.md</code> §4.1. Do <strong>not</strong>
          point it at the ordinary shadowsocks-rust ports (8443/8445/8448): they do
          not implement sing-box's UDP-over-TCP and will refuse the connections.
        </p>
        <p>
          Confirm the listener is up before setting the port:
          <code>systemctl status sing-box-uot-strike</code> and
          <code>ss -lntup | grep -E '8446|8447|8449'</code> — each must listen on
          <em>UDP</em> as well as TCP. A port advertised with nothing listening
          breaks UDP (games, voice) while TCP and DNS keep working, so the failure
          looks like “some things work, some don't” rather than an outage.
        </p>
        <p>
          <strong>Hub address to offer</strong> moves this fleet off a hostname a
          network has blocked — a school filter that classifies the default hub
          address as uncategorised. It reaches clients on their next check-in and
          takes effect on their next launch; the built-in address keeps working for
          everyone else, so nothing breaks while this is blank. It must be an
          absolute <code>https://</code> URL with no path. The server must actually
          answer on that name: add a Caddy alias block with
          <code>HUB_ALIAS_DOMAIN</code> on the box first.
        </p>
        <p>
          Clearing the field withdraws the offer. Leaving it unchanged does not,
          because a heartbeat that omits the key never overrides a stored value.
        </p>
      </div>
    </details>

    <p class="foot-note">
      {{ served.length }} of {{ rows.length }} tier(s) are being served. A change
      here reaches a connected client within about five minutes; a client that is
      offline picks it up whenever it next checks in.
    </p>
  </template>
</template>
