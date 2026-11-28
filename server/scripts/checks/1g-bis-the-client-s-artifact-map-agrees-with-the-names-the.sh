#!/usr/bin/env bash
# Section 1g-bis -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_1g_bis() {
    echo
    echo "1g-bis. The client's artifact map agrees with the names the hub serves"
    # ─────────────────────────────────────────────────────────────
    # 1g-bis. `artifact_for` (client) vs `NEW_PLATFORMS` (hub).
    #
    # The two sides silently disagreed until 2026-11-25: the client named the bare
    # `chrysalis-darwin-*` Mach-O for macOS while the hub served the `.app.tar.gz`.
    # A bare Mach-O is downloadable, verifiable and UNINSTALLABLE — `install.rs`
    # refuses it, and §24 refuses to publish it — so a future caller of
    # `artifact_for` would have built a URL for a file no client can apply.
    #
    # It was invisible because `artifact_for` has no production caller: the client is
    # handed `update_url` by the hub. That is what makes it a *latent* trap rather
    # than a live bug, and the reason it is still checked — the next person to use it
    # will not know that.
    # `|| true`: the python below exits non-zero on its own sentinel paths (NORUST /
    # NOHUB), and this script runs under `set -u` with an errexit-sensitive style —
    # an assignment that fails must still leave a defined variable for the `case`.
    _ch_check=$(cd "$REPO" && python3 - <<'PY3'
import re
def read(p):
    try:
        return open(p, encoding="utf-8", errors="replace").read()
    except OSError:
        return ""
contract = read("client/src-tauri/src/chrysalis/contract.rs")
rust = dict(re.findall(r'b"([a-z_]+)" => Some\("([^"]+)"\)', contract))
if len(rust) != 4:
    print("NORUST"); raise SystemExit
fetch_src = read("server/scripts/fetch-release.py")
block = re.search(r"^NEW_PLATFORMS = \[(.*?)^\]", fetch_src, re.S | re.M)
if not block:
    print("NOHUB"); raise SystemExit
hub = dict(re.findall(r'\("([a-z_]+)",\s*"([^"]+)"', block.group(1)))
if not hub:
    print("NOHUB"); raise SystemExit
bad = ["%s: client=%s hub=%s" % (k, rust.get(k), v) for k, v in hub.items() if rust.get(k) != v]
print("OK" if not bad else "DIFFER " + " | ".join(bad))
PY3
    ) || _ch_check="PYFAILED"
    case "$_ch_check" in
        OK)     ok "contract.rs artifact_for matches NEW_PLATFORMS in fetch-release.py on every platform" ;;
        NORUST) bad "1g-bis: could not read the four artifact_for arms from contract.rs" ;;
        NOHUB)  bad "1g-bis: could not read NEW_PLATFORMS from fetch-release.py" ;;
        PYFAILED) bad "1g-bis: the comparison could not run (python3 failed)" ;;
        *)      bad "contract.rs and the hub disagree about an artifact name:"
                printf '%s\n' "${_ch_check#DIFFER }" | tr '|' '\n' | sed 's/^ */         /'
                bad "  a name only one side knows is a URL no client can fetch, or a payload"
                bad "  the plugin refuses; see the macOS case in NAME-MIGRATION.md §3.1" ;;
    esac
}
