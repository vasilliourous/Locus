#!/usr/bin/env bash
# Section 39 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_39() {
    echo "39. The downloaded build inputs match their pinned hashes"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `client/scripts/prebuild.mjs` fetches the mihomo engine and the geo databases
    # before the Rust build can start. It resolves the engine version by fetching
    # `version.txt` from GitHub's `releases/latest` AT BUILD TIME, and the only
    # validation on what arrives is a gzip/zip MAGIC-BYTE check.
    #
    # Three consequences, all real:
    #
    #   * Two builds of the same commit can ship different engines, so a defect
    #     cannot be bisected by commit — the engine is not a function of the commit.
    #   * A corrupted or substituted archive that is still a valid archive is
    #     accepted. TLS is the only integrity control.
    #   * The pinned bytes lived in a gitignored cache (`node_modules/.verge/` plus
    #     `src-tauri/sidecar/`). A fresh clone has neither, so "what did we ship" was
    #     answerable only from a machine that still had the cache — the same class of
    #     loss as the gitignored age key.
    #
    # `server/data/sidecar.lock.json` now records the sha256 of every sidecar and
    # geo database that was actually used, plus the engine versions read from the
    # binaries themselves. This section fails when the files on disk stop matching.
    #
    # ── WHAT THIS CHECKS, AND THE HONEST LIMIT ──
    #
    # Files present but differing from the lock → BAD. Files pinned but absent →
    # reported as a warning, not a failure, because a checkout that has not run
    # prebuild is a normal state (CI always runs it; a docs-only clone does not).
    #
    # As of 2026-10-29 the sidecars are also COMMITTED under `vendor/build-inputs/`,
    # and the checker verifies the committed copy when `src-tauri/sidecar/` is
    # absent. That closes the "a fresh clone reports every pin as MISSING" false
    # finding §39 used to emit, and it means the bytes a build ships are the bytes in
    # the repository rather than whatever a download produced that morning.
    #
    # The four geo databases are now mirrored too, as of 2026-11-03: they live in
    # `vendor/build-inputs/resources/` and are committed, because their 2026-10-28
    # asset IDs 404'd and `resources` spent a week declared vendored and empty. The
    # paragraph here said "still fetched ... declared vendored and empty" for two
    # weeks after that changed, which is the same class of stale sentence this script
    # exists to catch — and the reason the check below exists rather than a comment.
    # docs/operate/VENDORED-BUILD-INPUTS.md §4 owns the story.
    # ─────────────────────────────────────────────────────────────
    sidecar_report=$(python3 "$SCRIPTS/sidecar-lock.py" --check 2>&1)
    sidecar_status=$?
    case "$sidecar_status" in
        0)
            ok "every downloaded build input matches its pinned sha256"
            ;;
        2)
            # Not an error: nothing has been fetched in this checkout.
            warn "no downloaded build inputs present — run prebuild to verify them against the lock"
            ;;
        *)
            while IFS= read -r line; do
                case "$line" in
                    BAD*) bad "build inputs: ${line#BAD	}" ;;
                    MISSING*) warn "build inputs: ${line#MISSING	}" ;;
                    *) [ -n "$line" ] && bad "build inputs: $line" ;;
                esac
            done <<EOF
$sidecar_report
EOF
            ;;
    esac

    echo
    echo "════════════════════════════════════════════════════════════"
}
