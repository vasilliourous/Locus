import { execFileSync, execSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import zlib from 'node:zlib'

import AdmZip from 'adm-zip'
import { glob } from 'glob'
import { HttpsProxyAgent } from 'https-proxy-agent'
import { extract } from 'tar'

import { resolveServiceRelease } from './service-release.mjs'
import { log_debug, log_error, log_info, log_success, log_warn } from './utils.mjs'


/** Prepares platform resources, caching versions and unchanged files unless `--force` is used. */

const cwd = process.cwd()

// ── The pinned build inputs ────────────────────────────────────────────────
//
// `server/data/sidecar.lock.json` records, per sidecar and per geo database, the
// sha256 of the bytes that were actually used to ship a release, plus the engine
// version read from the binaries themselves.
//
// WHY THE PIN OVERRIDES `releases/latest`. Before this, the engine version was
// resolved by fetching `version.txt` from GitHub at build time, so two builds of
// the same commit could carry different engines — which makes a defect
// impossible to bisect by commit. With the lock present, the version comes from
// the lock and the network is only a way to *fetch* it, not to *choose* it.
//
// WHAT IS ENFORCED AND WHAT IS NOT. After a download the sha256 is checked
// against the lock and a mismatch aborts the build. That closes the
// silent-wrong-binary path. It does NOT make the build offline: the archive is
// still fetched from GitHub. Vendoring the archives is recorded in
// docs/reference/STILL-OPEN.md as not done.
//
// A missing lock is not fatal — a contributor without one should still be able
// to build — but it is loud, because it means the build is unpinned.
const LOCK_PATH = path.join(cwd, '..', 'server', 'data', 'sidecar.lock.json')
let SIDECAR_LOCK = null
try {
  SIDECAR_LOCK = JSON.parse(fs.readFileSync(LOCK_PATH, 'utf-8'))
} catch (err) {
  log_warn(
    `no sidecar lock at ${path.relative(cwd, LOCK_PATH)} — build inputs are UNPINNED (${err.code || err.message})`,
  )
}

/// The locked version for an engine, or undefined when there is no lock.
function lockedEngineVersion(key) {
  return SIDECAR_LOCK?.engine_versions?.[key] || undefined
}

/// The locked sha256 for a sidecar target file, or undefined.
function lockedSidecarHash(targetFile) {
  if (!SIDECAR_LOCK?.sidecars) return undefined
  for (const byTriple of Object.values(SIDECAR_LOCK.sidecars)) {
    for (const entry of Object.values(byTriple)) {
      if (entry.file === targetFile) return entry.sha256
    }
  }
  return undefined
}

/// Whether this sidecar is vendored in the repository rather than downloaded.
///
/// A vendored sidecar is one whose upstream URL cannot be relied on — today that
/// is only the mihomo alpha, whose rolling prerelease URL stops existing when a
/// new alpha lands. It is committed to the repository, so the fetch is skipped
/// entirely and the file is verified against its pin like any other input.
function lockedSidecarVendored(targetFile) {
  if (!SIDECAR_LOCK?.sidecars) return false
  for (const byTriple of Object.values(SIDECAR_LOCK.sidecars)) {
    for (const entry of Object.values(byTriple)) {
      if (entry.file === targetFile) return entry.vendored === true
    }
  }
  return false
}

/// The locked sha256 for a geo database filename, or undefined.
function lockedResourceHash(file) {
  return SIDECAR_LOCK?.resources?.[file]?.sha256
}

/// Hash a file and compare to a pin. Throws on mismatch, naming both hashes.
async function assertPinned(filePath, expected, label) {
  if (!expected) return
  const actual = await calculateFileHash(filePath)
  if (actual !== expected) {
    throw new Error(
      `${label} does not match its pin: got ${String(actual).slice(0, 16)}, ` +
        `lock pins ${expected.slice(0, 16)}. The build would ship an engine ` +
        `nobody reviewed. If this change is intended, record it with ` +
        `\`python3 server/scripts/sidecar-lock.py --write\` and commit the diff.`,
    )
  }
  log_debug(`${label} matches its pin`)
}
const TEMP_DIR = path.join(cwd, 'node_modules/.verge')
const FORCE = process.argv.includes('--force') || process.argv.includes('-f')
const VERSION_CACHE_FILE = path.join(TEMP_DIR, '.version_cache.json')
const HASH_CACHE_FILE = path.join(TEMP_DIR, '.hash_cache.json')

const PLATFORM_MAP = {
  'x86_64-pc-windows-msvc': 'win32',
  'i686-pc-windows-msvc': 'win32',
  'aarch64-pc-windows-msvc': 'win32',
  'x86_64-apple-darwin': 'darwin',
  'aarch64-apple-darwin': 'darwin',
  'x86_64-unknown-linux-gnu': 'linux',
  'i686-unknown-linux-gnu': 'linux',
  'aarch64-unknown-linux-gnu': 'linux',
  'armv7-unknown-linux-gnueabihf': 'linux',
  'riscv64gc-unknown-linux-gnu': 'linux',
  'loongarch64-unknown-linux-gnu': 'linux',
}
const ARCH_MAP = {
  'x86_64-pc-windows-msvc': 'x64',
  'i686-pc-windows-msvc': 'ia32',
  'aarch64-pc-windows-msvc': 'arm64',
  'x86_64-apple-darwin': 'x64',
  'aarch64-apple-darwin': 'arm64',
  'x86_64-unknown-linux-gnu': 'x64',
  'i686-unknown-linux-gnu': 'ia32',
  'aarch64-unknown-linux-gnu': 'arm64',
  'armv7-unknown-linux-gnueabihf': 'arm',
  'riscv64gc-unknown-linux-gnu': 'riscv64',
  'loongarch64-unknown-linux-gnu': 'loong64',
}

const arg1 = process.argv.slice(2)[0]
const arg2 = process.argv.slice(2)[1]
const target = arg1 === '--force' || arg1 === '-f' ? arg2 : arg1
if (process.env.CI && !target) {
  throw new Error('prebuild requires an explicit target triple in CI')
}
const { platform, arch } = target
  ? { platform: PLATFORM_MAP[target], arch: ARCH_MAP[target] }
  : process

const SIDECAR_HOST = target
  ? target
  : execSync('rustc -vV')
      .toString()
      .match(/(?<=host: ).+(?=\s*)/g)[0]

const RESOURCES_DIR = path.join(cwd, 'src-tauri', 'resources')
const SIDECAR_DIR = path.join(cwd, 'src-tauri', 'sidecar')
// Linux service binaries are bundled as externalBin sidecars (see tauri.linux.conf.json)
const SERVICE_DIR = platform === 'linux' ? SIDECAR_DIR : RESOURCES_DIR

// Version cache
async function loadVersionCache() {
  try {
    if (fs.existsSync(VERSION_CACHE_FILE)) {
      const data = await fsp.readFile(VERSION_CACHE_FILE, 'utf-8')
      return JSON.parse(data)
    }
  } catch (err) {
    log_debug('Failed to load version cache:', err.message)
  }
  return {}
}
async function saveVersionCache(cache) {
  try {
    await fsp.mkdir(TEMP_DIR, { recursive: true })
    await fsp.writeFile(VERSION_CACHE_FILE, JSON.stringify(cache, null, 2))
    log_debug('Version cache saved')
  } catch (err) {
    log_debug('Failed to save version cache:', err.message)
  }
}
async function getCachedVersion(key) {
  const cache = await loadVersionCache()
  const cached = cache[key]
  if (cached && Date.now() - cached.timestamp < 3600000) {
    log_info(`Using cached version for ${key}: ${cached.version}`)
    return cached.version
  }
  return null
}
async function setCachedVersion(key, version) {
  const cache = await loadVersionCache()
  cache[key] = { version, timestamp: Date.now() }
  await saveVersionCache(cache)
}

// File hash cache
async function calculateFileHash(filePath) {
  try {
    const fileBuffer = await fsp.readFile(filePath)
    const hashSum = createHash('sha256')
    hashSum.update(fileBuffer)
    return hashSum.digest('hex')
  } catch (ignoreErr) {
    return null
  }
}
async function loadHashCache() {
  try {
    if (fs.existsSync(HASH_CACHE_FILE)) {
      const data = await fsp.readFile(HASH_CACHE_FILE, 'utf-8')
      return JSON.parse(data)
    }
  } catch (err) {
    log_debug('Failed to load hash cache:', err.message)
  }
  return {}
}
async function saveHashCache(cache) {
  try {
    await fsp.mkdir(TEMP_DIR, { recursive: true })
    await fsp.writeFile(HASH_CACHE_FILE, JSON.stringify(cache, null, 2))
    log_debug('Hash cache saved')
  } catch (err) {
    log_debug('Failed to save hash cache:', err.message)
  }
}
async function hasFileChanged(filePath, targetPath) {
  if (FORCE) return true
  if (!fs.existsSync(targetPath)) return true
  const hashCache = await loadHashCache()
  const sourceHash = await calculateFileHash(filePath)
  const targetHash = await calculateFileHash(targetPath)
  if (!sourceHash || !targetHash) return true
  const cacheKey = targetPath
  const cachedHash = hashCache[cacheKey]
  if (cachedHash === sourceHash && sourceHash === targetHash) {
    return false
  }
  return true
}
async function updateHashCache(targetPath) {
  const hashCache = await loadHashCache()
  const hash = await calculateFileHash(targetPath)
  if (hash) {
    hashCache[targetPath] = hash
    await saveHashCache(hashCache)
  }
}

// Mihomo release maps
const META_ALPHA_VERSION_URL =
  'https://github.com/MetaCubeX/mihomo/releases/download/Prerelease-Alpha/version.txt'
const META_ALPHA_URL_PREFIX = `https://github.com/MetaCubeX/mihomo/releases/download/Prerelease-Alpha`
let META_ALPHA_VERSION

const META_VERSION_URL =
  'https://github.com/MetaCubeX/mihomo/releases/latest/download/version.txt'
const META_URL_PREFIX = `https://github.com/MetaCubeX/mihomo/releases/download`
let META_VERSION

const META_ALPHA_MAP = {
  'win32-x64': 'mihomo-windows-amd64-v2',
  'win32-ia32': 'mihomo-windows-386',
  'win32-arm64': 'mihomo-windows-arm64',
  'darwin-x64': 'mihomo-darwin-amd64-v1-go122',
  'darwin-arm64': 'mihomo-darwin-arm64-go122',
  'linux-x64': 'mihomo-linux-amd64-v2',
  'linux-ia32': 'mihomo-linux-386',
  'linux-arm64': 'mihomo-linux-arm64',
  'linux-arm': 'mihomo-linux-armv7',
  'linux-riscv64': 'mihomo-linux-riscv64',
  'linux-loong64': 'mihomo-linux-loong64',
}

const META_MAP = {
  'win32-x64': 'mihomo-windows-amd64-v2',
  'win32-ia32': 'mihomo-windows-386',
  'win32-arm64': 'mihomo-windows-arm64',
  'darwin-x64': 'mihomo-darwin-amd64-v2-go122',
  'darwin-arm64': 'mihomo-darwin-arm64-go122',
  'linux-x64': 'mihomo-linux-amd64-v2',
  'linux-ia32': 'mihomo-linux-386',
  'linux-arm64': 'mihomo-linux-arm64',
  'linux-arm': 'mihomo-linux-armv7',
  'linux-riscv64': 'mihomo-linux-riscv64',
  'linux-loong64': 'mihomo-linux-loong64',
}

// Release discovery
async function getLatestAlphaVersion() {
  // Same rule as getLatestReleaseVersion: a pinned alpha is fetched, not chosen.
  const pinned = lockedEngineVersion('mihomo_alpha')
  if (pinned) {
    META_ALPHA_VERSION = pinned
    log_info(`mihomo alpha pinned by the lock: ${META_ALPHA_VERSION}`)
    return
  }
  if (!FORCE) {
    const cached = await getCachedVersion('META_ALPHA_VERSION')
    if (cached) {
      META_ALPHA_VERSION = cached
      return
    }
  }
  const options = {}
  const httpProxy =
    process.env.HTTP_PROXY ||
    process.env.http_proxy ||
    process.env.HTTPS_PROXY ||
    process.env.https_proxy
  if (httpProxy) options.agent = new HttpsProxyAgent(httpProxy)

  try {
    const response = await fetch(META_ALPHA_VERSION_URL, {
      ...options,
      method: 'GET',
    })
    if (!response.ok)
      throw new Error(
        `Failed to fetch ${META_ALPHA_VERSION_URL}: ${response.status}`,
      )
    META_ALPHA_VERSION = (await response.text()).trim()
    log_info(`Latest alpha version: ${META_ALPHA_VERSION}`)
    await setCachedVersion('META_ALPHA_VERSION', META_ALPHA_VERSION)
  } catch (err) {
    log_error('Error fetching latest alpha version:', err.message)
    process.exit(1)
  }
}

async function getLatestReleaseVersion() {
  // The lock decides WHICH engine; the network only decides where to get it.
  const pinned = lockedEngineVersion('mihomo')
  if (pinned) {
    META_VERSION = pinned
    log_info(`mihomo version pinned by the lock: ${META_VERSION}`)
    return
  }
  if (!FORCE) {
    const cached = await getCachedVersion('META_VERSION')
    if (cached) {
      META_VERSION = cached
      return
    }
  }
  const options = {}
  const httpProxy =
    process.env.HTTP_PROXY ||
    process.env.http_proxy ||
    process.env.HTTPS_PROXY ||
    process.env.https_proxy
  if (httpProxy) options.agent = new HttpsProxyAgent(httpProxy)

  try {
    const response = await fetch(META_VERSION_URL, {
      ...options,
      method: 'GET',
    })
    if (!response.ok)
      throw new Error(`Failed to fetch ${META_VERSION_URL}: ${response.status}`)
    META_VERSION = (await response.text()).trim()
    log_info(`Latest release version: ${META_VERSION}`)
    await setCachedVersion('META_VERSION', META_VERSION)
  } catch (err) {
    log_error('Error fetching latest release version:', err.message)
    process.exit(1)
  }
}

if (!META_MAP[`${platform}-${arch}`]) {
  throw new Error(`clash meta unsupported platform "${platform}-${arch}"`)
}
if (!META_ALPHA_MAP[`${platform}-${arch}`]) {
  throw new Error(`clash meta alpha unsupported platform "${platform}-${arch}"`)
}

function clashMetaAlpha() {
  const name = META_ALPHA_MAP[`${platform}-${arch}`]
  const isWin = platform === 'win32'
  const urlExt = isWin ? 'zip' : 'gz'
  return {
    name: 'verge-mihomo-alpha',
    targetFile: `verge-mihomo-alpha-${SIDECAR_HOST}${isWin ? '.exe' : ''}`,
    exeFile: `${name}${isWin ? '.exe' : ''}`,
    zipFile: `${name}-${META_ALPHA_VERSION}.${urlExt}`,
    downloadURL: `${META_ALPHA_URL_PREFIX}/${name}-${META_ALPHA_VERSION}.${urlExt}`,
  }
}

function clashMeta() {
  const name = META_MAP[`${platform}-${arch}`]
  const isWin = platform === 'win32'
  const urlExt = isWin ? 'zip' : 'gz'
  return {
    name: 'verge-mihomo',
    targetFile: `verge-mihomo-${SIDECAR_HOST}${isWin ? '.exe' : ''}`,
    exeFile: `${name}${isWin ? '.exe' : ''}`,
    zipFile: `${name}-${META_VERSION}.${urlExt}`,
    downloadURL: `${META_URL_PREFIX}/${META_VERSION}/${name}-${META_VERSION}.${urlExt}`,
  }
}

/// The download URL for a geo database, pinned by release-asset ID.
///
/// WHY NOT `releases/download/latest/...`. `meta-rules-dat` publishes a single
/// rolling `latest` release and keeps no dated tag, so that URL points at
/// whatever was rebuilt last. Observed 2026-10-26: `country.mmdb` changed
/// between two runs minutes apart, silently. A build is not reproducible if a
/// file it embeds can change under it.
///
/// A GitHub release ASSET ID is permanent: the ID names one upload and its
/// contents never change. `GET /releases/assets/<id>` with
/// `Accept: application/octet-stream` returns those exact bytes forever, so a
/// pinned date is addressed by ID and the hash check confirms it.
///
/// Falls back to the legacy `latest` URL only when the lock has no ID, so an
/// unpinned checkout still works (and says so).
function geoDownloadUrl(file, legacyUrl) {
  const id = SIDECAR_LOCK?.resources?.[file]?.asset_id
  if (!id) return legacyUrl
  return `https://api.github.com/repos/MetaCubeX/meta-rules-dat/releases/assets/${id}`
}

/// Headers required to read a release asset by ID. Without the Accept header
/// the API answers with JSON metadata instead of the bytes.
function geoDownloadHeaders(url) {
  return url.includes('/releases/assets/')
    ? { Accept: 'application/octet-stream' }
    : { 'Content-Type': 'application/octet-stream' }
}

async function downloadFile(url, outPath) {
  const options = {}
  const httpProxy =
    process.env.HTTP_PROXY ||
    process.env.http_proxy ||
    process.env.HTTPS_PROXY ||
    process.env.https_proxy
  if (httpProxy) options.agent = new HttpsProxyAgent(httpProxy)

  const response = await fetch(url, {
    ...options,
    method: 'GET',
    // An asset-ID URL needs `Accept: application/octet-stream` or the API
    // returns JSON metadata rather than the file.
    headers: url.includes('/releases/assets/')
      ? { Accept: 'application/octet-stream' }
      : { 'Content-Type': 'application/octet-stream' },
  })
  if (!response.ok) {
    const body = await response.text().catch(() => '')
    await fsp.mkdir(path.dirname(outPath), { recursive: true })
    await fsp.writeFile(outPath, body)
    throw new Error(`Failed to download ${url}: status ${response.status}`)
  }

  const buf = Buffer.from(await response.arrayBuffer())
  await fsp.mkdir(path.dirname(outPath), { recursive: true })

  if (url.endsWith('.gz') || url.endsWith('.tgz')) {
    if (!(buf[0] === 0x1f && buf[1] === 0x8b)) {
      await fsp.writeFile(outPath, buf)
      throw new Error(
        `Downloaded file for ${url} is not a valid gzip (magic mismatch).`,
      )
    }
  } else if (url.endsWith('.zip')) {
    if (!(buf[0] === 0x50 && buf[1] === 0x4b)) {
      await fsp.writeFile(outPath, buf)
      throw new Error(
        `Downloaded file for ${url} is not a valid zip (magic mismatch).`,
      )
    }
  }

  await fsp.writeFile(outPath, buf)
  log_success(`download finished: ${url}`)
}

async function resolveSidecar(binInfo) {
  const { name, targetFile, zipFile, exeFile, downloadURL } = binInfo
  const sidecarPath = path.join(SIDECAR_DIR, targetFile)
  await fsp.mkdir(SIDECAR_DIR, { recursive: true })

  // "Already exists" is not the same as "is what the lock pins". A cached
  // binary left over from a previous engine version would otherwise be reused
  // silently — the exact drift the lock exists to stop — so an existing file is
  // verified rather than trusted.
  const pinnedHash = lockedSidecarHash(targetFile)

  // A vendored sidecar is committed, so there is nothing to fetch. Verify it and
  // stop: downloading one would defeat the reason it exists, and --force must
  // not delete the only copy — that is exactly how the alpha was lost once.
  if (lockedSidecarVendored(targetFile)) {
    await assertPinned(sidecarPath, pinnedHash, `"${name}" (vendored)`)
    log_success(`"${name}" is vendored in the repository and matches its pin`)
    if (platform !== 'win32') await fsp.chmod(sidecarPath, 0o755)
    return
  }

  // `--force` means "re-download", not "discard what is here first". If the
  // file already matches its pin there is nothing to gain by fetching it again
  // — the bytes would be identical — and if the fetch fails we would have
  // gambled an irreplaceable file for no reason. So a matching file is kept
  // even under --force, and --force only short-circuits the *skip* decision.
  if (fs.existsSync(sidecarPath) && pinnedHash) {
    const actual = await calculateFileHash(sidecarPath)
    if (actual === pinnedHash) {
      log_success(
        `"${name}" matches its pin${FORCE ? ' — kept (nothing to re-download)' : ', skipping download'}`,
      )
      return
    }
    log_warn(
      `"${name}" on disk does not match its pin (${String(actual).slice(0, 16)} != ${pinnedHash.slice(0, 16)}) — re-downloading`,
    )
  } else if (!FORCE && fs.existsSync(sidecarPath)) {
    log_success(`"${name}" already exists, skipping download (UNPINNED)`)
    return
  }

  const tempDir = path.join(TEMP_DIR, name)
  const tempZip = path.join(tempDir, zipFile)
  const tempExe = path.join(tempDir, exeFile)
  await fsp.mkdir(tempDir, { recursive: true })

  try {
    if (!fs.existsSync(tempZip)) {
      await downloadFile(downloadURL, tempZip)
    }

    if (zipFile.endsWith('.zip')) {
      const zip = new AdmZip(tempZip)
      zip.getEntries().forEach((entry) => {
        log_debug(`"${name}" entry: ${entry.entryName}`)
      })
      zip.extractAllTo(tempDir, true)
      if (fs.existsSync(tempExe)) {
        await fsp.rename(tempExe, sidecarPath)
      } else {
        const files = await fsp.readdir(tempDir)
        const candidate = files.find(
          (f) =>
            f === path.basename(exeFile) ||
            f.endsWith('.exe') ||
            !f.includes('.'),
        )
        if (!candidate)
          throw new Error(`Expected binary not found in ${tempDir}`)
        await fsp.rename(path.join(tempDir, candidate), sidecarPath)
      }
      if (platform !== 'win32') await fsp.chmod(sidecarPath, 0o755)
      log_success(`unzip finished: "${name}"`)
    } else if (zipFile.endsWith('.tgz')) {
      await extract({ cwd: tempDir, file: tempZip })
      const files = await fsp.readdir(tempDir)
      log_debug(`"${name}" extracted files:`, files)
      let extracted = files.find(
        (f) =>
          f === path.basename(exeFile) ||
          f.startsWith('虚空终端-') ||
          !f.includes('.'),
      )
      if (!extracted) extracted = files[0]
      if (!extracted) throw new Error(`Expected file not found in ${tempDir}`)
      await fsp.rename(path.join(tempDir, extracted), sidecarPath)
      await fsp.chmod(sidecarPath, 0o755)
      log_success(`tgz processed: "${name}"`)
    } else {
      const readStream = fs.createReadStream(tempZip)
      const writeStream = fs.createWriteStream(sidecarPath)
      await new Promise((resolve, reject) => {
        readStream
          .pipe(zlib.createGunzip())
          .on('error', (e) => {
            log_error(`gunzip error for ${name}:`, e.message)
            reject(e)
          })
          .pipe(writeStream)
          .on('finish', () => {
            if (platform !== 'win32')
              execFileSync('chmod', ['755', sidecarPath])
            resolve()
          })
          .on('error', (e) => {
            log_error(`write stream error for ${name}:`, e.message)
            reject(e)
          })
      })
      log_success(`gz binary processed: "${name}"`)
    }

    // Executable bit, applied once for every archive shape.
    //
    // WHY IT IS HERE AND NOT ONLY INSIDE THE STREAMS. The `.gz` path chmods in
    // the write stream's `finish` handler, which can run after the enclosing
    // promise has resolved — so the rename and chmod can interleave, and the
    // file lands non-executable. Observed on 2026-10-26: after a `--force` run
    // the stable engine was present with the right content but mode 0644, and
    // the binary refused to run. A sidecar that cannot be executed is worse
    // than a missing one, because everything downstream reports "not found".
    if (platform !== 'win32') await fsp.chmod(sidecarPath, 0o755)

    // The pin is checked once, here, for every archive shape.
    await assertPinned(sidecarPath, pinnedHash, `"${name}"`)
  } catch (err) {
    // DO NOT delete sidecarPath here.
    //
    // This line used to `rm` the sidecar on ANY failure. Because a failed
    // download throws before the new binary is in place, that turned a
    // transient network error — or an artifact that upstream has deleted —
    // into permanent loss of a file the repository cannot supply: the sidecars
    // are gitignored downloads with no copy in git.
    //
    // That is not hypothetical. On 2026-10-26 a `--force` run hit a 404 for
    // `mihomo-linux-amd64-v2-alpha-94b13ae.gz` (MetaCubeX deletes older alpha
    // builds) and this line deleted the only copy of the alpha that existed
    // anywhere. It could not be recovered.
    //
    // The new binary is written via `rename` onto sidecarPath only after a
    // successful download and extraction, so a failure leaves the previous
    // file intact. Keeping it is strictly safer than deleting it.
    const hadPrevious = fs.existsSync(sidecarPath)
    log_warn(
      `"${name}" failed: ${err.message}` +
        (hadPrevious
          ? ' — the existing binary was LEFT IN PLACE'
          : ' — no binary is present for this sidecar'),
    )
    throw err
  } finally {
    await fsp.rm(tempDir, { recursive: true, force: true })
  }
}

async function resolveResource(binInfo) {
  const { file, downloadURL, localPath, dir } = binInfo
  const baseDir = dir ?? RESOURCES_DIR
  const targetPath = path.join(baseDir, file)

  if (!FORCE && fs.existsSync(targetPath) && !downloadURL && !localPath) {
    log_success(`"${file}" already exists, skipping`)
    return
  }

  if (downloadURL) {
    // Same rule as the sidecars: an existing file is verified, not trusted.
    // The geo databases are pinned in the lock too, and a "latest" redirect for
    // `meta-rules-dat` is exactly as capable of changing under us as mihomo's
    // `releases/latest`.
    const pinnedHash = lockedResourceHash(file)
    if (!FORCE && fs.existsSync(targetPath)) {
      if (pinnedHash) {
        const actual = await calculateFileHash(targetPath)
        if (actual === pinnedHash) {
          log_success(`"${file}" already exists and matches its pin, skipping download`)
          return
        }
        log_warn(
          `"${file}" on disk does not match its pin — re-downloading`,
        )
      } else {
        log_success(`"${file}" already exists, skipping download (UNPINNED)`)
        return
      }
    }
    await fsp.mkdir(baseDir, { recursive: true })
    await downloadFile(downloadURL, targetPath)
    await assertPinned(targetPath, pinnedHash, `"${file}"`)
    await updateHashCache(targetPath)
  }

  if (localPath) {
    if (!(await hasFileChanged(localPath, targetPath))) {
      return
    }
    await fsp.mkdir(baseDir, { recursive: true })
    await fsp.copyFile(localPath, targetPath)
    await updateHashCache(targetPath)
    log_success(`Copied file: ${file}`)
  }

  log_success(`${file} finished`)
}

// Windows NSIS plugin
const resolvePlugin = async () => {
  const url =
    'https://nsis.sourceforge.io/mediawiki/images/e/ef/NSIS_Simple_Service_Plugin_Unicode_1.30.zip'
  const tempDir = path.join(TEMP_DIR, 'SimpleSC')
  const tempZip = path.join(
    tempDir,
    'NSIS_Simple_Service_Plugin_Unicode_1.30.zip',
  )
  const tempDll = path.join(tempDir, 'SimpleSC.dll')
  const pluginDir = path.join(process.env.APPDATA || '', 'Local/NSIS')
  const pluginPath = path.join(pluginDir, 'SimpleSC.dll')
  await fsp.mkdir(pluginDir, { recursive: true })
  await fsp.mkdir(tempDir, { recursive: true })
  if (!FORCE && fs.existsSync(pluginPath)) return
  try {
    if (!fs.existsSync(tempZip)) {
      await downloadFile(url, tempZip)
    }
    const zip = new AdmZip(tempZip)
    zip
      .getEntries()
      .forEach((entry) => void log_debug(`"SimpleSC" entry`, entry.entryName))
    zip.extractAllTo(tempDir, true)
    if (fs.existsSync(tempDll)) {
      await fsp.cp(tempDll, pluginPath, { recursive: true, force: true })
      log_success(`unzip finished: "SimpleSC"`)
    } else {
      const files = await fsp.readdir(tempDir)
      const dll = files.find((f) => f.toLowerCase().endsWith('.dll'))
      if (dll) {
        await fsp.cp(path.join(tempDir, dll), pluginPath, {
          recursive: true,
          force: true,
        })
        log_success(`unzip finished: "SimpleSC" (found ${dll})`)
      } else {
        throw new Error('SimpleSC.dll not found in zip')
      }
    }
  } finally {
    await fsp.rm(tempDir, { recursive: true, force: true })
  }
}

// Service executable permissions
const resolveServicePermission = async () => {
  const serviceExecutables = [
    'clash-verge-service*',
    'clash-verge-service-install*',
    'clash-verge-service-uninstall*',
  ]
  const hashCache = await loadHashCache()
  let hasChanges = false

  for (const f of serviceExecutables) {
    const files = glob.sync(path.join(SERVICE_DIR, f))
    for (const filePath of files) {
      if (fs.existsSync(filePath)) {
        const currentHash = await calculateFileHash(filePath)
        const cacheKey = `${filePath}_chmod`
        if (!FORCE && hashCache[cacheKey] === currentHash) {
          continue
        }
        try {
          execSync(`chmod 755 ${filePath}`)
          log_success(`chmod finished: "${filePath}"`)
        } catch (e) {
          log_error(`chmod failed for ${filePath}:`, e.message)
        }
        hashCache[cacheKey] = currentHash
        hasChanges = true
      }
    }
  }

  if (hasChanges) {
    await saveHashCache(hashCache)
  }
}

// Other resources
const SERVICE_BINARIES = [
  'clash-verge-service',
  'clash-verge-service-install',
  'clash-verge-service-uninstall',
]

function serviceFileInfo(name) {
  const ext = platform === 'win32' ? '.exe' : ''
  const suffix = platform === 'linux' ? `-${SIDECAR_HOST}` : ''
  return {
    sourceFile: `${name}${ext}`,
    targetFile: `${name}${suffix}${ext}`,
  }
}

async function findExtractedFile(dir, fileName) {
  const entries = await fsp.readdir(dir, { withFileTypes: true })
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name)
    if (entry.isFile() && entry.name === fileName) return entryPath
    if (entry.isDirectory()) {
      const found = await findExtractedFile(entryPath, fileName)
      if (found) return found
    }
  }
  return null
}

async function resolveServiceBundle() {
  const files = SERVICE_BINARIES.map((name) => {
    const info = serviceFileInfo(name)
    return {
      ...info,
      targetPath: path.join(SERVICE_DIR, info.targetFile),
    }
  })

  const cargoManifest = await fsp.readFile(
    path.join(cwd, 'src-tauri', 'Cargo.toml'),
    'utf8',
  )
  const serviceDependency = cargoManifest
    .split(/\r?\n/)
    .find((line) => line.trimStart().startsWith('clash_verge_service_ipc ='))
  const sourcePath = serviceDependency?.match(/\bpath\s*=\s*"([^"]+)"/)?.[1]
  if (sourcePath) {
    const manifest = path.resolve(cwd, 'src-tauri', sourcePath, 'Cargo.toml')
    const targetDirectory = path.join(cwd, 'target', 'bundled-service')
    execFileSync(
      'cargo',
      [
        'build',
        '--manifest-path',
        manifest,
        '--target-dir',
        targetDirectory,
        '--target',
        SIDECAR_HOST,
        '--release',
        '--features',
        'standalone,client',
        '--bins',
      ],
      { stdio: 'inherit' },
    )
    await fsp.mkdir(SERVICE_DIR, { recursive: true })
    for (const { sourceFile, targetPath } of files) {
      await fsp.copyFile(
        path.join(targetDirectory, SIDECAR_HOST, 'release', sourceFile),
        targetPath,
      )
      if (platform !== 'win32') await fsp.chmod(targetPath, 0o755)
      await updateHashCache(targetPath)
    }
    return
  }
  const { archiveFile, downloadURL } = resolveServiceRelease(
    cargoManifest,
    SIDECAR_HOST,
    platform,
  )
  const tempDir = path.join(TEMP_DIR, 'clash-verge-service-ipc')
  const tempArchive = path.join(tempDir, archiveFile)

  await fsp.mkdir(tempDir, { recursive: true })
  await fsp.mkdir(SERVICE_DIR, { recursive: true })

  try {
    await downloadFile(downloadURL, tempArchive)

    if (platform === 'win32') {
      const zip = new AdmZip(tempArchive)
      zip
        .getEntries()
        .forEach(
          (entry) =>
            void log_debug('"clash-verge-service-ipc" entry:', entry.entryName),
        )
      zip.extractAllTo(tempDir, true)
    } else {
      await extract({ cwd: tempDir, file: tempArchive })
    }

    for (const { sourceFile, targetFile, targetPath } of files) {
      const extractedFile = await findExtractedFile(tempDir, sourceFile)
      if (!extractedFile) {
        throw new Error(`Expected binary ${sourceFile} not found in archive`)
      }

      await fsp.copyFile(extractedFile, targetPath)
      if (platform !== 'win32') await fsp.chmod(targetPath, 0o755)
      await updateHashCache(targetPath)
      log_success(`Extracted service file: ${targetFile}`)
    }

    log_success(`service bundle finished: ${archiveFile}`)
  } finally {
    await fsp.rm(tempDir, { recursive: true, force: true })
  }
}

/// REMOVED in 3.2.10: `resolveCoreHashes` / `core-hashes.nsh`.
///
/// It generated an `installerHooks` `.nsh` that only the project's custom copy of
/// `installer.nsi` included, and it ran as a `winOnly` task inside `prebuild` — which
/// every CI build job skips, because `TAURI_CONFIG` blanks `beforeBuildCommand` (the
/// frontend bundle is built once in `verify`). The file was therefore never generated
/// where the Windows bundle was built, the bundler's one `!include "{{installer_hooks}}"`
/// pointed at a path that did not exist, and makensis aborted with `!include: could not
/// open file` — NSIS's own "File not found" dialog.
///
/// The custom template went with it in the same release, so nothing consumed these
/// digests any more. Core digests still reach the service by the route that always
/// worked: `externalBin` bundles the cores as sidecars, and the installer publishes the
/// shipped bytes. Nothing here should be reintroduced without both the template and a
/// build step that can actually generate the file on Windows.

const resolveMmdb = () =>
  resolveResource({
    file: 'Country.mmdb',
    downloadURL: geoDownloadUrl(
      'Country.mmdb',
      `https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/country.mmdb`,
    ),
  })
const resolveASNMmdb = () =>
  resolveResource({
    file: 'ASN.mmdb',
    downloadURL: geoDownloadUrl(
      'ASN.mmdb',
      `https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/GeoLite2-ASN.mmdb`,
    ),
  })
const resolveGeosite = () =>
  resolveResource({
    file: 'geosite.dat',
    downloadURL: geoDownloadUrl(
      'geosite.dat',
      `https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat`,
    ),
  })
const resolveGeoIP = () =>
  resolveResource({
    file: 'geoip.dat',
    downloadURL: geoDownloadUrl(
      'geoip.dat',
      `https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.dat`,
    ),
  })
const resolveEnableLoopback = () =>
  resolveResource({
    file: 'enableLoopback.exe',
    downloadURL: `https://github.com/Kuingsmile/uwp-tool/releases/download/latest/enableLoopback.exe`,
  })

const resolveSetDnsScript = () =>
  resolveResource({
    file: 'set_dns.sh',
    localPath: path.join(cwd, 'scripts/set_dns.sh'),
  })
const resolveUnSetDnsScript = () =>
  resolveResource({
    file: 'unset_dns.sh',
    localPath: path.join(cwd, 'scripts/unset_dns.sh'),
  })

const tasks = [
  {
    // `optional: true` because nothing in the product can select this core, and
    // upstream deletes old alpha builds so the pinned one may not exist. See
    // the runTask comment and docs/reference/STILL-OPEN.md.
    name: 'verge-mihomo-alpha',
    func: () =>
      getLatestAlphaVersion().then(() => resolveSidecar(clashMetaAlpha())),
    retry: 5,
    optional: true,
  },
  {
    name: 'verge-mihomo',
    func: () =>
      getLatestReleaseVersion().then(() => resolveSidecar(clashMeta())),
    retry: 5,
  },
  { name: 'plugin', func: resolvePlugin, retry: 5, winOnly: true },
  { name: 'service', func: resolveServiceBundle, retry: 5 },
  { name: 'mmdb', func: resolveMmdb, retry: 5 },
  { name: 'asn_mmdb', func: resolveASNMmdb, retry: 5 },
  { name: 'geosite', func: resolveGeosite, retry: 5 },
  { name: 'geoip', func: resolveGeoIP, retry: 5 },
  {
    name: 'enableLoopback',
    func: resolveEnableLoopback,
    retry: 5,
    winOnly: true,
  },
  {
    name: 'service_chmod',
    func: resolveServicePermission,
    retry: 5,
    unixOnly: platform === 'linux' || platform === 'darwin',
  },
  {
    name: 'set_dns_script',
    func: resolveSetDnsScript,
    retry: 5,
    macosOnly: true,
  },
  {
    name: 'unset_dns_script',
    func: resolveUnSetDnsScript,
    retry: 5,
    macosOnly: true,
  },
]

async function runTask() {
  const task = tasks.shift()
  if (!task) return
  if (task.unixOnly && platform === 'win32') return runTask()
  if (task.winOnly && platform !== 'win32') return runTask()
  if (task.macosOnly && platform !== 'darwin') return runTask()
  if (task.linuxOnly && platform !== 'linux') return runTask()

  for (let i = 0; i < task.retry; i++) {
    try {
      await task.func()
      break
    } catch (err) {
      log_error(`task::${task.name} try ${i} ==`, err.message)
      if (i === task.retry - 1) {
        // A task explicitly marked `optional` failing does not fail the build.
        //
        // WHY: the alpha engine is listed in `externalBin`, but nothing in the
        // product can select it — `config/verge.rs` forces `clash_core` to
        // `verge-mihomo` and no UI calls `change_clash_core`. Meanwhile
        // MetaCubeX deletes old alpha prereleases, so the pinned alpha can
        // become undownloadable at any time through no fault of this tree.
        // Treating that as fatal makes every fresh clone unbuildable for an
        // artifact the product never runs.
        //
        // It is NOT silent: the failure is logged above, and the eventual
        // Tauri bundling step will refuse to produce an installer that expects
        // a sidecar it does not have. The decision to drop the alpha from
        // `externalBin` is recorded in docs/reference/STILL-OPEN.md.
        if (task.optional) {
          log_warn(
            `task::${task.name} is optional and could not be fetched — continuing. ` +
              `The build may fail at bundling if this sidecar is required by tauri.conf.json.`,
          )
          break
        }
        throw err
      }
    }
  }
  return runTask()
}

runTask()
