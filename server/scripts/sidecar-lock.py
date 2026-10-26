#!/usr/bin/env python3
"""Lock the build inputs that `prebuild.mjs` downloads, and verify them.

WHY THIS EXISTS

`client/scripts/prebuild.mjs` fetches five things before the Rust build can start:
the mihomo core, the alpha mihomo core, the privilege-service bundle, and four
geo databases. It resolves the mihomo version by fetching `version.txt` from
GitHub's `releases/latest` at build time and downloading whatever that points at.

The consequences, none of them theoretical:

  * **Two builds of the same commit can ship different engines.** "Latest" today
    is not "latest" next Tuesday. A defect cannot be bisected by commit, because
    the engine is not a function of the commit.
  * **Nothing is integrity-checked.** `downloadFile()` validates the gzip and zip
    *magic bytes* and nothing else. A corrupted or substituted archive that is
    still a valid archive is accepted. The only thing standing between a
    compromised `github.com` path and every installed client is TLS.
  * **The pinned bytes live in a gitignored cache.** `node_modules/.verge/` holds
    the resolved versions, and `src-tauri/sidecar/` holds the binaries. A fresh
    clone has neither, so "what did we actually ship" is answerable only from a
    machine that still has the cache — the same class of loss as the gitignored
    age key that started this.

`data/sidecar.lock.json` records, per sidecar and per geo database, the URL it
came from, the version, and the sha256 of the bytes that were actually used. It
is committed. From then on a build either reproduces those exact bytes or fails
and says which one changed.

MODES

  --write    Record the sidecars currently on disk, and the versions resolved in
             the local cache, into the lock. Run this ONLY when you intend to
             move to a new engine version; the diff is the review artifact.
  --check    Verify the files on disk match the lock. Exit 1 on any mismatch,
             and exit 2 if nothing has been downloaded yet (which is a different
             situation from a mismatch and must not read the same).
  --explain  Print the lock in readable form.

WHAT THIS DOES NOT DO

It cannot verify bytes it has not seen. `--check` compares disk against the lock;
the lock is only as trustworthy as the run that produced it. It also does not
close the build-time network dependency — resolving a *pinned* version still
fetches from GitHub. It makes that fetch checkable, not absent. A fully offline
build needs the archives vendored, which is a separate decision about repository
size and is recorded in docs/reference/STILL-OPEN.md.
"""

import hashlib
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LOCK = REPO / "server" / "data" / "sidecar.lock.json"
SIDECAR_DIR = REPO / "client" / "src-tauri" / "sidecar"
RESOURCES_DIR = REPO / "client" / "src-tauri" / "resources"
VERSION_CACHE = REPO / "client" / "node_modules" / ".verge" / ".version_cache.json"

# The five binaries plus the four databases, keyed by the filename prebuild.mjs
# writes. Kept as a list rather than a glob so that a NEW sidecar appearing in
# the directory is an additions report, not silently absorbed.
SIDECARS = [
    "verge-mihomo",
    "verge-mihomo-alpha",
    "clash-verge-service",
    "clash-verge-service-install",
    "clash-verge-service-uninstall",
]
RESOURCES = ["Country.mmdb", "ASN.mmdb", "geosite.dat", "geoip.dat"]


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def cached_versions():
    try:
        return json.loads(VERSION_CACHE.read_text(encoding="utf-8"))
    except OSError:
        return {}
    except json.JSONDecodeError:
        return {}


def find_sidecars():
    """Map each logical sidecar to the file on disk, for whatever triples exist.

    The filenames carry a target triple suffix (`verge-mihomo-x86_64-unknown-
    linux-gnu`), and a developer may have fetched more than one triple. The lock
    records each triple it finds, so one lock covers every platform somebody has
    actually built.
    """
    found = {}
    if not SIDECAR_DIR.is_dir():
        return found
    for path in sorted(SIDECAR_DIR.iterdir()):
        if not path.is_file():
            continue
        name = path.name
        for base in SIDECARS:
            # Longest base first: `clash-verge-service-install` must not be
            # matched by `clash-verge-service`.
            pass
        for base in sorted(SIDECARS, key=len, reverse=True):
            prefix = base + "-"
            if name.startswith(prefix):
                triple = name[len(prefix):]
                found.setdefault(base, {})[triple] = path
                break
    return found


def runtime_version(path):
    """Ask the mihomo binary what version it is.

    More trustworthy than the gitignored cache, and it works on a fresh clone:
    the binary carries its own version string (`mihomo -v` prints
    `Mihomo Meta v1.19.32 ...`). Falls back to None on any failure rather than
    guessing, because a guessed version in a lock is worse than an absent one.
    """
    import re
    import subprocess

    try:
        out = subprocess.run(
            [str(path), "-v"], capture_output=True, text=True, timeout=10
        )
    except (OSError, subprocess.SubprocessError):
        return None
    text = (out.stdout or "") + (out.stderr or "")
    m = re.search(r"Mihomo Meta\s+(\S+)", text)
    return m.group(1) if m else None


# Annotations that are DECISIONS, not facts about the bytes on disk. They cannot
# be re-derived, so `--write` must carry them over from the existing lock.
# Forgetting one is not hypothetical: the first --write after vendoring the alpha
# dropped `vendored: true`, and prebuild would have gone back to downloading an
# artifact whose URL no longer exists.
PRESERVED_TOP_LEVEL = ("vendored", "unavailable")


def preserved(existing):
    if not existing:
        return {}
    out = {}
    for key in PRESERVED_TOP_LEVEL:
        if key in existing:
            out[key] = existing[key]
    # Per-sidecar `vendored` flags, keyed by file, survive as well.
    flags = {}
    for by_triple in (existing.get("sidecars") or {}).values():
        for entry in by_triple.values():
            if entry.get("vendored"):
                flags[entry["file"]] = True
    if flags:
        out["_vendored_files"] = flags
    return out


def collect():
    previous = None
    try:
        previous = json.loads(LOCK.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        previous = None
    keep = preserved(previous)
    vendored_files = keep.pop("_vendored_files", {})

    lock = {
        "version": 1,
        "what": (
            "The build inputs prebuild.mjs downloads, pinned by sha256 of the "
            "bytes actually used. See server/scripts/sidecar-lock.py."
        ),
        "source": "client/scripts/prebuild.mjs",
        "engine_versions": {
            "mihomo": None,
            "mihomo_alpha": None,
        },
        "sidecars": {},
        "resources": {},
    }

    found = find_sidecars()
    for base, by_triple in found.items():
        lock["sidecars"][base] = {}
        for triple, path in by_triple.items():
            entry = {
                "file": path.name,
                "sha256": sha256(path),
                "bytes": path.stat().st_size,
            }
            if vendored_files.get(path.name):
                entry["vendored"] = True
            lock["sidecars"][base][triple] = entry

    # The engine version, read from the binaries themselves. This is what makes
    # the lock legible ("we shipped v1.19.32") rather than only a hash nobody
    # can interpret. It is ASSERTED against the binary, not against the cache:
    # the cache is gitignored and may not exist.
    for base, key in (("verge-mihomo", "mihomo"), ("verge-mihomo-alpha", "mihomo_alpha")):
        for triple, path in found.get(base, {}).items():
            v = runtime_version(path)
            if v:
                lock["engine_versions"][key] = v
            break  # one triple is enough; the version is not per-platform

    # Where the version came from, so a reader knows how much to trust it.
    lock["engine_versions_provenance"] = (
        "read from the local binaries with `-v`; the gitignored prebuild cache "
        "is consulted only as a cross-check and is not required"
    )

    if RESOURCES_DIR.is_dir():
        for name in RESOURCES:
            path = RESOURCES_DIR / name
            if path.is_file():
                lock["resources"][name] = {
                    "sha256": sha256(path),
                    "bytes": path.stat().st_size,
                }

    lock.update(keep)
    return lock


def check(lock):
    """Compare disk against the lock. Returns (lines, exit_code)."""
    out = []
    versions = cached_versions()

    for key, want in (
        ("mihomo", lock.get("engine_versions", {}).get("mihomo")),
        ("mihomo_alpha", lock.get("engine_versions", {}).get("mihomo_alpha")),
    ):
        if not want:
            continue
        got = {
            "mihomo": versions.get("META_VERSION", {}).get("version"),
            "mihomo_alpha": versions.get("META_ALPHA_VERSION", {}).get("version"),
        }[key]
        if got and got != want:
            out.append(
                "BAD\t%s: the build resolved %r but the lock pins %r — the engine "
                "changed without the lock being updated" % (key, got, want)
            )

    # Some artifacts are pinned but KNOWN not to exist upstream any more. That is
    # a different state from "you have not run prebuild": one is recoverable by
    # running a command, the other needs a decision. Reporting them the same way
    # sends the reader to a command that cannot work.
    unavailable = lock.get("unavailable", {})

    present = 0
    for base, by_triple in lock.get("sidecars", {}).items():
        for triple, want in by_triple.items():
            path = SIDECAR_DIR / want["file"]
            if not path.is_file():
                if base in unavailable:
                    info = unavailable[base]
                    out.append(
                        "UNAVAILABLE\t%s is absent and CANNOT be re-downloaded: %s "
                        "(last seen %s). Not a build error here — see %s"
                        % (
                            want["file"],
                            info.get("reason", "known unavailable"),
                            info.get("last_seen_version", "?"),
                            info.get("see", "docs/reference/STILL-OPEN.md"),
                        )
                    )
                else:
                    out.append(
                        "MISSING\t%s is pinned in the lock but not on disk — run "
                        "prebuild for %s" % (want["file"], triple)
                    )
                continue
            present += 1
            got = sha256(path)
            if got != want["sha256"]:
                out.append(
                    "BAD\t%s hashes %s but the lock pins %s — the build would "
                    "ship an engine nobody reviewed" % (want["file"], got[:16], want["sha256"][:16])
                )

    for name, want in lock.get("resources", {}).items():
        path = RESOURCES_DIR / name
        if not path.is_file():
            out.append("MISSING\t%s is pinned in the lock but not on disk" % name)
            continue
        present += 1
        got = sha256(path)
        if got != want["sha256"]:
            out.append(
                "BAD\t%s hashes %s but the lock pins %s"
                % (name, got[:16], want["sha256"][:16])
            )

    if present == 0:
        # Nothing downloaded at all. This is NOT the same as a mismatch, and
        # conflating them would make a fresh clone look like a corrupted build.
        return out, 2

    # UNAVAILABLE is reported but does not fail: the file cannot be obtained by
    # running a command, so failing here would make every build red for a
    # reason no build step can fix. BAD and MISSING both do fail — those are
    # actionable.
    return out, (1 if any(l.startswith(("BAD", "MISSING")) for l in out) else 0)


def main(argv):
    mode = "--check"
    for arg in argv[1:]:
        if arg in ("--write", "--check", "--explain"):
            mode = arg
        elif arg in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            print("sidecar-lock: unknown argument %r" % arg)
            return 2

    if mode == "--write":
        lock = collect()
        if not lock["sidecars"] and not lock["resources"]:
            print("sidecar-lock: nothing on disk to record — run prebuild first")
            return 1
        LOCK.parent.mkdir(parents=True, exist_ok=True)
        LOCK.write_text(json.dumps(lock, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print("wrote %s" % LOCK.relative_to(REPO))
        return 0

    if mode == "--explain":
        print(json.dumps(collect(), indent=2, sort_keys=True))
        return 0

    try:
        lock = json.loads(LOCK.read_text(encoding="utf-8"))
    except OSError:
        print("BAD\tthe sidecar lock is missing: %s" % LOCK.relative_to(REPO))
        print("BAD\t  run: python3 server/scripts/sidecar-lock.py --write")
        return 1

    lines, code = check(lock)
    for line in lines:
        print(line)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
