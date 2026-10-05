<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { call, fetchRelease, type FetchArtifact } from '../api'
import { toast } from '../toast'

// ── Releases page: publish by pulling a GitHub Release ──
//
// This page used to upload four ~15-30 MB binaries through the browser. That
// made publishing depend on the operator's laptop, its upload bandwidth and its
// browser, and it needed a dedicated upload service on the hub because
// PocketBase rejects bodies above a few MB.
//
// Now the hub fetches the artifacts itself, straight from the GitHub Release
// for a version. Nothing large is sent TO the hub at all, so the upload path —
// and its whole class of transfer problems — is gone.
//
// The flow is deliberately three explicit steps, in this order:
//
//   1. Publish  — hub pulls from GitHub, verifies, writes update_config
//   2. Offer    — choose what proportion of clients is told about it
//
// Fetching and offering are separate steps because they fail differently: a
// GitHub fetch can fail for reasons that have nothing to do with the release
// itself, and offering an un-fetched (or unsigned) version sends the fleet to a
// 404 or an unverifiable download. Doing the fetch first makes that mistake
// impossible to reach.
//
// There is no rollout percentage any more — see setOffering().

interface Platform {
  url: string
  sha256: string
  /** Whether a signature exists for this platform's artifact. Without one the
   *  updater refuses to install, so an unsigned platform reaches nobody. */
  signed: boolean
}
interface Release {
  version: string
  active: boolean
  platforms: Record<string, Platform>
  /**
   * Human-facing installers the hub has staged for this version, keyed the same
   * way the fetch result is (`zip_macos_amd64`, `zip_macos_arm64`). Optional
   * because a hub that predates this field, or a release published before the
   * macOS packaging step existed, simply has none — and that must render as
   * "not published" rather than as an error.
   *
   * `releases.get` may not return this yet; the page prefers the live fetch
   * result and falls back to this, so the table is populated either way.
   */
  installers?: Record<string, FetchArtifact | undefined>
}

// The filenames the client's updater looks for. If these do not match, updates
// silently never apply, so the page names them explicitly.
const EXPECTED: { key: string; filename: string; label: string }[] = [
  { key: 'linux', filename: 'locus-linux-amd64', label: 'Linux (amd64)' },
  { key: 'windows', filename: 'locus-windows-amd64.exe', label: 'Windows (amd64)' },
  { key: 'macos_intel', filename: 'locus-darwin-amd64', label: 'macOS (Intel)' },
  { key: 'macos_arm', filename: 'locus-darwin-arm64', label: 'macOS (Apple Silicon)' },
]

const release = ref<Release | null>(null)
const version = ref('')
const loading = ref(true)
const savingRollout = ref(false)
const fetching = ref(false)

// Result of the last fetch, so the operator can see exactly what was verified
// (filename, size, hash, and the format actually detected) rather than a
// success toast they have to take on faith.
const fetched = ref<Record<string, FetchArtifact> | null>(null)
const fetchError = ref('')
const fetchElapsed = ref<number | null>(null)

// One-shot trigger link state.
const link = ref('')
const linkBusy = ref(false)

function fmtBytes(n: number): string {
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

async function loadRelease() {
  loading.value = true
  const res = await call<{ release: Release }>('releases.get')
  if (res.ok && res.data) {
    release.value = res.data.release
    // Do not clobber a version the operator has already typed: on first load
    // the current version is a useful default, but after they enter a new one
    // (e.g. 2.2.1 while the hub is on 2.2.0) reloading must not reset the field.
    if (!version.value.trim()) version.value = res.data.release.version
  } else {
    toast.err(res.message || res.transportError || 'Could not load release state')
  }
  loading.value = false
}

// ── Step 1: pull the release from GitHub ──
//
// Calls the fetch service THROUGH Caddy, which resolves every asset by name
// from the GitHub release manifest, streams each one to disk, verifies the
// format, and reports the hashes it computed. A release missing a platform is
// refused there — no partial publish is possible from this button.
async function publishFromGithub() {
  const v = version.value.trim().replace(/^v/, '')
  if (!v) {
    toast.err('Enter the version to publish (e.g. 2.2.1)')
    return
  }
  if (!/^[0-9]+\.[0-9]+\.[0-9]+/.test(v)) {
    toast.err('Version must look like 1.2.3')
    return
  }
  fetching.value = true
  fetched.value = null
  fetchError.value = ''
  fetchElapsed.value = null

  try {
    const res = await fetchRelease(v)
    if (!res.ok || !res.data) {
      fetchError.value = res.message || res.transportError || 'Fetch failed'
      toast.err(fetchError.value)
      return
    }

    fetched.value = res.data.artifacts
    fetchElapsed.value = res.data.elapsed ?? null

    // ── Step 2 (DB half): write update_config from what was verified ──
    //
    // The hashes come from the files the hub actually hashed on disk, not from
    // anything the browser claims. Rollout is NOT changed here — publishing and
    // offering are separate decisions (releases.set owns `active`).
    const pub = await call('releases.publish', {
      version: v,
      artifacts: res.data.artifacts,
      base_url: window.location.origin,
    })
    if (!pub.ok) {
      fetchError.value = pub.message || pub.transportError || 'Could not record the release'
      toast.err(fetchError.value)
      return
    }

    toast.ok(`Release ${v} published — all four platforms verified`)
    await loadRelease()
  } finally {
    fetching.value = false
  }
}

// ── One-shot trigger link ──
//
// The point of the link is that a fetch can be triggered from somewhere that
// does not hold the admin token — a phone, a CI job. Every link is fresh,
// expires, and can be used exactly once; the hub refuses a replay. That is what
// makes it safe to paste into a chat or a bookmark.
async function makeLink() {
  const v = version.value.trim().replace(/^v/, '')
  if (!v) {
    toast.err('Enter a version first — the link is bound to one version')
    return
  }
  linkBusy.value = true
  try {
    const res = await call<{ link: string; expires_in: number }>('releases.fetchLink', { version: v })
    if (res.ok && res.data?.link) {
      link.value = res.data.link
      toast.ok('New one-shot link minted')
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
    toast.ok('Link copied — single use, and it expires')
  } catch {
    toast.err('Could not copy — select the text manually')
  }
}

// ── Offering on/off ──
//
// There is no rollout percentage. Publishing a release IS offering it; this
// flips the single `active` flag, which is the only remaining lever.
//
// Why the percentage went: it existed to limit how many devices saw a bad
// build, because the client has no rollback beyond the installer's own
// atomicity. In practice it mostly caused misdiagnosis — on a small fleet a low
// percentage means your own test device is probably outside the bucket, so a
// working updater looks broken.
async function setOffering(active: boolean) {
  const v = version.value.trim().replace(/^v/, '')
  if (!v) {
    toast.err('Enter a version number')
    return
  }

  // Never advertise a version we cannot serve.
  //
  // The hub sends whatever URLs sit in update_config, independently of what is
  // on disk and independently of whether the artifacts are signed. Activating a
  // release whose URLs are empty, point at another version, or carry no
  // signature tells every client to fetch something that will 404 or fail
  // verification — and a client that cannot install cannot update, so the
  // release silently fails for the whole fleet with no error anywhere but the
  // client log. The hook enforces this too; checking here gives a better
  // message than a 400.
  if (active) {
    const missing = EXPECTED.filter((e) => {
      const platform = release.value?.platforms[e.key]
      return !platform?.url || !platform?.signed
    }).map((e) => e.label)
    if (missing.length) {
      toast.err(
        `Refusing to offer ${v} — not published, or unsigned, for: ${missing.join(', ')}. ` +
          `Publish the release first, or clients will be sent to a 404 or handed ` +
          `an update the updater cannot verify.`
      )
      return
    }
  }

  savingRollout.value = true
  try {
    const res = await call('releases.set', { version: v, active })
    if (res.ok) {
      toast.ok(
        active
          ? `Release ${v} is now offered to all clients`
          : `Stopped offering ${v}`
      )
      await loadRelease()
    } else {
      toast.err(res.message || res.transportError || 'Could not update the release')
    }
  } finally {
    savingRollout.value = false
  }
}

// What the hub is currently advertising, for the "on the hub" summary.
const publishedCount = computed(
  () => EXPECTED.filter((e) => release.value?.platforms[e.key]?.url).length,
)
const allPublished = computed(() => publishedCount.value === EXPECTED.length)

// ── The macOS human download ──
//
// Mirrors `installer_zip_names()` in fetch-release.py: two architectures, named
// `installer-Locus_<version>_<arch>.zip`. This is a CONTRACT with the hub and
// with CI, and it is the one place the console names a non-updater asset — if
// the template changes there, this must change with it. Deliberately NOT added
// to EXPECTED above: that array drives the "all four platforms published"
// warning and the offer guard, and a missing human download must never be able
// to block offering an update.
//
// The read-out prefers the last FETCH RESULT (which carries the hub's own
// verified size/hash/format) and falls back to the published release row, so a
// page reload still shows what is on the hub rather than an empty table.
const MAC_ARCHES = [
  { key: 'zip_macos_amd64', arch: 'amd64', label: 'macOS (Intel)' },
  { key: 'zip_macos_arm64', arch: 'arm64', label: 'macOS (Apple Silicon)' },
] as const

const macDownloads = computed(() => {
  const v = (release.value?.version || version.value).trim().replace(/^v/, '')
  if (!v) return []
  return MAC_ARCHES.map((a) => ({
    key: a.key,
    label: a.label,
    filename: `installer-Locus_${v}_${a.arch}.zip`,
    art: fetched.value?.[a.key] ?? release.value?.installers?.[a.key] ?? undefined,
  }))
})

onMounted(() => {
  loadRelease()
})
</script>

<template>
  <h1 class="page-title">Releases</h1>
  <p class="page-sub">
    Publish a client update by pulling its build straight from GitHub. Nothing is
    uploaded from this browser.
  </p>

  <!-- ── What the hub currently advertises ── -->
  <div class="card">
    <h2>On the hub now</h2>
    <p v-if="loading" class="muted">Loading…</p>
    <template v-else-if="release">
      <div class="kv">
        <div><span class="muted">Version</span><strong>{{ release.version || '—' }}</strong></div>
        <div><span class="muted">Offered</span><strong>{{ release.active ? 'yes' : 'no' }}</strong></div>
      </div>

      <table class="grid">
        <thead>
          <tr><th>Platform</th><th>Artifact</th><th>SHA-256</th></tr>
        </thead>
        <tbody>
          <tr v-for="e in EXPECTED" :key="e.key">
            <td>{{ e.label }}</td>
            <td class="mono">{{ e.filename }}</td>
            <td class="mono">
              {{ release.platforms[e.key]?.sha256 ? release.platforms[e.key].sha256.slice(0, 16) + '…' : '— not published —' }}
            </td>
          </tr>
        </tbody>
      </table>

      <div v-if="!allPublished" class="msg warn">
        The version above is advertised with {{ publishedCount }} of 4 platforms.
        Clients on a missing platform will be offered the update and have nothing
        to download. Publish the release to fill them in.
      </div>
    </template>
  </div>

  <!-- ── Step 1: publish from GitHub ── -->
  <div class="card">
    <h2>Publish from GitHub</h2>
    <p class="muted">
      The hub downloads the four raw binaries, their signatures and
      <code>manifest.json</code> from the GitHub Release tagged
      <code>v&lt;version&gt;</code>, verifies every file, and points the hub at
      them. CI must have finished and the tag must be pushed.
    </p>
    <p class="muted">
      The macOS <code>.zip</code> download is fetched too, but it is
      <strong>not</strong> an update payload — it is the compressed, signed app a
      person downloads. It appears below, never in the platform table, because
      the updater replaces the app binary directly and cannot unpack a zip.
    </p>

    <div class="row">
      <label class="field">
        <span>Version</span>
        <input v-model="version" type="text" placeholder="2.2.1" />
      </label>
      <div class="shrink">
        <button class="primary" :disabled="fetching" @click="publishFromGithub">
          {{ fetching ? 'Fetching from GitHub…' : 'Fetch & publish' }}
        </button>
      </div>
    </div>

    <p v-if="fetching" class="muted">
      Downloading and verifying every artifact. This takes a few seconds per
      platform; do not close the page.
    </p>

    <div v-if="fetchError" class="msg err">{{ fetchError }}</div>

    <table v-if="fetched" class="grid">
      <thead>
        <tr><th>Platform</th><th>File</th><th>Size</th><th>Detected</th><th>SHA-256</th></tr>
      </thead>
      <tbody>
        <tr v-for="e in EXPECTED" :key="e.key">
          <td>{{ e.label }}</td>
          <td class="mono">{{ fetched[e.key]?.filename || '—' }}</td>
          <td>{{ fetched[e.key] ? fmtBytes(fetched[e.key].bytes) : '—' }}</td>
          <td>{{ fetched[e.key]?.format || '—' }}</td>
          <td class="mono">{{ fetched[e.key]?.sha256.slice(0, 16) }}…</td>
        </tr>
      </tbody>
    </table>
    <p v-if="fetched && fetchElapsed !== null" class="muted">
      Verified and recorded in {{ fetchElapsed }}s. Rollout is unchanged — choose
      it below.
    </p>

    <!-- ── The macOS human download ──
         Deliberately a SEPARATE card from the platform table above, and outside
         it. Those rows are what a client fetches and executes to replace itself;
         this is a file a person downloads and runs. Presenting them in one table
         is how the two would eventually be confused, and confusing them is the
         mistake that stopped every Windows client updating for three releases.

         The read-out is the point of showing it at all: the operator should be
         able to hand a student a link and a hash, and see that the hub actually
         verified the archive rather than trusting a "published" toast. -->
    <div v-if="macDownloads.length" class="mac-downloads">
      <h3>macOS download (for people, not clients)</h3>
      <p class="muted">
        The compressed, ad-hoc-signed <code>Locus.app</code> for each
        architecture. A student who cannot open the <code>.dmg</code> should be
        given this instead — the signature moves the Gatekeeper failure into
        System Settings → Privacy &amp; Security → <strong>Open Anyway</strong>,
        so no Terminal is needed.
      </p>
      <table class="grid">
        <thead>
          <tr><th>Architecture</th><th>File</th><th>Size</th><th>Detected</th><th>SHA-256</th></tr>
        </thead>
        <tbody>
          <tr v-for="d in macDownloads" :key="d.key">
            <td>{{ d.label }}</td>
            <td class="mono">
              <!-- Served by Caddy from /updates/<version>/, the same
                   directory the platform artifacts live in. It is fetched by
                   the hub from the GitHub Release, so no upload is involved. -->
              <a :href="`/updates/${release?.version || version}/${d.filename}`">{{ d.filename }}</a>
            </td>
            <td>{{ d.art ? fmtBytes(d.art.bytes) : '—' }}</td>
            <td>{{ d.art?.format || '—' }}</td>
            <td class="mono">
              {{ d.art?.sha256 ? d.art.sha256.slice(0, 16) + '…' : '— not published —' }}
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="macDownloads.some((d) => !d.art)" class="msg warn">
        One or more macOS downloads are absent from this release. That does not
        block the update — a missing human download never does — but students on
        macOS will have only the <code>.dmg</code>, which needs Terminal to get
        past Gatekeeper.
      </div>
    </div>
  </div>

  <!-- ── One-shot trigger links ── -->
  <div class="card">
    <h2>One-shot trigger link</h2>
    <p class="muted">
      A link that fetches this same version without the admin token — useful from
      a phone or a CI job. Each link is <strong>single-use</strong> and expires;
      a used or expired link is refused. Mint a new one every time.
    </p>
    <div class="row">
      <div class="shrink">
        <button :disabled="linkBusy" @click="makeLink">
          {{ linkBusy ? 'Minting…' : 'New link' }}
        </button>
      </div>
      <div v-if="link" class="shrink">
        <button @click="copyLink">Copy</button>
      </div>
    </div>
    <p v-if="link" class="mono breakall">{{ link }}</p>
  </div>

  <!-- ── Step 2: offer it, or stop offering it ── -->
  <div class="card">
    <h2>Offer this update</h2>
    <p class="muted">
      While this is on, every client is told the update exists on its next
      check. There is no staged rollout — what you publish is what clients are
      offered, so publish only once you are ready for everyone.
    </p>

    <div class="actions">
      <button class="primary" :disabled="savingRollout" @click="setOffering(true)">
        {{ savingRollout ? 'Saving…' : 'Offer to all clients' }}
      </button>
      <button class="danger" :disabled="savingRollout" @click="setOffering(false)">
        Stop offering
      </button>
    </div>

    <div v-if="!allPublished" class="msg warn">
      Not every platform is published and signed, so this release cannot be
      offered yet. Publish it first.
    </div>

    <p class="muted" style="margin-bottom: 0; margin-top: 12px; font-size: 12px">
      <strong>There is no automatic downgrade.</strong> Stopping only stops
      <em>offering</em> the update — clients that already installed it stay on
      it. If a build is bad, the fix is to publish a higher version, so test a
      release on a real machine before turning this on.
    </p>
  </div>
</template>
