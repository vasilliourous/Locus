#!/usr/bin/env bash
# smoke-macos-payload-resolution.sh — prove the hub resolves a macOS UPDATER
# slot to an installable `.app.tar.gz`, and REFUSES rather than silently
# falling back to a bare Mach-O.
#
# WHY THIS EXISTS
#
# `tauri_plugin_updater` consumes a tar of an `.app` bundle on macOS. A bare
# Mach-O in a macOS update slot therefore downloads, verifies, and installs
# nothing. That is not hypothetical: v3.2.26 was published to the live hub with
# `macos_arm` pointing at `locus-darwin-arm64`, and a student on 3.2.24 was
# offered it and got:
#
#     the update to 3.2.26 is not an installer (48150583 bytes); refusing to
#     execute it
#
# The client was right. The hub had served a payload its installer can never
# apply, and nothing failed at publish time, because `resolve_platform_names`
# fell back to the bare binary, logged a WARNING, and carried on. A broken row
# looks healthy from the operator's seat; a refusal does not.
#
# WHAT THIS CHECKS
#
# Two behaviours of the SHIPPED `resolve_platform_names`, exercised directly:
#
#   1. A release WITH the tarball resolves every macOS slot to `.app.tar.gz`.
#   2. A release with ONLY the pre-fix bare Mach-O is REFUSED (FetchError), and
#      the refusal names the platform and the file — so "no updates, and nothing
#      says why" cannot recur.
#
# It imports the real module rather than reimplementing the rule, so it cannot
# pass against a copy that has drifted from what ships (the same reason
# smoke-update-endpoint.sh extracts its logic from the hook).
#
# Usage: server/scripts/smoke-macos-payload-resolution.sh
#
# Exit codes: 0 all assertions held, 1 an assertion failed, 2 setup problem.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FETCH="${REPO_ROOT}/server/scripts/fetch-release.py"

[ -f "$FETCH" ] || { echo "smoke-macos: no such file: $FETCH" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || { echo "smoke-macos: python3 is required" >&2; exit 2; }

# `fetch-release.py` reads a link-secret path under /root on first import and
# computes a listen port, but imports cleanly without a server. Run it from a
# writable secret file so import is side-effect-free on a dev box.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "smoke-macos: exercising resolve_platform_names from $(basename "$FETCH")"

FETCH_SECRET_FILE="$WORK/link_secret" python3 - "$FETCH" <<'PY'
import importlib.util
import sys

path = sys.argv[1]
spec = importlib.util.spec_from_file_location("fr_under_test", path)
fr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fr)

passed = 0
failed = 0


def check(desc, cond):
    global passed, failed
    if cond:
        passed += 1
        return
    failed += 1
    print("  \033[0;31mFAIL\033[0m %s" % desc)


# ── The two asset sets, both real ──
#
# PRE_FIX is v3.2.26 as it actually exists on GitHub: every platform present,
# macOS only as the bare Mach-O. FIXED adds the packaged bundle CI produces
# from 3.2.27 onward, with its own signature.
PRE_FIX = {
    "locus-linux-amd64", "locus-linux-amd64.sig",
    "locus-windows-amd64.exe", "locus-windows-amd64.exe.sig",
    "installer-Locus_3.2.26_x64-setup.exe", "installer-Locus_3.2.26_x64-setup.exe.sig",
    "locus-darwin-amd64", "locus-darwin-amd64.sig",
    "locus-darwin-arm64", "locus-darwin-arm64.sig",
    "manifest.json",
}
FIXED = set(PRE_FIX) | {
    "locus-darwin-amd64.app.tar.gz", "locus-darwin-amd64.app.tar.gz.sig",
    "locus-darwin-arm64.app.tar.gz", "locus-darwin-arm64.app.tar.gz.sig",
}


def resolved_for(version, assets):
    return {k: n for k, n, _ in fr.resolve_platform_names(version, available=assets)}


# 1. A release that HAS the tarball resolves macOS to it. This is the happy
#    path, and asserting it keeps the refusal below from being "refuse always".
r = resolved_for("3.2.28", FIXED)
check("macos_intel resolves to the .app.tar.gz when present",
      r.get("macos_intel") == "locus-darwin-amd64.app.tar.gz")
check("macos_arm resolves to the .app.tar.gz when present",
      r.get("macos_arm") == "locus-darwin-arm64.app.tar.gz")
check("linux still resolves to the bare ELF",
      r.get("linux") == "locus-linux-amd64")
check("windows resolves to the NSIS installer",
      r.get("windows") == "installer-Locus_3.2.28_x64-setup.exe")

# 2. A release with ONLY the bare Mach-O is REFUSED — not downgraded. This is
#    the exact defect that reached the live hub as v3.2.26.
try:
    fr.resolve_platform_names("3.2.26", available=PRE_FIX)
    check("a macOS slot with no .app.tar.gz is refused (not silently downgraded)", False)
    print("  \033[0;31mFAIL\033[0m resolve_platform_names returned a bare Mach-O instead of raising")
except fr.FetchError as exc:
    msg = str(exc)
    check("the refusal is a FetchError", True)
    check("the refusal names the missing tarball", "app.tar.gz" in msg)
    check("the refusal names the pre-fix bare binary it refused to serve",
          "locus-darwin" in msg)
    check("the refusal tells the operator how to proceed",
          "new release" in msg.lower() or "cut a new release" in msg.lower())
    print("  refusal message: %s" % (msg.splitlines()[0][:120]))
except SystemExit as exc:
    # importlib exec can surface a module-level SystemExit; treat distinctly.
    check("resolve_platform_names raised FetchError (not SystemExit(%r))" % exc, False)

# 3. With NO release context, the preferred names are returned unchanged. The
#    cross-check and other callers rely on this; a refusal here would break
#    callers that only want to know what the hub PREFERS.
p = {k: n for k, n, _ in fr.resolve_platform_names("3.2.26")}
check("preferred names (no available=) are the .app.tar.gz forms",
      p.get("macos_arm") == "locus-darwin-arm64.app.tar.gz")

print("  %d assertion(s) passed" % passed)
sys.exit(0 if failed == 0 else 1)
PY
rc=$?

echo
if [ "$rc" -eq 0 ]; then
    printf '\033[0;32mA macOS update slot resolves to an installable payload, and a pre-fix release is refused.\033[0m\n'
else
    printf '\033[0;31mA macOS update slot could be served a payload no client can install.\033[0m\n' >&2
fi
exit "$rc"
