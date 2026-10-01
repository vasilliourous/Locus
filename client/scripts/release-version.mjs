/**
 * CLI tool to update every version site in one command.
 *
 * Usage:
 *   cd client && pnpm release-version <version>
 *
 * It writes FIVE files. Four are manifests:
 *   - package.json
 *   - src-tauri/Cargo.toml
 *   - Cargo.lock
 *   - src-tauri/tauri.conf.json
 *
 * The fifth is NOT a manifest — `../docs/state.toml`'s derived
 * `[client.version]`. It is a fact about the manifests, and
 * `server/scripts/check-consistency.sh` §7 fails the `verify` job if it
 * disagrees with `package.json`. It is handled here so a release cannot forget
 * it; see `updateStateTomlVersion` for why.
 *
 * THIS SCRIPT IS ONLY HALF OF CUTTING A RELEASE. It edits files; it does not
 * commit or tag, and CI does NOT bump versions for you — it labels the release
 * and the manifest from the git tag. So the order is:
 *
 *   1. cd client && pnpm release-version X.Y.Z
 *   2. git add -A && git commit -m "chore(release): X.Y.Z"
 *   3. git tag -a vX.Y.Z -m "Locus client vX.Y.Z" && git push origin vX.Y.Z
 *
 * Skipping step 1 and tagging anyway is what shipped 3.2.12 binaries under a
 * v3.2.13 tag on 2026-09-30. The `verify` job now fails a tag whose name does
 * not match `package.json` ("Tag must name the version being built"), so the
 * mistake costs a red check instead of a release. Full procedure:
 * `../docs/operate/RELEASING.md`.
 *
 * <version> can be:
 *   - A full semver version (e.g., 1.2.3, v1.2.3, 1.2.3-beta, v1.2.3+build)
 *   - A tag: "alpha", "beta", "rc", "autobuild", "autobuild-latest", or "deploytest"
 *     - "alpha", "beta", "rc": Appends the tag to the current base version (e.g., 1.2.3-beta)
 *     - "autobuild": Appends a timestamped autobuild tag (e.g., 1.2.3+autobuild.2406101530)
 *     - "autobuild-latest": Appends an autobuild tag with latest Tauri commit (e.g., 1.2.3+autobuild.0614.a1b2c3d)
 *     - "deploytest": Appends a timestamped deploytest tag (e.g., 1.2.3+deploytest.2406101530)
 *
 * Examples:
 *   pnpm release-version 1.2.3
 *   pnpm release-version v1.2.3-beta
 *   pnpm release-version beta
 *   pnpm release-version autobuild
 *   pnpm release-version autobuild-latest
 *   pnpm release-version deploytest
 *
 */

import { execSync } from 'child_process'
import fs from 'fs/promises'
import path from 'path'

import { program } from 'commander'

function getGitShortCommit() {
  try {
    return execSync('git rev-parse --short HEAD').toString().trim()
  } catch {
    console.warn("[WARN]: Failed to get git short commit, fallback to 'nogit'")
    return 'nogit'
  }
}

function getLatestTauriCommit() {
  // The upstream helper `scripts-workflow/get_latest_tauri_commit.bash` was
  // removed with the rest of the Clash Verge release tooling. Fall back to the
  // current git short commit for the autobuild/deploytest tag format.
  return getGitShortCommit()
}

/** Generates `MMDD`, optionally followed by a commit, in the Asia/Shanghai timezone. */
function generateShortTimestamp(withCommit = false, useTauriCommit = false) {
  const now = new Date()

  const formatter = new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
  })

  const parts = formatter.formatToParts(now)
  const month = parts.find((part) => part.type === 'month').value
  const day = parts.find((part) => part.type === 'day').value

  if (withCommit) {
    const gitShort = useTauriCommit
      ? getLatestTauriCommit()
      : getGitShortCommit()
    return `${month}${day}.${gitShort}`
  }
  return `${month}${day}`
}

function isValidVersion(version) {
  return /^v?\d+\.\d+\.\d+(-(alpha|beta|rc)(\.\d+)?)?(\+[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)*)?$/i.test(
    version,
  )
}

function normalizeVersion(version) {
  return version.startsWith('v') ? version : `v${version}`
}

function getBaseVersion(version) {
  let base = version.replace(/-(alpha|beta|rc)(\.\d+)?/i, '')
  base = base.replace(/\+[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)*/g, '')
  return base
}

async function updatePackageVersion(newVersion) {
  const _dirname = process.cwd()
  const packageJsonPath = path.join(_dirname, 'package.json')
  try {
    const data = await fs.readFile(packageJsonPath, 'utf8')
    const packageJson = JSON.parse(data)

    console.log(
      '[INFO]: Current package.json version is: ',
      packageJson.version,
    )
    const versionWithoutV = newVersion.startsWith('v')
      ? newVersion.slice(1)
      : newVersion
    const updatedData = data.replace(
      /^(\s*"version"\s*:\s*)"[^"]+"/m,
      `$1"${versionWithoutV}"`,
    )

    // `=== data` means the replace matched nothing, which happens for TWO very
    // different reasons: the field is absent, or it already holds this version.
    // The original code reported both as "version field was not found", so
    // re-running a bump printed `[ERROR]: Failed to update versions` on a tree
    // that was already correct — alarming, and indistinguishable from a real
    // failure. Say which one it is.
    if (updatedData === data) {
      if (new RegExp(`^\\s*"version"\\s*:\\s*"${versionWithoutV}"`, 'm').test(data)) {
        console.log(
          `[INFO]: package.json is already at ${versionWithoutV} — no change`,
        )
        return
      }
      throw new Error('version field was not found in package.json')
    }

    await fs.writeFile(packageJsonPath, updatedData, 'utf8')
    console.log(`[INFO]: package.json version updated to: ${versionWithoutV}`)
  } catch (error) {
    console.error('Error updating package.json version:', error)
    throw error
  }
}

async function updateCargoVersion(newVersion) {
  const _dirname = process.cwd()
  const cargoTomlPath = path.join(_dirname, 'src-tauri', 'Cargo.toml')
  try {
    const data = await fs.readFile(cargoTomlPath, 'utf8')
    const lines = data.split('\n')
    const versionWithoutV = newVersion.startsWith('v')
      ? newVersion.slice(1)
      : newVersion

    const updatedLines = lines.map((line) => {
      if (line.trim().startsWith('version =')) {
        return line.replace(
          /version\s*=\s*"[^"]+"/,
          `version = "${versionWithoutV}"`,
        )
      }
      return line
    })

    await fs.writeFile(cargoTomlPath, updatedLines.join('\n'), 'utf8')
    console.log(`[INFO]: Cargo.toml version updated to: ${versionWithoutV}`)
  } catch (error) {
    console.error('Error updating Cargo.toml version:', error)
    throw error
  }
}

async function updateCargoLockVersion(newVersion) {
  const _dirname = process.cwd()
  const cargoLockPath = path.join(_dirname, 'Cargo.lock')
  const versionWithoutV = newVersion.startsWith('v')
    ? newVersion.slice(1)
    : newVersion
  // The workspace package is `locus` (renamed from upstream `clash-verge`); this
  // pattern still matched the old name and so failed the whole bump with
  // "clash-verge package entry was not found", after having already written the
  // other version sites. Kept name-aware so a rename cannot silently skip the
  // lockfile again.
  const packageVersionPattern =
    /(\[\[package\]\]\r?\nname = "locus"\r?\nversion = )"[^"]+"/

  try {
    const data = await fs.readFile(cargoLockPath, 'utf8')
    const updatedData = data.replace(
      packageVersionPattern,
      `$1"${versionWithoutV}"`,
    )

    // Already at this version, or the entry is genuinely absent — see the note
    // in `updatePackageVersion` for why these must be told apart.
    if (updatedData === data) {
      if (
        new RegExp(
          `\\[\\[package\\]\\]\\r?\\nname = "locus"\\r?\\nversion = "${versionWithoutV}"`,
        ).test(data)
      ) {
        console.log(
          `[INFO]: Cargo.lock is already at ${versionWithoutV} — no change`,
        )
        return
      }
      throw new Error('locus package entry was not found in Cargo.lock')
    }

    await fs.writeFile(cargoLockPath, updatedData, 'utf8')
    console.log(`[INFO]: Cargo.lock version updated to: ${versionWithoutV}`)
  } catch (error) {
    console.error('Error updating Cargo.lock version:', error)
    throw error
  }
}

async function updateTauriConfigVersion(newVersion) {
  const _dirname = process.cwd()
  const tauriConfigPath = path.join(_dirname, 'src-tauri', 'tauri.conf.json')
  try {
    const data = await fs.readFile(tauriConfigPath, 'utf8')
    const tauriConfig = JSON.parse(data)
    const versionWithoutV = newVersion.startsWith('v')
      ? newVersion.slice(1)
      : newVersion

    console.log(
      '[INFO]: Current tauri.conf.json version is: ',
      tauriConfig.version,
    )

    const updatedData = data.replace(
      /^(\s*"version"\s*:\s*)"[^"]+"/m,
      `$1"${versionWithoutV}"`,
    )

    // Already at this version, or the field is genuinely absent — see the note
    // in `updatePackageVersion`.
    if (updatedData === data) {
      if (new RegExp(`^\\s*"version"\\s*:\\s*"${versionWithoutV}"`, 'm').test(data)) {
        console.log(
          `[INFO]: tauri.conf.json is already at ${versionWithoutV} — no change`,
        )
        return
      }
      throw new Error('version field was not found in tauri.conf.json')
    }

    await fs.writeFile(tauriConfigPath, updatedData, 'utf8')
    console.log(
      `[INFO]: tauri.conf.json version updated to: ${versionWithoutV}`,
    )
  } catch (error) {
    console.error('Error updating tauri.conf.json version:', error)
    throw error
  }
}

async function getCurrentVersion() {
  const _dirname = process.cwd()
  const packageJsonPath = path.join(_dirname, 'package.json')
  try {
    const data = await fs.readFile(packageJsonPath, 'utf8')
    const packageJson = JSON.parse(data)
    return packageJson.version
  } catch (error) {
    console.error('Error getting current version:', error)
    throw error
  }
}

/**
 * The FIFTH version site: `docs/state.toml`'s derived `[client.version]`.
 *
 * It is not a manifest — it is a fact ABOUT the manifests, recorded as data and
 * guarded by `server/scripts/check-consistency.sh` §7, which fails the `verify`
 * job with `BAD client.version` when it disagrees with `client/package.json`.
 *
 * WHY THIS IS HERE AND NOT JUST IN A DOCUMENT: on 2026-09-30 a release was
 * tagged without bumping anything, and the recovery attempt bumped only the four
 * files this script knew about — so CI rejected it for this one. A step that
 * lives only in prose gets skipped; a step in the tool that everyone already
 * runs does not. If `state.toml` moves, fix the path HERE.
 */
async function updateStateTomlVersion(newVersion) {
  const _dirname = process.cwd()
  // client/ -> repo root -> docs/state.toml
  const statePath = path.join(_dirname, '..', 'docs', 'state.toml')
  const versionWithoutV = newVersion.startsWith('v')
    ? newVersion.slice(1)
    : newVersion

  try {
    const data = await fs.readFile(statePath, 'utf8')
    // Anchored on the section header so this cannot rewrite `client.first_shipped`
    // or any other `value =` line. `[client.version]` is followed by its `value`
    // on the next line, so match the header and the one value beneath it.
    const sectionPattern = /(\[client\.version\]\r?\nvalue\s*=\s*)"[^"]+"/
    const updatedData = data.replace(sectionPattern, `$1"${versionWithoutV}"`)

    // Already at this version, or the section is genuinely absent — see the note
    // in `updatePackageVersion`. Told apart because a re-run must not look like
    // a failure: §7 of check-consistency.sh is the guard, not this message.
    if (updatedData === data) {
      if (
        new RegExp(
          `\\[client\\.version\\]\\r?\\nvalue\\s*=\\s*"${versionWithoutV}"`,
        ).test(data)
      ) {
        console.log(
          `[INFO]: docs/state.toml [client.version] is already at ${versionWithoutV} — no change`,
        )
        return
      }
      throw new Error('[client.version] value was not found in docs/state.toml')
    }

    await fs.writeFile(statePath, updatedData, 'utf8')
    console.log(
      `[INFO]: docs/state.toml [client.version] updated to: ${versionWithoutV}`,
    )
  } catch (error) {
    console.error('Error updating docs/state.toml version:', error)
    throw error
  }
}

async function main(versionArg) {
  if (!versionArg) {
    console.error('Error: Version argument is required')
    process.exit(1)
  }

  try {
    let newVersion
    const validTags = [
      'alpha',
      'beta',
      'rc',
      'autobuild',
      'autobuild-latest',
      'deploytest',
    ]

    if (validTags.includes(versionArg.toLowerCase())) {
      const currentVersion = await getCurrentVersion()
      const baseVersion = getBaseVersion(currentVersion)

      if (versionArg.toLowerCase() === 'autobuild') {
        newVersion = `${baseVersion}+autobuild.${generateShortTimestamp(true, true)}`
      } else if (versionArg.toLowerCase() === 'autobuild-latest') {
        const latestTauriCommit = getLatestTauriCommit()
        newVersion = `${baseVersion}+autobuild.${generateShortTimestamp()}.${latestTauriCommit}`
      } else if (versionArg.toLowerCase() === 'deploytest') {
        newVersion = `${baseVersion}+deploytest.${generateShortTimestamp(true, true)}`
      } else {
        newVersion = `${baseVersion}-${versionArg.toLowerCase()}`
      }
    } else {
      if (!isValidVersion(versionArg)) {
        console.error('Error: Invalid version format')
        process.exit(1)
      }
      newVersion = normalizeVersion(versionArg)
    }

    console.log(`[INFO]: Updating versions to: ${newVersion}`)
    await updatePackageVersion(newVersion)
    await updateCargoVersion(newVersion)
    await updateCargoLockVersion(newVersion)
    await updateTauriConfigVersion(newVersion)
    await updateStateTomlVersion(newVersion)
    console.log('[SUCCESS]: All version updates completed successfully!')
  } catch (error) {
    console.error('[ERROR]: Failed to update versions:', error)
    process.exit(1)
  }
}

program
  .name('pnpm release-version')
  .description('Update project version numbers')
  .argument('<version>', 'version tag or full version')
  .action(main)
  .parse(process.argv)
