#!/usr/bin/env bash
# Section 37 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_37() {
    echo "37. The wire contract still matches the frozen snapshot"

    # ── WHY THIS GUARD EXISTS ──
    #
    # Every defect class in docs/reference/FIXES.md is a name that crossed a
    # boundary with nothing enforcing agreement: `uot_port` / `server_port_uot`,
    # `download_*` / `update_*`, `bound_this_device` / `already_used`, the
    # `macos_*` platform key against the `darwin-*` filename.
    #
    # Sections 33 and 34 catch two of those boundaries — the status struct against
    # its TypeScript interface, and the IPC command set across three sites. Neither
    # looks at the HUB boundary, which is the one an installed client cannot
    # renegotiate: a deployed 3.3.x build sends `device_fingerprint` and
    # `server_config`, reads `server.config`-shaped responses, and calls
    # `/api/heartbeat`. Nothing in this repository recorded that set.
    #
    # docs/reference/contract.lock.json now records it, and this section is what
    # makes the record binding rather than documentation. A field removed from a
    # wire struct, an endpoint a client still calls and the hub no longer serves,
    # a platform artifact renamed, a proxy name made equal to its group name — each
    # fails here, by name, before a release can be built.
    #
    # ── WHAT THIS CHECKS ──
    #
    # Runs contract-snapshot.py --check (see that file's header for the full rule).
    # A name the lock holds that the tree has lost is a BREAK and fails the build.
    # A name the tree has that the lock does not hold is ADDITIVE and is reported:
    # it does not fail, because adding a wire field is sometimes correct, but it
    # must be acknowledged with `--write` so the change appears in a reviewable
    # diff instead of happening silently.
    # ─────────────────────────────────────────────────────────────
    snapshot_report=$(python3 "$SCRIPTS/contract-snapshot.py" --check 2>&1)
    snapshot_status=$?
    if [ "$snapshot_status" -eq 0 ]; then
        ok "no frozen wire name was removed or respelled"
        if printf '%s\n' "$snapshot_report" | grep -q '^NEW'; then
            while IFS= read -r line; do
                case "$line" in
                    NEW*) warn "undocumented addition: ${line#NEW	}" ;;
                esac
            done <<EOF
$snapshot_report
EOF
            warn "acknowledge additive contract changes with: python3 server/scripts/contract-snapshot.py --write"
        fi
    elif [ "$snapshot_status" -eq 1 ]; then
        while IFS= read -r line; do
            case "$line" in
                BAD*) bad "contract: ${line#BAD	}" ;;
                NEW*) warn "undocumented addition: ${line#NEW	}" ;;
                *) [ -n "$line" ] && bad "contract: $line" ;;
            esac
        done <<EOF
$snapshot_report
EOF
    else
        bad "the contract snapshot checker could not run (exit $snapshot_status) — see its output above"
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
