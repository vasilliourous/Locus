#!/usr/bin/env bash
# Section 11 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_11() {
    # ─────────────────────────────────────────────────────────────
    # 11. The licence identifier agrees across the three places it is written.
    #
    # WHY THIS EXISTS
    #
    # The root LICENSE is CC BY-NC-ND 4.0 with an explicit scope carve-out: `client/`
    # is NOT covered by it, because client/ is a fork of Clash Verge Rev and a GPL
    # fork cannot be relicensed. That carve-out is a legal statement, and it is
    # duplicated as a machine-readable SPDX id in two manifests. Nothing bound them.
    #
    # It drifted immediately. The commit that ADDED the carve-out also stamped
    # `"license": "CC-BY-NC-ND-4.0"` onto client/package.json in the same change —
    # asserting the exact thing the carve-out excludes — and left
    # client/src-tauri/Cargo.toml at the earlier `UNLICENSED`. Three files, three
    # answers, no mechanism. This is the same defect class as §1 and §4, on the one
    # field where being wrong is a licensing violation rather than a silent no-op.
    #
    # WHAT IT ENFORCES: the `client/` row of the root LICENSE scope table is the
    # authority. Both client manifests must carry that exact SPDX id.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "11. Licence identifier — root carve-out vs client manifests"

    LICENSE_FILE="$REPO/LICENSE"
    PKG_JSON="$REPO/client/package.json"
    CARGO_TOML="$REPO/client/src-tauri/Cargo.toml"

    if [ ! -f "$LICENSE_FILE" ]; then
        bad "LICENSE is missing — there is no authority for the carve-out"
    else
        # The scope table row is:  | `client/` | **GPL-3.0-only** | ... |
        # Read it rather than hardcoding, so changing the licence is a one-file edit
        # and this check follows. A strikethrough/italic marker is tolerated.
        EXPECTED_LIC=$(grep -E '^\|[[:space:]]*`client/`' "$LICENSE_FILE" \
            | head -1 \
            | awk -F'|' '{print $3}' \
            | sed -e 's/\*//g' -e 's/`//g' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')

        if [ -z "$EXPECTED_LIC" ]; then
            bad "LICENSE has no \`client/\` row in its scope table — the carve-out is gone"
            bad "expected a line like: | \`client/\` | **GPL-3.0-only** | ... |"
        else
            # package.json — read the JSON rather than grepping, so a reformat does
            # not defeat it. Parse failure is itself the finding.
            PKG_LIC=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("license",""))' "$PKG_JSON" 2>/dev/null)
            if [ -z "$PKG_LIC" ]; then
                bad "could not read a license field from client/package.json"
            elif [ "$PKG_LIC" = "$EXPECTED_LIC" ]; then
                ok "client/package.json — '${PKG_LIC}' agrees with the root carve-out"
            else
                bad "client/package.json says '${PKG_LIC}' but the root LICENSE carves client/ out as '${EXPECTED_LIC}'"
            fi

            # Cargo.toml — `license = "..."` in [package]. Only the first such line.
            CARGO_LIC=$(grep -m1 -E '^license[[:space:]]*=' "$CARGO_TOML" \
                | sed -e 's/^license[[:space:]]*=[[:space:]]*//' -e 's/^"//' -e 's/"$//')
            if [ -z "$CARGO_LIC" ]; then
                bad "could not read a license field from client/src-tauri/Cargo.toml"
            elif [ "$CARGO_LIC" = "$EXPECTED_LIC" ]; then
                ok "client/src-tauri/Cargo.toml — '${CARGO_LIC}' agrees with the root carve-out"
            else
                bad "client/src-tauri/Cargo.toml says '${CARGO_LIC}' but the root LICENSE carves client/ out as '${EXPECTED_LIC}'"
            fi

            # client/LICENSE must actually BE that licence — a fork whose bundled
            # text disagrees with its declared SPDX id is the same defect one level
            # down, and that file is the one a downstream reader opens.
            if [ -f "$REPO/client/LICENSE" ]; then
                case "$EXPECTED_LIC" in
                    GPL-3.0-only|GPL-3.0)
                        if grep -q 'GNU GENERAL PUBLIC LICENSE' "$REPO/client/LICENSE" \
                           && grep -q 'Version 3' "$REPO/client/LICENSE"; then
                            ok "client/LICENSE contains the GPL-3 text it is declared to be"
                        else
                            bad "client/LICENSE is not the GPL-3 text, but is declared '${EXPECTED_LIC}'"
                        fi ;;
                    *)
                        # Other licences are not text-matched here; only the two
                        # manifests are enforced. Reported so the gap is visible.
                        warn "client/LICENSE not text-checked against '${EXPECTED_LIC}' (only GPL-3 is)" ;;
                esac
            else
                bad "client/LICENSE is missing — the carve-out points at a file that is not there"
            fi
        fi
    fi
}
