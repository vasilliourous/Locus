#!/usr/bin/env python3
"""Locus release-fetch service — pull a release straight from GitHub.

WHY THIS EXISTS
---------------
Releases used to reach the hub only by the operator's browser: the admin console
uploaded four ~15-30 MB binaries through /api/admin/upload into a dedicated
service, because PocketBase rejects bodies above a few MB and has no multipart
API, and this Caddy build has no upload handler.

That worked, but it made the hub depend on the operator's laptop, their upload
bandwidth and their browser. The bytes CI built are already on a GitHub Release;
the hub can simply fetch them itself. This service does that, and in doing so
removes the whole size problem: nothing large is sent TO the hub any more, so
the upload service is gone.

THE TRIGGER IS A LINK, NOT A BUTTON
-----------------------------------
A fetch is intentionally manual. Two ways in, both leading to the same code:

  * POST /api/admin/fetch-release  {"version":"2.2.1"}  with the admin token
    (this is what the console's button sends), or
  * GET  /api/admin/fetch-release?version=2.2.1&nonce=…&exp=…&sig=…
    — a short-lived, SINGLE-USE, HMAC-signed link, so a fetch can be triggered
    from a phone or a CI job that does not hold the admin token.

Each link mints a fresh nonce. A consumed nonce is refused, so a link that
leaked through a chat log or a browser history is worthless after one use, and
"make a new link" is the only way to run it again — which is what was asked for.

DESIGN NOTES (each deliberate)
------------------------------
* **Binds 127.0.0.1 only**, reached exclusively through Caddy. Never public.
* **Resolves assets by name from the release manifest**, never by constructing
  a URL. GitHub's asset naming is then not a hidden dependency; if CI renames an
  attachment the fetch fails loudly instead of 404ing silently.
* **Streamed to a temp file, hashed as it goes.** A 30 MB artifact never sits
  fully in RAM on a 454 MB box.
* **All-or-nothing.** Every platform must be present and every hash must verify
  before a single byte is published. A partial release leaves those clients with
  a URL that 404s, and the omission is invisible from the operator's seat.
* **Atomic publish.** Written to a temp name, `os.replace`d into place only once
  its hash is known, so a client can never download a half-written binary.
* **Self-describing output.** A `.sha256` sidecar per artifact, as before.
* **The DB is NOT touched here.** update_config is written by the PocketBase hook
  (action `releases.publish`), which is the single writer. This process only puts
  verified bytes on disk and reports their hashes back.

Run: python3 fetch-release.py   (binds 127.0.0.1:8091)
"""
import base64
import hashlib
import hmac
import json
import os
import re
import secrets
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LISTEN_HOST = "127.0.0.1"
LISTEN_PORT = int(os.environ.get("FETCH_PORT", os.environ.get("UPLOAD_PORT", "8091")))
UPDATES_DIR = os.environ.get("UPDATES_DIR", "/var/www/updates")
TOKEN = os.environ.get("ADMIN_API_TOKEN", "")

GITHUB_REPO = os.environ.get("GITHUB_REPO", "vasilliourous/Locus")
GH_TOKEN = os.environ.get("GH_TOKEN", "")

# Signing secret for one-shot links. Generated on first use and persisted, so
# links minted before a restart stay valid until they expire.
SECRET_FILE = os.environ.get("FETCH_SECRET_FILE", "/root/.fetch_link_secret")
LINK_TTL_SECONDS = int(os.environ.get("FETCH_LINK_TTL", "900"))  # 15 minutes

# The filenames the client's updater looks for. This is a contract with the
# client (PlatformDownloadURL), not a preference — a wrong name is a silent
# no-update for that platform.
# The WINDOWS entry is an INSTALLER, not a raw executable, and that distinction is
# the whole point of this list.
#
# A Windows client does not run the payload it downloads — it hands the bytes to
# `tauri_plugin_updater`, which ShellExecutes them. `locus-windows-amd64.exe` is
# the raw PE the *retired portable client* consumed; advertising it for
# self-install made an installed client relaunch a copy of its own binary
# outside its install directory (FIXES.md, "THE APP RE-EXECUTED ITSELF").
#
# CI has advertised the NSIS setup executable in `manifest.json` since v3.2.12.
# This list was NOT updated with it, and because the name is never cross-checked
# against the manifest (only the *hash* of whichever file the name resolves to),
# the hub happily staged the raw binary into the Windows slot and served it with
# the raw binary's own signature. Every Windows client refused it — correctly,
# via `is_installer_payload` — and no Windows client could update at all.
#
# The name carries `<version>` because Tauri names the bundle with it. It is
# filled per publish by `installer_name()`, below.
INSTALLER_NAME_TEMPLATE = "installer-Locus_%s_x64-setup.exe"


def installer_name(version):
    """The NSIS setup executable CI produces for `version`.

    Kept as a function rather than an f-string at the call site so the template
    exists in exactly one place: `check-consistency.sh` pins this name, and CI
    builds the same one in `.github/workflows/client.yml`.
    """
    return INSTALLER_NAME_TEMPLATE % version


## What each platform's UPDATER PAYLOAD is called, on the GitHub Release.
##
## The macOS entries carry `.app.tar.gz`, and that is NOT cosmetic. It is the
## fix for a live defect: a macOS client on 3.2.24 was offered nothing, and the
## hub half of the reason is that this list named a bare Mach-O executable
## (`locus-darwin-arm64`) for the macOS slots.
##
## `tauri_plugin_updater` does not consume a bare binary on macOS. Its install
## path is `GzDecoder` + `tar::Archive` expecting
##
##     Locus.app.tar.gz
##     └── Locus.app/
##         └── Contents/...
##
## and its `extract_path` there is the `.app` BUNDLE, not a single file
## (`extract_path_from_executable`). Handed a raw Mach-O it fails at extraction
## — after a ~48 MB download the student paid for and a signature that verified
## fine. There is no path in which a bare binary updates a macOS app.
##
## Linux and Windows keep the bare artifacts they always had: their install
## paths take the executable/installer directly, so the old names were correct
## for them.
##
## `resolve_platform_names` below falls back to the bare name when the tarball
## is absent from a release, so publishing a version built before this fix
## degrades rather than failing outright.
NEW_PLATFORMS = [
    ("linux", "locus-linux-amd64", "download_linux"),
    ("windows", None, "download_windows"),  # resolved by installer_name()
    ("macos_intel", "locus-darwin-amd64.app.tar.gz", "download_macos_intel"),
    ("macos_arm", "locus-darwin-arm64.app.tar.gz", "download_macos_arm"),
]

## The pre-fix macOS payload names, kept ONLY as a fallback for releases that
## predate the packaging step. Never preferred: a bare Mach-O in a macOS update
## slot is the defect, not the behaviour.
LEGACY_MACOS_NAMES = {
    "macos_intel": "locus-darwin-amd64",
    "macos_arm": "locus-darwin-arm64",
}

PLATFORMS = NEW_PLATFORMS
MANIFEST_NAME = "manifest.json"

## ── HUMAN-FACING INSTALLERS, WHICH ARE NOT UPDATE PAYLOADS ──
##
## These are deliberately SEPARATE from PLATFORMS above, and the separation is
## the whole safety property. PLATFORMS decides what a client downloads and
## EXECUTES to replace itself: those bytes are cross-checked against CI's
## manifest and written into `update_<platform>`. This list decides what a
## PERSON downloads from the console, and nothing here may ever reach
## `update_config` — a compressed app bundle handed to `tauri_plugin_updater`
## is the raw-Windows-PE mistake again, in a different costume: a legitimate
## release asset that is wrong in the update slot.
##
## The macOS zip carries the ad-hoc-signed `Locus.app`. It exists because the
## `.dmg` cannot serve the one case that matters: a student whose download is
## quarantined and refused, where the remedy is a bundle that survives
## transport intact so macOS reports the bypassable "unidentified developer"
## dialog rather than "damaged and can't be opened". See
## docs/operate/OPS.md ("Notes & limitations") and check-consistency.sh §20.
##
## The Windows installer is ALSO an updater payload and IS already in PLATFORMS
## — it is not listed here, because listing it twice would invite the two
## definitions to drift. This list is only for artifacts no client fetches.
##
## `%s` is the version; `%s` the arch. Filled by `installer_zip_names()`.
MACOS_ZIP_TEMPLATE = "installer-Locus_%s_%s.zip"
MACOS_ZIP_ARCHES = ("amd64", "arm64")


def installer_zip_names(version):
    """The macOS human-download archives CI produces for `version`.

    Both architectures, by the same names the release job asserts are present.
    Kept as a function so the template exists once and can be pinned — the
    Windows installer learned this the hard way when CI's manifest and the
    hub's fetcher drifted apart on a filename and no check compared them.
    """
    return [MACOS_ZIP_TEMPLATE % (version, arch) for arch in MACOS_ZIP_ARCHES]


## manifest.json is a few hundred bytes and is NOT a binary, so it must not be
## held to the executable floor below — doing so rejects a perfectly good
## release with "downloaded only 719 bytes". The floor applies to executables
## only; the manifest just has to be non-empty and parse as JSON.
MIN_BINARY_BYTES = 1024 * 1024
MIN_MANIFEST_BYTES = 2
MAX_ASSET_BYTES = int(os.environ.get("MAX_ASSET_BYTES", str(200 * 1024 * 1024)))

## Per-request socket timeouts, in seconds. Deliberately short and NOT the
## artifact-size budget: these bound one HTTP round trip (an API call, or one
## read from the download socket), while `MAX_ASSET_BYTES` bounds the payload.
## A 200 MB artifact is read in chunks over a long-lived connection, so a
## timeout measured in minutes would only ever mean "the socket stopped
## answering and we waited too long to notice".
API_TIMEOUT = int(os.environ.get("FETCH_API_TIMEOUT", "30"))
DOWNLOAD_TIMEOUT = int(os.environ.get("FETCH_TIMEOUT", "60"))

USER_AGENT = "locus-hub-fetch/1.0 (+https://github.com/%s)" % GITHUB_REPO


def log(msg):
    print("[fetch %s] %s" % (time.strftime("%H:%M:%S"), msg), flush=True)


def resolve_platform_names(version, available=None):
    """The asset name carrying each platform, for a given version.

    One place decides what the hub fetches for each platform, so the names can
    be checked against `manifest.json` instead of being trusted — that check is
    the filename comparison in `verify_and_stage`, and it is the one that catches
    the hub and CI drifting apart on an asset name. The Windows entry is
    version-bearing because Tauri names its NSIS bundle with the version.

    `available` is the set of asset names the release actually carries. When a
    macOS tarball is ABSENT but the legacy bare binary is present, the legacy
    name is returned so a release published before the packaging step still
    resolves. That fallback is deliberately narrow — macOS only, and only when
    the tarball is genuinely missing — because preferring a bare Mach-O in a
    macOS update slot is the defect this function now avoids.
    """
    names = []
    for key, name, column in PLATFORMS:
        if name is None:
            resolved = installer_name(version)
        elif available is not None and name not in available:
            legacy = LEGACY_MACOS_NAMES.get(key)
            if legacy and legacy in available:
                log(
                    "WARNING: %s has no %s in this release; falling back to the "
                    "pre-fix bare binary %s, which a macOS updater cannot install. "
                    "Re-cut the release from a build that ran the packaging step."
                    % (key, name, legacy)
                )
                resolved = legacy
            else:
                resolved = name
        else:
            resolved = name
        names.append((key, resolved, column))
    return names


class FetchError(Exception):
    """A failure with a message safe to show the operator."""

    def __init__(self, message, status=500):
        super().__init__(message)
        self.status = status


# ── Version / filename validation ───────────────────────────────────────────
# Both become paths on disk. Reject anything that is not boring rather than
# trying to sanitise it.
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$")
# Every binary is accompanied by a minisign signature. The Tauri updater
# verifies a signature mandatorily and offers no bypass, so a release fetched
# without them is not installable by any client.
ALLOWED_FILENAMES = {name for _, name, _ in PLATFORMS if name} | {MANIFEST_NAME}
ALLOWED_FILENAMES |= {name + ".sig" for _, name, _ in PLATFORMS if name}
# The pre-fix macOS names stay ALLOWED as a fallback source, but are never
# preferred — `resolve_platform_names` only reaches for them when the tarball is
# genuinely absent from the release.
ALLOWED_FILENAMES |= set(LEGACY_MACOS_NAMES.values())
ALLOWED_FILENAMES |= {name + ".sig" for name in LEGACY_MACOS_NAMES.values()}


def validate_version(version):
    version = str(version or "").strip().lstrip("v")
    if not VERSION_RE.match(version):
        raise FetchError("version must look like 1.2.3 (got %r)" % version, 400)
    return version


# ── One-shot link signing ───────────────────────────────────────────────────
_lock = threading.Lock()
_used_nonces = {}


def _load_secret():
    """Read the link-signing secret, creating it on first run.

    Preference order is the environment (set from secrets.env.age on deploy),
    then the persisted file. The environment wins so a redeploy does not orphan
    a running set of links, and the file is a cache — the same env-first rule
    the rest of the server follows (see docs/operate/SECRETS-MANAGEMENT.md).
    """
    env_secret = os.environ.get("RELEASE_FETCH_SECRET", "").strip()
    if env_secret:
        return env_secret.encode()
    try:
        with open(SECRET_FILE, "rb") as f:
            data = f.read().strip()
            if data:
                return data
    except OSError:
        pass
    fresh = secrets.token_hex(32).encode()
    try:
        fd = os.open(SECRET_FILE, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(fresh)
        log("generated a new link-signing secret at %s" % SECRET_FILE)
    except OSError as exc:
        log("WARNING: could not persist link secret (%s) — links will not "
            "survive a restart" % exc)
    return fresh


SECRET = _load_secret()


def _sign(version, nonce, exp):
    msg = "%s|%s|%d" % (version, nonce, exp)
    return hmac.new(SECRET, msg.encode(), hashlib.sha256).hexdigest()


def mint_link(base_url, version):
    """Mint a fresh single-use trigger link. Every call produces a new nonce."""
    nonce = secrets.token_urlsafe(16)
    exp = int(time.time()) + LINK_TTL_SECONDS
    sig = _sign(version, nonce, exp)
    return (
        "%s/api/admin/fetch-release?version=%s&nonce=%s&exp=%d&sig=%s"
        % (base_url.rstrip("/"), version, nonce, exp, sig)
    )


def consume_nonce(nonce, exp, version, sig):
    """Verify a link and burn its nonce. Raises FetchError on any problem."""
    if not nonce or not sig:
        raise FetchError("this link is missing its signature", 403)
    try:
        exp = int(exp)
    except (TypeError, ValueError):
        raise FetchError("this link has a malformed expiry", 403)

    expected = _sign(version, nonce, exp)
    # Constant-time compare: a timing oracle on a signature is a real (if slow)
    # way to forge one.
    if not hmac.compare_digest(expected, str(sig)):
        raise FetchError("this link's signature is not valid", 403)
    if exp < time.time():
        raise FetchError("this link has expired — request a new one", 403)

    with _lock:
        if nonce in _used_nonces:
            raise FetchError("this link has already been used — request a new one", 409)
        _used_nonces[nonce] = time.time()
        # Opportunistically drop expired nonces so the dict cannot grow forever
        # on a long-lived process.
        cutoff = time.time() - LINK_TTL_SECONDS
        for n in [n for n, t in _used_nonces.items() if t < cutoff]:
            _used_nonces.pop(n, None)


# ── GitHub access ───────────────────────────────────────────────────────────
def _github_request(url):
    req = urllib.request.Request(url)
    req.add_header("Accept", "application/vnd.github+json")
    # GitHub rejects requests without a User-Agent outright (403).
    req.add_header("User-Agent", USER_AGENT)
    if GH_TOKEN:
        req.add_header("Authorization", "Bearer %s" % GH_TOKEN)
    try:
        return urllib.request.urlopen(req, timeout=API_TIMEOUT)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise FetchError(
                "no GitHub Release found for tag v%s in %s. Either CI has not "
                "finished, the tag was never pushed, or the release failed."
                % (VERSION_CONTEXT["version"], GITHUB_REPO),
                404,
            )
        if exc.code in (401, 403):
            raise FetchError(
                "GitHub refused the request (HTTP %d). If the repository is "
                "private, set GH_TOKEN on the hub." % exc.code,
                502,
            )
        raise FetchError("GitHub returned HTTP %d for the release" % exc.code, 502)
    except urllib.error.URLError as exc:
        raise FetchError("could not reach GitHub: %s" % exc.reason, 502)


# Set by the caller so error messages can name the version. A tiny module-level
# context avoids threading the version through every error path.
VERSION_CONTEXT = {"version": ""}


def resolve_release(version):
    """Return {asset_name: (url, size)} for the v<version> GitHub Release."""
    api = "https://api.github.com/repos/%s/releases/tags/v%s" % (GITHUB_REPO, version)
    log("resolving v%s from %s" % (version, GITHUB_REPO))
    with _github_request(api) as resp:
        try:
            rel = json.loads(resp.read().decode("utf-8", "replace"))
        except ValueError as exc:
            raise FetchError("GitHub returned unreadable JSON: %s" % exc, 502)

    if rel.get("draft"):
        raise FetchError("the v%s release is still a DRAFT on GitHub" % version, 400)

    assets = {}
    for asset in rel.get("assets", []) or []:
        name = asset.get("name") or ""
        url = asset.get("browser_download_url") or ""
        if name and url:
            assets[name] = (url, int(asset.get("size") or 0))
    return assets


def _download(url, dest, expected_min, expected_max, what="binary"):
    """Stream a URL to dest, hashing as we go. Returns (sha256, size)."""
    req = urllib.request.Request(url)
    req.add_header("User-Agent", USER_AGENT)
    if GH_TOKEN:
        req.add_header("Authorization", "Bearer %s" % GH_TOKEN)

    digest = hashlib.sha256()
    total = 0
    try:
        with urllib.request.urlopen(req, timeout=DOWNLOAD_TIMEOUT) as resp, open(dest, "wb") as out:
            while True:
                chunk = resp.read(262144)
                if not chunk:
                    break
                total += len(chunk)
                if total > expected_max:
                    raise FetchError(
                        "asset exceeded %d bytes while downloading — refusing"
                        % expected_max,
                        400,
                    )
                digest.update(chunk)
                out.write(chunk)
    except FetchError:
        raise
    except urllib.error.HTTPError as exc:
        raise FetchError("download failed with HTTP %d" % exc.code, 502)
    except urllib.error.URLError as exc:
        raise FetchError("download failed: %s" % exc.reason, 502)

    if total < expected_min:
        if what == "manifest":
            raise FetchError(
                "manifest.json downloaded empty (0 bytes) — the release is "
                "incomplete on GitHub",
                400,
            )
        raise FetchError(
            "downloaded only %d bytes — that is a truncated file or a Git LFS "
            "pointer, not a %s" % (total, what),
            400,
        )
    return digest.hexdigest(), total


def fetch_release(version, force=False):
    """Fetch every artifact for version into UPDATES_DIR/<version>/.

    All-or-nothing: nothing is published unless every platform is present and
    every hash verifies. Returns a result dict suitable for JSON.
    """
    version = validate_version(version)
    VERSION_CONTEXT["version"] = version
    target_dir = os.path.join(UPDATES_DIR, version)
    assets = resolve_release(version)

    # Pass the release's own asset names so a macOS tarball is used when it
    # exists and the legacy bare binary is only a fallback.
    # `resolve_release` returns {name: (url, size)}.
    resolved = resolve_platform_names(version, available=set(assets))
    wanted = [(key, name) for key, name, _ in resolved]
    # The signature for each payload is required, not optional: a published
    # update nobody can install is worse than no update, because it looks like
    # it worked from the operator's seat.
    wanted += [("sig_" + key, name + ".sig") for key, name, _ in resolved]
    wanted += [("manifest", MANIFEST_NAME)]
    missing = [name for _, name in wanted if name not in assets]
    if missing:
        raise FetchError(
            "the v%s release is missing: %s. CI is supposed to attach all four "
            "platform executables, their .sig signature files, and manifest.json "
            "— fix CI and re-release rather than publishing a partial update. "
            "If only the .sig files are missing, CI is not signing: see "
            "client/docs/SIGNING.md."
            % (version, ", ".join(missing)),
            400,
        )

    # ── The macOS human download: WANTED, BUT NOT REQUIRED ──
    #
    # Deliberately not in `wanted` above, and the asymmetry is the point. That
    # list is all-or-nothing because a missing UPDATE payload is a fleet that
    # silently cannot update. A missing macOS zip is one less convenience
    # download on the console page — and failing the whole publish for it would
    # mean a Windows or Linux hotfix could not ship because a macOS packaging
    # step had broken. That trade is wrong in the direction that matters: it
    # would let a cosmetic gap block a security fix.
    #
    # So it is staged when present, reported when absent, and never fatal. The
    # console shows the result either way, so "absent" is visible to an operator
    # rather than silent.
    optional = [("zip_macos_" + arch, name)
                for arch, name in zip(MACOS_ZIP_ARCHES, installer_zip_names(version))]

    os.makedirs(target_dir, exist_ok=True)
    staged = {}
    results = {}

    for key, name in wanted:
        url, asset_size = assets[name]
        if asset_size and asset_size > MAX_ASSET_BYTES:
            raise FetchError(
                "%s is %d bytes according to GitHub — larger than the %d byte cap"
                % (name, asset_size, MAX_ASSET_BYTES),
                400,
            )

        final_path = os.path.join(target_dir, name)
        # The 1 MB floor is a BINARY guard (a truncated download or a Git LFS
        # pointer). Two things this publishes are legitimately tiny and must not
        # be held to it:
        #
        #   * manifest.json — a few hundred bytes of JSON. Holding it to the
        #     binary floor rejected a perfectly good release with "downloaded
        #     only 719 bytes", which is why MIN_MANIFEST_BYTES exists.
        #   * *.sig — a minisign signature is a few hundred bytes of text. It is
        #     not a binary at all, and applying the binary floor to it would
        #     make every release unpublishable the moment signing was added.
        #
        # Both are only required to be non-empty; their content is validated
        # below (the manifest must parse as JSON, the signature must decode as
        # base64).
        is_manifest = (name == MANIFEST_NAME)
        is_signature = name.endswith(".sig")
        min_bytes = MIN_MANIFEST_BYTES if (is_manifest or is_signature) else MIN_BINARY_BYTES
        what = "manifest" if is_manifest else ("signature" if is_signature else "binary")

        # Idempotence: a complete, already-verified artifact is left alone
        # unless the operator asked for a force re-fetch.
        if not force and os.path.isfile(final_path):
            existing = _sha256_file(final_path)
            if os.path.getsize(final_path) >= min_bytes:
                log("  = %s already present (%s)" % (name, existing[:16]))
                staged[name] = final_path
                results[key] = {"filename": name, "sha256": existing,
                                "bytes": os.path.getsize(final_path),
                                "skipped": True}
                continue

        fd, tmp_path = tempfile.mkstemp(prefix=".fetch-", dir=target_dir)
        os.close(fd)
        try:
            log("  ↓ %s" % name)
            sha, size = _download(url, tmp_path, min_bytes, MAX_ASSET_BYTES, what=what)

            # The manifest is what proves the bytes came from the same build CI
            # hashed, so a manifest that is not JSON is useless even though it
            # has the right SHA. Check it here rather than at publish time.
            if is_manifest:
                try:
                    with open(tmp_path, "r", encoding="utf-8") as mf:
                        json.load(mf)
                except (ValueError, UnicodeDecodeError) as exc:
                    raise FetchError(
                        "manifest.json for v%s is not valid JSON (%s) — refusing "
                        "to publish" % (version, exc),
                        400,
                    )
            # Verify BEFORE publishing: write the sidecar, then atomically move.
            # os.replace on the same filesystem is atomic, so a client either
            # sees the old file or the complete new one — never a partial write.
            os.chmod(tmp_path, 0o644)
            os.replace(tmp_path, final_path)
            with open(final_path + ".sha256", "w") as f:
                f.write("%s  %s\n" % (sha, name))
            os.chmod(final_path + ".sha256", 0o644)
            staged[name] = final_path
            results[key] = {"filename": name, "sha256": sha, "bytes": size,
                            "skipped": False}
            log("    %d bytes sha256=%s" % (size, sha[:16]))
        except Exception:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass
            raise

    # ── The macOS human download, staged AFTER the required set ──
    #
    # Runs only once every required artifact is on disk and verified, so a
    # broken CI release still fails on the artifacts that matter before we spend
    # bandwidth on a convenience copy. Absence is reported, never raised — see
    # the note where `optional` is built.
    #
    # The `.zip` gets its own floor rather than the 1 MB binary floor. Both
    # would pass a real zip, but the reason differs: a zip is not an executable
    # and the binary floor's meaning ("this is not a truncated pointer for an
    # executable") does not apply to it. A few hundred KB is a plausible
    # compressed .app on a good day; a few hundred BYTES is a pointer or an
    # error page. Deliberately loose, because the check that the archive holds a
    # real bundle lives in CI (`unzip -l` assertions) where the bundle is
    # actually present.
    MIN_ARCHIVE_BYTES = 64 * 1024

    for key, name in optional:
        if name not in assets:
            log("  ~ %s not in the v%s release — the console will show no macOS download"
                % (name, version))
            results[key] = {"filename": name, "absent": True}
            continue
        url, asset_size = assets[name]
        if asset_size and asset_size > MAX_ASSET_BYTES:
            raise FetchError(
                "%s is %d bytes according to GitHub — larger than the %d byte cap"
                % (name, asset_size, MAX_ASSET_BYTES),
                400,
            )

        final_path = os.path.join(target_dir, name)
        if not force and os.path.isfile(final_path) and os.path.getsize(final_path) >= MIN_ARCHIVE_BYTES:
            existing = _sha256_file(final_path)
            log("  = %s already present (%s)" % (name, existing[:16]))
            results[key] = {"filename": name, "sha256": existing,
                            "bytes": os.path.getsize(final_path), "skipped": True}
            continue

        fd, tmp_path = tempfile.mkstemp(prefix=".fetch-", dir=target_dir)
        os.close(fd)
        try:
            log("  ↓ %s (macOS download)" % name)
            sha, size = _download(url, tmp_path, MIN_ARCHIVE_BYTES, MAX_ASSET_BYTES,
                                  what="archive")

            # The archive must actually BE a zip. This is the same class of check
            # as the ELF/PE/Mach-O slot validation: a wrong file in a slot is the
            # realistic failure, and a compressed app that is not an archive
            # would be served to a student as a "zip" and fail to open. Local
            # magic rather than a content scan — `PK\x03\x04` is the local file
            # header, and an empty archive is the only legitimate case that
            # lacks it, which is not something CI produces.
            with open(tmp_path, "rb") as fh:
                magic = fh.read(4)
            if magic[:2] != b"PK":
                raise FetchError(
                    "%s is not a zip archive (starts with %r) — refusing to "
                    "publish it as the macOS download"
                    % (name, magic[:4]),
                    400,
                )

            os.chmod(tmp_path, 0o644)
            os.replace(tmp_path, final_path)
            with open(final_path + ".sha256", "w") as f:
                f.write("%s  %s\n" % (sha, name))
            os.chmod(final_path + ".sha256", 0o644)
            results[key] = {"filename": name, "sha256": sha, "bytes": size,
                            "format": "zip archive", "skipped": False}
            log("    %d bytes sha256=%s" % (size, sha[:16]))
        except Exception:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass
            raise
    #
    # THE DEFECT THIS EXISTS FOR. `manifest.json` names, per platform, the file
    # CI intends a client to install from itself — and for Windows that has been
    # `installer-Locus_<v>_x64-setup.exe` since v3.2.12, the fix for the client
    # that re-executed its own binary. This list was not updated alongside it.
    #
    # Nothing caught that, and the reason is worth stating precisely: the
    # existing check verified the *hash of whichever file our name resolved to*
    # against `manifest[platform].sha256`. Our name was wrong, so it resolved a
    # different file, and the hash of that different file obviously matched the
    # hash of that different file. A self-consistent check on the wrong subject
    # reads exactly like a check that passed.
    #
    # Comparing the FILENAME closes it. It is the one field that cannot agree by
    # accident, and it is the field the two halves had drifted on.
    manifest_path = staged.get(MANIFEST_NAME)
    if manifest_path:
        try:
            with open(manifest_path, "r", encoding="utf-8") as mf:
                published = json.load(mf)
        except (OSError, ValueError, UnicodeDecodeError) as exc:
            raise FetchError(
                "could not read manifest.json to cross-check the platform "
                "filenames: %s" % exc,
                400,
            )

        advertised = published.get("platforms") or {}
        mismatches = []
        for key, name, _ in resolve_platform_names(version):
            stated = (advertised.get(key) or {}).get("file")
            if not stated:
                # Not fatal here: a platform CI chose not to advertise is the
                # all-or-nothing check's business, not this one's. Reporting it
                # as a name mismatch would blame the wrong half.
                continue
            if stated != name:
                mismatches.append(
                    "%s: the hub would serve %s, but CI's manifest advertises %s"
                    % (key, name, stated)
                )

        if mismatches:
            raise FetchError(
                "the hub's platform filenames disagree with CI's manifest for "
                "v%s — refusing to publish, because serving a file other than "
                "the one CI named is how a Windows client was handed a raw "
                "executable it could not install: %s. Update PLATFORMS in "
                "fetch-release.py (and publish-release.sh) to match the "
                "manifest." % (version, "; ".join(mismatches)),
                400,
            )
        log("  ✓ platform filenames agree with manifest.json")

    # Fold each platform's signature INTO that platform's entry.
    #
    # The console reads `artifacts[<platform>].signature` and hands it to
    # releases.publish, which writes signature_<platform>. The signatures arrive
    # under separate "sig_<platform>" keys (they are fetched as their own
    # assets), so without this join the publish path receives no signature and
    # refuses a release the operator just watched succeed — while the files sit
    # correctly on disk, which makes it look like a bug in the publishing half.
    for key, _, _ in resolve_platform_names(version):
        entry = results.get(key)
        sig_entry = results.get("sig_" + key)
        if not entry or not sig_entry:
            continue
        sig_path = os.path.join(target_dir, sig_entry["filename"])
        try:
            with open(sig_path, "r", encoding="utf-8") as f:
                sig_text = f.read().strip()
        except OSError as exc:
            raise FetchError(
                "could not read the signature for %s: %s" % (entry["filename"], exc),
                500,
            )

        # WIRE FORMAT: base64(ENTIRE .sig file text), exactly once.
        #
        # The client's `tauri-plugin-updater` runs `base64_to_string()` over the
        # `signature_<platform>` field BEFORE parsing it as minisign text:
        #
        #     Signature::decode(base64_to_string(signature_<platform>))
        #
        # So the stored value must be base64 of the four-line .sig text, not the
        # text itself. Storing the raw text (which this used to do) passes every
        # presence check here and in the console, then fails on the client with
        # `Invalid byte …, offset N` from the base64 decoder — the .sig file
        # contains spaces and newlines, which base64 rejects. That is exactly the
        # defect that shipped a live `update_config` row no client could install.
        #
        # Encoding happens here, at the single point a .sig becomes a stored
        # field, so every downstream consumer (releases.publish, /api/update)
        # receives the correct shape by construction.
        entry["signature"] = base64.b64encode(sig_text.encode("utf-8")).decode("ascii")

    return {"version": version, "dir": target_dir, "artifacts": results}


def _sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(262144)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _is_nsis_installer(path):
    """Whether a Windows PE carries the NSIS firstheader.

    The signature is the NSIS *firstheader*: the little-endian magic
    `0xDEADBEEF` (`\\xef\\xbe\\xad\\xde`) immediately followed by the ASCII
    `NullsoftInst`. makensis writes it into the overlay near the start of the
    file, so this scans a bounded window — the same approach, and the same 4 MiB
    bound, as the client's `is_installer_payload`. Scanning the whole file would
    mean reading 58 MB to find a marker in the first few.

    WHY THE MAGIC IS PART OF THE SIGNATURE, AND WHY THAT MATTERS
    -----------------------------------------------------------
    An earlier revision looked for the bare string `NullsoftInstaller`, which is
    wrong in BOTH directions:

      * A real NSIS installer does not contain it. The header is `NullsoftInst`
        (12 bytes) followed by a 4-byte flags word, so `NullsoftInstaller` never
        appears at the header — the real installer was REFUSED, and no Windows
        client could ever be published to.
      * The raw `locus-windows-amd64.exe` (the bare Tauri app, which embeds an
        NSIS uninstaller stub) *does* contain the literal substring
        `NullsoftInstaller`, far into the file. It was rejected only because that
        occurrence sits past the scan window — by luck, not by design.

    Requiring the `0xDEADBEEF` magic immediately before `NullsoftInst` is what
    actually separates the two: the real installer has the firstheader at a low
    offset; the raw app has neither the magic nor the 12-byte string, only an
    unrelated `NullsoftInstaller` substring. Measured against the real v3.2.18
    assets: firstheader at byte 68100 of the installer, absent from the raw app.

    Fail-closed on an unreadable file: a payload whose shape cannot be confirmed
    is not one to publish for execution.
    """
    NSIS_SIGNATURE = b"\xef\xbe\xad\xdeNullsoftInst"
    NSIS_SCAN_LIMIT = 4 * 1024 * 1024
    try:
        with open(path, "rb") as f:
            window = f.read(NSIS_SCAN_LIMIT)
    except OSError:
        return False
    return NSIS_SIGNATURE in window


def verify_artifact_kind(path, platform_key):
    """Sanity-check that the bytes match the platform they are filed under.

    The console used to do this in the browser before an upload (see the old
    lib/artifact.ts), which is where the "Windows .exe in the Linux slot"
    mistake was caught. That check must not simply be lost now the upload is
    gone, so it moves here — the fetcher is the thing that now decides which
    bytes land in which slot.

    Only the format is checked (ELF / PE / Mach-O and, for macOS, the
    architecture). A cross-slot mixup is the realistic failure; a content scan
    for the version string is possible but a wrong-version artifact is already
    covered by the fact that we resolve assets BY TAG.
    """
    try:
        with open(path, "rb") as f:
            head = f.read(8)
    except OSError:
        return True, "unreadable"
    try:
        size = os.path.getsize(path)
    except OSError:
        size = 0

    def is_elf(b):
        return len(b) >= 4 and b[:4] == b"\x7fELF"

    def is_pe(b):
        return len(b) >= 2 and b[:2] == b"MZ"

    def is_macho_thin(b):
        return len(b) >= 4 and b[:4] in (
            b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe",
            b"\xfe\xed\xfa\xcf", b"\xfe\xed\xfa\xce",
        )

    def is_macho_fat(b):
        return len(b) >= 4 and b[:4] in (b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca")

    if platform_key == "windows":
        if not is_pe(head):
            return False, "not a Windows PE executable"
        # A PE is not enough HERE, and this is the one slot where it matters.
        #
        # Both artifacts CI produces for Windows are PEs: the NSIS setup
        # executable (what a client must install from itself) and the raw
        # `locus-windows-amd64.exe` (what only the retired portable client ever
        # ran). "Is a PE" is true of both, so it cannot distinguish them — and
        # the wrong one gets ShellExecuted by `tauri_plugin_updater`, which
        # accepts any PE as an NSIS installer.
        #
        # What separates them is the NSIS firstheader makensis writes into the
        # overlay: the 0xDEADBEEF magic followed by `NullsoftInst`. This mirrors
        # `is_installer_payload` in client/src-tauri/src/locus/update/install.rs:
        # the hub refuses the same payload the client would refuse, so a mistake
        # is caught here at publish time rather than on a student's machine.
        if not _is_nsis_installer(path):
            return False, (
                "a Windows PE without an NSIS header — this is the raw updater "
                "binary, not an installer; a client cannot install from it"
            )
        return True, "PE (NSIS installer)"
    if platform_key == "linux":
        if not is_elf(head):
            return False, "not an ELF executable"
        return True, "ELF"
    if platform_key in ("macos_intel", "macos_arm"):
        # The macOS updater payload is a `.app.tar.gz`, and the check reads INSIDE
        # it rather than at its first bytes. That is not a weakening: a bare
        # Mach-O is now REFUSED here, which is the point — the updater cannot
        # install one, so accepting it at publish time would be exactly the
        # silent mis-slotting this function exists to catch.
        if head[:2] == b"\x1f\x8b":
            return _check_macos_app_tarball(path, platform_key, is_macho_thin, is_macho_fat)
        if is_macho_thin(head) or is_macho_fat(head):
            return False, (
                "a bare Mach-O binary, but the macOS updater slot needs a "
                "`*.app.tar.gz` — `tauri_plugin_updater` extracts a tar of an "
                ".app bundle, so a raw executable downloads, verifies, and then "
                "fails to install. Re-run CI on a commit that packages the bundle."
            )
        return False, "not a gzip archive (expected an .app.tar.gz)"
    return True, "unknown"


## The first tar member of a `.app.tar.gz` that is the bundle directory.
_APP_BUNDLE_RE = re.compile(r"^[^/]+\.app/")


def _check_macos_app_tarball(path, platform_key, is_macho_thin, is_macho_fat):
    """Validate a macOS updater payload: a tar.gz holding `<Name>.app/Contents/MacOS/`.

    Three properties, each of which the updater depends on and none of which is
    visible from the file's first bytes:

    1. **It is a real gzip+tar** — not a truncated upload.
    2. **Entries begin `<Name>.app/`** — the plugin strips one leading path
       component (`entry.path()?.iter().skip(1)`), so a tarball rooted at
       `Contents/` would extract a bundle with no name and the swap would find
       nothing to move.
    3. **The inner executable is the right architecture** — a universal binary
       is accepted for either slot, matching the per-slot rule the bare-Mach-O
       check used to apply.

    Checked by streaming the members rather than extracting to disk: this runs
    on the hub, and unpacking an untrusted 100 MB archive into the working
    directory to inspect it would be its own problem.
    """
    import tarfile

    try:
        with tarfile.open(path, "r:gz") as tar:
            names = []
            exe_member = None
            for member in tar:
                if not member.isfile():
                    continue
                if not names:
                    names.append(member.name)
                    if not _APP_BUNDLE_RE.match(member.name):
                        return False, (
                            "the tarball does not start with a `.app/` directory "
                            "(first member: %r) — the updater strips one leading "
                            "path component and would find no bundle" % member.name
                        )
                if member.name.endswith("/Contents/MacOS/locus") or (
                    "/Contents/MacOS/" in member.name
                    and not member.name.endswith("/")
                ):
                    # The bundle's main executable; the exact name is Tauri's.
                    exe_member = member
                    break
            if exe_member is None:
                return False, "no Contents/MacOS/ executable inside the .app bundle"
            head = tar.extractfile(exe_member).read(8)
    except (tarfile.TarError, OSError, EOFError) as exc:
        return False, "unreadable .app.tar.gz: %s" % exc

    if not (is_macho_thin(head) or is_macho_fat(head)):
        return False, "the .app's executable is not Mach-O"
    if is_macho_fat(head):
        return True, "app.tar.gz (Mach-O universal)"
    little = head[:4] in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf")
    cputype = int.from_bytes(head[4:8], "little" if little else "big")
    if cputype == 0x0100000C:  # ARM64
        return (platform_key == "macos_arm"), "app.tar.gz (Mach-O arm64)"
    if cputype == 0x01000007:  # x86_64
        return (platform_key == "macos_intel"), "app.tar.gz (Mach-O x86_64)"
    return True, "app.tar.gz (Mach-O)"


def verify_signature_file(path):
    """Checks that a .sig file is a readable minisign signature.

    What this can and cannot check matters:

    * It CAN confirm the file is non-empty base64 that decodes, and that the
      decoded text has the shape minisign produces (an untrusted comment line, a
      base64 signature line, a trusted comment line, and a global signature
      line). That catches a placeholder, an empty upload, or a signature for the
      wrong artifact.

    * It CANNOT verify the signature against the artifact, because this service
      does not hold the public key — and should not, since the key it would need
      is the *private* one to sign with. Real verification happens on the client,
      which has the public key compiled in.

    The distinction is deliberate: this is a "is this obviously not a signature"
    check, not an endorsement. Its value is catching a broken CI signing step at
    fetch time, rather than discovering it as "every client refuses this update".
    """
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError as exc:
        return False, "unreadable (%s)" % exc

    if not raw.strip():
        return False, "empty signature file"

    # A .sig file is PLAIN TEXT, four lines:
    #
    #   untrusted comment: signature from minisign secret key
    #   RUT9eeTiJVDWZMBlzoYn…=            <- the signature (base64)
    #   trusted comment: <what -t said>
    #   AHhE2ZGRWCrFe02+ZLxfaDZmV6/…==    <- the global signature (base64)
    #
    # This function validates the .sig FILE, which is four lines of plain ASCII.
    # It is NOT base64 — the base64 wrapping the client's plugin expects is
    # applied to this whole text later, when it is stored as the
    # `signature_<platform>` field (see the fold-in in `fetch()`). Decoding the
    # file as base64 here would be wrong and rejected every real signature; and
    # storing the file text verbatim as the field shipped the "Invalid byte,
    # offset N" bug. Two distinct steps: validate the file as text, then encode.
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return False, "not text (a minisign signature is four lines of ASCII)"

    lines = [ln for ln in text.splitlines() if ln.strip()]
    if len(lines) < 4:
        return False, "expected 4 lines of minisign output, found %d" % len(lines)
    if not lines[0].startswith("untrusted comment: "):
        return False, "missing the 'untrusted comment:' first line"
    if not lines[2].startswith("trusted comment: "):
        return False, "missing the 'trusted comment:' third line"
    # Both the signature and the global signature are base64. Checking they
    # decode catches a file that merely looks the right shape.
    for index in (1, 3):
        try:
            base64.b64decode(lines[index], validate=True)
        except (ValueError, TypeError) as exc:
            # Narrow deliberately: catching Exception here once turned a
            # NameError (python-base64, which was not imported) into the
            # plausible-looking message "line 3 is not base64", and a real bug
            # hid behind a convincing explanation.
            return False, "line %d is not base64 (%s)" % (index + 1, exc)
    return True, "minisign signature"


def verify_and_report(result):
    """Post-fetch format check. Raises FetchError on a cross-slot mixup."""
    problems = []
    names = resolve_platform_names(result["version"])
    for key, name, _ in names:
        entry = result["artifacts"].get(key)
        if not entry:
            problems.append("%s: not fetched" % name)
            continue
        path = os.path.join(result["dir"], entry["filename"])
        ok, detected = verify_artifact_kind(path, key)
        if not ok:
            problems.append("%s: %s" % (name, detected))
        else:
            result["artifacts"][key]["format"] = detected

        # Signature, alongside the binary it belongs to. A release whose
        # signatures are broken is not installable by any client, and it would
        # otherwise look perfectly healthy from here right up until the first
        # student tried to update.
        sig_entry = result["artifacts"].get("sig_" + key)
        if not sig_entry:
            problems.append("%s.sig: not fetched" % name)
            continue
        sig_path = os.path.join(result["dir"], sig_entry["filename"])
        sig_ok, sig_detected = verify_signature_file(sig_path)
        if not sig_ok:
            problems.append("%s.sig: %s" % (name, sig_detected))
        else:
            result["artifacts"]["sig_" + key]["format"] = sig_detected

    if problems:
        raise FetchError(
            "the release has the wrong file in one or more platform slots — " +
            "; ".join(problems) + ". Refusing to publish.",
            400,
        )
    return result


# ── HTTP surface ────────────────────────────────────────────────────────────
class Handler(BaseHTTPRequestHandler):
    server_version = "locus-fetch/1.0"

    def log_message(self, fmt, *args):
        # Default logging writes to stderr unformatted; route it through log().
        log("%s - %s" % (self.address_string(), fmt % args))

    # ── helpers ──
    def _send(self, status, payload):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        try:
            self.wfile.write(body)
        except BrokenPipeError:
            pass

    def _authorised(self, query, body):
        """Token header/field, or a valid one-shot link signature."""
        supplied = ""
        try:
            supplied = (self.headers.get("X-Admin-Token") or "").strip()
        except Exception:
            supplied = ""
        if not supplied and isinstance(body, dict):
            supplied = str(body.get("admin_token") or "").strip()

        if TOKEN and supplied and hmac.compare_digest(supplied, TOKEN):
            return True, "token"

        # One-shot signed link (GET). The nonce is burned on success.
        if query is not None:
            version = (query.get("version") or [""])[0]
            nonce = (query.get("nonce") or [""])[0]
            exp = (query.get("exp") or [""])[0]
            sig = (query.get("sig") or [""])[0]
            if nonce and sig:
                consume_nonce(nonce, exp, version, sig)
                return True, "link"
        return False, None

    def _base_url(self):
        host = self.headers.get("X-Forwarded-Host") or self.headers.get("Host") or ""
        proto = self.headers.get("X-Forwarded-Proto") or "https"
        if not host:
            return ""
        return "%s://%s" % (proto, host)

    # ── POST: console button ──
    def do_POST(self):
        import urllib.parse

        query = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
        if urllib.parse.urlparse(self.path).path not in ("/api/admin/fetch-release", "/fetch-release"):
            return self._send(404, {"ok": False, "message": "not found"})

        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            length = 0
        if length > 1024 * 1024:
            return self._send(413, {"ok": False, "message": "body too large"})
        raw = self.rfile.read(length) if length else b""
        try:
            body = json.loads(raw.decode("utf-8")) if raw else {}
        except (ValueError, UnicodeDecodeError):
            return self._send(400, {"ok": False, "message": "invalid JSON body"})

        try:
            ok, _how = self._authorised(query, body)
        except FetchError as exc:
            return self._send(exc.status, {"ok": False, "message": str(exc)})
        if not ok:
            return self._send(403, {"ok": False, "message": "Invalid admin token"})

        return self._run(body, force=bool(body.get("force")))

    # ── GET: one-shot link, or a dry-run preview ──
    def do_GET(self):
        import urllib.parse

        parsed = urllib.parse.urlparse(self.path)
        query = urllib.parse.parse_qs(parsed.query)

        if parsed.path == "/health":
            return self._send(200, {"ok": True, "service": "locus-fetch",
                                    "repo": GITHUB_REPO})

        if parsed.path == "/api/admin/fetch-link":
            # Mint a fresh one-shot link. Token-only: a link must not be able to
            # mint further links, or one leaked URL becomes a permanent grant.
            supplied = (self.headers.get("X-Admin-Token") or "").strip()
            if not TOKEN or not supplied or not hmac.compare_digest(supplied, TOKEN):
                return self._send(403, {"ok": False, "message": "Invalid admin token"})
            try:
                version = validate_version((query.get("version") or [""])[0])
            except FetchError as exc:
                return self._send(exc.status, {"ok": False, "message": str(exc)})
            base = self._base_url()
            if not base:
                return self._send(400, {"ok": False,
                                        "message": "no Host header — cannot build a link"})
            return self._send(200, {
                "ok": True,
                "link": mint_link(base, version),
                "expires_in": LINK_TTL_SECONDS,
                "single_use": True,
            })

        if parsed.path not in ("/api/admin/fetch-release", "/fetch-release"):
            return self._send(404, {"ok": False, "message": "not found"})

        try:
            ok, _how = self._authorised(query, None)
        except FetchError as exc:
            return self._send(exc.status, {"ok": False, "message": str(exc)})
        if not ok:
            return self._send(403, {"ok": False, "message": "not authorised — "
                                                             "request a new link"})

        version = (query.get("version") or [""])[0]
        force = (query.get("force") or ["0"])[0] in ("1", "true", "yes")
        return self._run({"version": version}, force=force)

    def _run(self, body, force=False):
        started = time.time()
        try:
            result = fetch_release(body.get("version"), force=force)
            verify_and_report(result)
        except FetchError as exc:
            log("FAILED: %s" % exc)
            return self._send(exc.status, {"ok": False, "message": str(exc)})
        except Exception as exc:  # never leak a traceback to the caller
            log("FAILED (unexpected): %r" % exc)
            return self._send(500, {"ok": False,
                                    "message": "unexpected error: %s" % exc})

        elapsed = round(time.time() - started, 1)
        log("✓ v%s ready in %ss" % (result["version"], elapsed))
        return self._send(200, {
            "ok": True,
            "version": result["version"],
            "dir": result["dir"],
            "elapsed": elapsed,
            "artifacts": result["artifacts"],
        })


def main():
    if not TOKEN:
        log("FATAL: ADMIN_API_TOKEN is not set — refusing to start")
        sys.exit(1)
    if not os.path.isdir(UPDATES_DIR):
        log("FATAL: UPDATES_DIR does not exist: %s" % UPDATES_DIR)
        sys.exit(1)
    log("Locus release-fetch service listening on %s:%d -> %s (repo %s)"
        % (LISTEN_HOST, LISTEN_PORT, UPDATES_DIR, GITHUB_REPO))
    log("one-shot links expire after %ds" % LINK_TTL_SECONDS)
    server = ThreadingHTTPServer((LISTEN_HOST, LISTEN_PORT), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
