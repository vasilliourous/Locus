#!/usr/bin/env bash
# Section 17 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_17() {
    echo "17. Every version site is written by the bump script"

    # WHY THIS EXISTS
    #
    # A release is cut by `cd client && pnpm release-version X`, which rewrites every
    # file that carries the version. On 2026-09-30 a `v3.2.13` tag was pushed on a
    # commit whose files all still said `3.2.12`; CI (which labels the release from
    # the tag name and never bumps) published 3.2.12 binaries under a 3.2.13 release
    # and the hub offered clients an "update" that reinstalled the same version
    # forever. The recovery attempt then bumped only FOUR files, because
    # `docs/state.toml`'s `[client.version]` is not a manifest and was easy to miss —
    # and this very script's §7 caught it with `BAD client.version`.
    #
    # §7 catches the value being WRONG. This catches a site being MISSING from the
    # bump, which §7 cannot see: if a fifth manifest appears and nobody adds it to
    # the script, the script keeps succeeding and publishes a bug in the field.
    #
    # It does not guess. It asserts the version in each known manifest is a string
    # the script can and does rewrite — i.e. each site is named in release-version.mjs
    # — so a new site fails here, at the cheap gate, naming itself.
    BUMP_SCRIPT="$REPO/client/scripts/release-version.mjs"
    if [ ! -f "$BUMP_SCRIPT" ]; then
        bad "client/scripts/release-version.mjs is missing — no supported way to bump"
    else
        # Each entry: a manifest path, and the exact CALL the bump script must make
        # to rewrite that file. The marker is the invocation, not the function name:
        # a name also appears in the definition and the docstring, so grepping for it
        # would pass even with the call deleted — which is precisely the failure this
        # guard exists to catch. (Verified: with the call removed and the function
        # left defined, a name-based marker reported OK.)
        #
        # The misses are accumulated in a FILE, not a variable: the loop reads its
        # list from a heredoc, and a construct that runs the loop in a subshell (as
        # some shells do) would silently discard a variable set inside it — a guard
        # that reports OK no matter what. A file survives either way.
        bump_gaps=$(mktemp)
        bump_checked=0
        while IFS='|' read -r site_path script_marker label; do
            [ -n "$site_path" ] || continue
            bump_checked=$((bump_checked + 1))
            if [ ! -f "$REPO/$site_path" ]; then
                # Record it as a gap too, so the success branch below cannot also
                # fire. Without this the check printed BAD *and* "all N ... OK" for
                # the same run — a self-contradicting result.
                echo "         $label — the file $site_path does not exist" >> "$bump_gaps"
                continue
            fi
            if ! grep -qF "$script_marker" "$BUMP_SCRIPT"; then
                echo "         $label ($site_path)" >> "$bump_gaps"
            fi
        done <<'SITES'
client/package.json|await updatePackageVersion(|package.json
client/src-tauri/Cargo.toml|await updateCargoVersion(|Cargo.toml
client/src-tauri/tauri.conf.json|await updateTauriConfigVersion(|tauri.conf.json
client/Cargo.lock|await updateCargoLockVersion(|Cargo.lock
docs/state.toml|await updateStateTomlVersion(|docs/state.toml
SITES

        if [ -s "$bump_gaps" ]; then
            bad "these version sites are not covered by the bump script:"
            while IFS= read -r line; do bad "$line"; done < "$bump_gaps"
            bad "A release would leave them stale, publishing a mislabelled build."
            bad "Add the file to client/scripts/release-version.mjs (its"
            bad "updateStateTomlVersion shows the pattern) and to §17's SITES list."
        else
            ok "all $bump_checked version site(s) are written by client/scripts/release-version.mjs"
        fi
        rm -f "$bump_gaps"
    fi
}
