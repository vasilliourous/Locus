#!/usr/bin/env bash
# Section 1 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_1() {
    # ─────────────────────────────────────────────────────────────
    # 1. The platform key list must be identical everywhere it appears.
    #
    # It is duplicated in six hooks (PocketBase loads each *.pb.js separately, so
    # they cannot share a module) plus Rust and Python. A platform missing from any
    # ONE of these means that platform silently stops being offered an update.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "1. Platform keys"

    EXPECTED='["linux","windows","macos_intel","macos_arm"]'
    EXPECTED_SPACED='["linux", "windows", "macos_intel", "macos_arm"]'

    # Every literal list in the hooks, normalised (strip spaces) for comparison.
    hook_lists=$(grep -rhoE '\["linux"[^]]*\]' "$HOOKS"/*.js 2>/dev/null | tr -d ' ' | sort -u)

    if [ -z "$hook_lists" ]; then
        bad "no platform-key list found in $HOOKS — has the layout changed?"
    elif [ "$(printf '%s\n' "$hook_lists" | wc -l)" -gt 1 ]; then
        bad "the hooks disagree with each other about the platform list:"
        printf '%s\n' "$hook_lists" | sed 's/^/         /'
    else
        found=$(printf '%s' "$hook_lists")
        if [ "$found" = "$EXPECTED" ]; then
            n=$(grep -rlE '\["linux"[^]]*\]' "$HOOKS"/*.js 2>/dev/null | wc -l)
            ok "all hook platform lists identical across ${n} file(s)"
        else
            bad "hook platform list is '$found', expected '$EXPECTED'"
        fi
    fi

    # Python: the signature columns and the release fetch must cover the same set.
    for p in linux windows macos_intel macos_arm; do
        grep -q "\"$p\"" "$SCRIPTS/fetch-release.py" 2>/dev/null \
            || bad "fetch-release.py does not mention platform '$p'"
    done

    # ─────────────────────────────────────────────────────────────
    # 1a. THE HUB MUST FETCH THE INSTALLER FOR WINDOWS, NOT THE RAW BINARY.
    #
    # This is the check whose absence shipped a broken Windows update. The rule it
    # guards is the same one §15 guards for the workflow's manifest, but on the
    # OTHER side of the contract — and the two sides drifted apart, which is what
    # made the bug possible: CI correctly advertised
    # `installer-Locus_<v>_x64-setup.exe` in manifest.json from v3.2.12 onward,
    # while fetch-release.py and publish-release.sh still resolved
    # `locus-windows-amd64.exe`. The hub therefore staged the raw PE into the
    # Windows slot, hashed it, paired it with the RAW BINARY's own minisign
    # signature, and served all three. Every Windows client refused it — correctly,
    # via `is_installer_payload` — so no Windows client could update at all.
    #
    # The old assertion here ("fetch-release.py mentions 'windows'") passed before,
    # during and after that bug: the string was present the whole time. A check that
    # cannot tell a right answer from a wrong one is not a check, so this one pins
    # the FILENAME.
    if grep -qE 'installer-Locus_%s_x64-setup\.exe|INSTALLER_NAME_TEMPLATE' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        ok "fetch-release.py resolves the Windows installer by name"
    else
        bad "fetch-release.py does not resolve 'installer-Locus_<version>_x64-setup.exe'"
        bad "  the hub would serve the raw 'locus-windows-amd64.exe' to Windows clients,"
        bad "  which the client's is_installer_payload refuses — no Windows client could update"
    fi

    # The stale name must be GONE. Checked separately from the positive above so
    # that keeping both (e.g. a fallback) cannot slip through: a fallback to the raw
    # binary is the bug, not a safety net.
    if grep -qE '"windows"[^,]*chrysalis-windows-amd64\.exe|chrysalis-windows-amd64\.exe"' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        bad "fetch-release.py still names 'locus-windows-amd64.exe' as a fetchable asset"
        bad "  that is the raw updater payload; the Windows slot must be the NSIS installer"
    else
        ok "fetch-release.py no longer names the raw Windows binary"
    fi

    # publish-release.sh writes the same update_config row from the CLI path, so it
    # must agree. Two writers with two opinions is how the row and the manifest came
    # to disagree in the first place.
    if grep -qE 'WINDOWS_INSTALLER="installer-Chrysalis_\$\{VERSION\}_x64-setup\.exe"' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
        ok "publish-release.sh resolves the Windows installer by name"
    else
        bad "publish-release.sh does not resolve the Windows installer by name"
        bad "  the CLI publish path would write download_windows pointing at the raw binary"
    fi

    # ── §1e: BOTH installer templates exist, in both writers ──
    #
    # The pair is the fleet contract from NAME-MIGRATION.md §3.1: the current name is
    # what CI builds, the legacy name is what a pre-move release carries. Keeping both
    # means the hub can serve either generation; dropping the legacy one makes every
    # parked release un-fetchable; dropping the current one means the release CI just
    # built cannot be published at all. Asserted as a PAIR so neither half can be
    # "tidied" on its own — that is the failure this whole section exists for.
    for _pair in "fetch-release.py:INSTALLER_NAME_TEMPLATE:Chrysalis" \
                 "fetch-release.py:LEGACY_INSTALLER_NAME_TEMPLATE:Locus" \
                 "publish-release.sh:WINDOWS_INSTALLER:Chrysalis" \
                 "publish-release.sh:WINDOWS_INSTALLER_LEGACY:Locus"; do
        _file="${_pair%%:*}"; _rest="${_pair#*:}"; _var="${_rest%%:*}"; _want="${_rest#*:}"
        if grep -qE "${_var}.*installer-${_want}_" "$SCRIPTS/$_file" 2>/dev/null; then
            ok "$_file names the ${_want} installer template ($_var)"
        else
            bad "$_file has no ${_want} installer template ($_var)"
            bad "  fetch-release.py and publish-release.sh must each know BOTH name sets:"
            bad "  Chrysalis is what CI builds now, Locus is what a pre-move release carries."
            bad "  Half the pair = a release that either cannot publish or cannot be fetched."
            bad "  See docs/reference/NAME-MIGRATION.md §3.1."
        fi
    done

    if grep -qE '"windows:chrysalis-windows-amd64\.exe:update_windows"' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
        bad "publish-release.sh still publishes the raw Windows binary as the update"
    else
        ok "publish-release.sh no longer publishes the raw Windows binary"
    fi

    # `productName` must still be the one the three resolvers above agree on.
    #
    # Tauri derives the bundle filenames from `productName`, so this ONE string
    # decides whether the file the hub fetches exists: `productName` ->
    # `<productName>_<v>_x64-setup.exe` -> CI prefixes it to
    # `installer-<productName>_<v>_x64-setup.exe`. The rename changed it to
    # `Chrysalis` while fetch-release.py, publish-release.sh and the checks above
    # kept resolving `installer-Locus_*`, so v3.5.0 built green, signed all 21
    # assets, and then could not be published: `the v3.5.0 release is missing:
    # installer-Locus_3.5.0_x64-setup.exe`. Nothing failed until the hub fetch,
    # because every other check looked at a side that was internally consistent.
    #
    # See docs/reference/NAME-MIGRATION.md §3.1 — this is a FILENAME, not branding.
    _installer_product="$(python3 - "$REPO/client/src-tauri/tauri.conf.json" <<'PY' 2>/dev/null || true
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("productName") or "")
except Exception:
    print("")
PY
    )"
    #
    # AS OF 2026-11-25 THIS ASSERTS AGREEMENT, NOT A LITERAL. The previous version
    # hardcoded `Locus` as the only acceptable answer, which meant the guard's own
    # correctness depended on nobody ever completing the rename — the guard would have
    # gone red on the commit that finished the job it documents. What matters is that
    # `productName` equals the name the *current* template embeds, whichever spelling
    # that is; the pair check above guarantees the legacy spelling also resolves.
    if [ -z "$_installer_product" ]; then
        bad "could not read productName from client/src-tauri/tauri.conf.json"
    else
        _tpl_product="$(grep -oE 'INSTALLER_NAME_TEMPLATE = "installer-[A-Za-z]+_' "$SCRIPTS/fetch-release.py" 2>/dev/null \
            | head -1 | sed -E 's/.*installer-([A-Za-z]+)_/\1/')"
        if [ -z "$_tpl_product" ]; then
            bad "could not read the product name out of INSTALLER_NAME_TEMPLATE in fetch-release.py"
        elif [ "$_installer_product" = "$_tpl_product" ]; then
            ok "productName (${_installer_product}) matches the installer name the hub resolves"
        else
            bad "productName is '${_installer_product}' but the hub resolves 'installer-${_tpl_product}_<v>_x64-setup.exe'"
            bad "  Tauri names the bundle from productName, so the release publishes"
            bad "  'installer-${_installer_product}_<v>_x64-setup.exe' and fetch-release.py"
            bad "  resolves 'installer-${_tpl_product}_<v>_x64-setup.exe' instead: the fetch"
            bad "  reports the release as missing its Windows installer — at publish time,"
            bad "  after a full four-platform build, which is how v3.5.0 failed."
            bad "  see docs/reference/NAME-MIGRATION.md §3.1"
        fi
    fi

    # Rust: the constants, and the artifact mapping for each.
    for pair in "PLATFORM_LINUX:linux" "PLATFORM_WINDOWS:windows" \
                "PLATFORM_MACOS_INTEL:macos_intel" "PLATFORM_MACOS_ARM:macos_arm"; do
        const="${pair%%:*}"; value="${pair#*:}"
        if grep -qE "^pub const ${const}: &str = \"${value}\";" "$RUST/contract.rs" 2>/dev/null; then
            ok "contract.rs ${const} = \"${value}\""
        else
            bad "contract.rs ${const} is not \"${value}\""
        fi
    done
}
