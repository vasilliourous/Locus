#!/usr/bin/env bash
# Section 46 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_46() {
    echo "46. The minisign used to sign on macOS is native to BOTH slices"
    # ─────────────────────────────────────────────────────────────
    # WHY THIS EXISTS
    #
    # 2026-11-08, the first release that ran on a real Intel macOS machine (v3.3.7,
    # run 42432058733): `Build (macos-arm)` passed and `Build (macos-intel)` died in
    #
    #   Install minisign   .../minisign: Bad CPU type in executable
    #
    # The step downloaded `minisign-0.12-macos.zip`, which contains exactly one
    # executable and it is **arm64**. That was invisible for a simple reason: until
    # the slices were split onto per-architecture images, BOTH ran on the arm64
    # machine, so the arm64 binary was always the right one. The arm slice passing on
    # that very run is why this needs its own check — one slice's green is not
    # evidence about the other's CPU.
    #
    # WHAT IT ASSERTS
    #
    #   (a) On macOS, minisign is not installed from a prebuilt release archive.
    #       `minisign-<v>-macos.zip` is a single-architecture artifact; there is no
    #       universal one to fall back to, and the arm64 one is what upstream ships.
    #   (b) The macOS branch BUILDS it from the pinned source tarball instead, so it
    #       targets whatever CPU the job was scheduled on.
    #   (c) The built tool is RUN and its version checked. A build that produced
    #       nothing, or the wrong arch, then fails inside that step instead of at
    #       signing — after the whole compile.
    #
    # WHAT IT CANNOT DO: it does not prove the macOS image carries `cmake`,
    # `pkg-config` and `libsodium`, nor that the tarball's layout is unchanged. The
    # step fails loudly on all three — and it fails at the START of the job, in
    # seconds, rather than minutes later at signing.
    # ─────────────────────────────────────────────────────────────
    if [ ! -f "$WORKFLOW" ]; then
        bad "workflow not found at ${WORKFLOW}; cannot check the macOS minisign install"
    else
        # The step's body, from its name line to the next `- name:` at the same
        # indentation. The naive range form closed on a comment line at the same
        # indentation and silently produced an EMPTY branch — which would have made
        # every assertion below vacuous. An empty branch is therefore a BAD, not a
        # skip: a check that cannot fail reads exactly like one that passed.
        MINISIGN_STEP="$(awk '
            /^      - name: Install minisign$/ { grab = 1 }
            grab && /^      - name: / && !/Install minisign/ { exit }
            grab { print }
        ' "$WORKFLOW")"
        if [ -z "$MINISIGN_STEP" ]; then
            bad "could not find the 'Install minisign' step in the workflow"
        else
            # `index()` rather than a regex: the line carries `"${{ runner.os }}"`,
            # and a brace-heavy regex here is a trap that matches nothing — which
            # is how this section first passed vacuously.
            macos_branch="$(printf '%s\n' "$MINISIGN_STEP" | awk '
                !grab && index($0, "runner.os }}") && index($0, "\"macOS\"") { grab = 1 }
                grab && /^          else$/ { exit }
                grab { print }
            ')"
            if [ -z "$macos_branch" ]; then
                bad "could not find the macOS branch of 'Install minisign'"
                bad "  An empty branch would make every assertion below vacuous."
            else
                if printf '%s' "$macos_branch" | grep -q 'minisign-[0-9.]*-macos\.zip'; then
                    bad "the macOS minisign is installed from the prebuilt release zip"
                    bad "  upstream publishes that zip as a SINGLE architecture (arm64 as"
                    bad "  of 0.12), so the Intel slice gets 'Bad CPU type in executable'"
                    bad "  after the compile. Build it from the pinned source tarball."
                else
                    ok "the macOS minisign is not taken from the prebuilt (single-arch) zip"
                fi

                if printf '%s' "$macos_branch" | grep -q 'minisign-0\.12\.tar\.gz'; then
                    ok "the macOS minisign is built from the pinned source tarball"
                else
                    bad "the macOS branch does not fetch the pinned minisign SOURCE tarball"
                    bad "  Without it there is nothing architecture-native to install."
                fi

                if printf '%s' "$macos_branch" | grep -qE '"\$DEST/minisign" -v'; then
                    ok "the built minisign is executed and its version checked, not assumed"
                else
                    bad "the macOS branch never RUNS the minisign it installed"
                    bad "  'install -m 755' succeeds for a binary this CPU cannot execute,"
                    bad "  so an unrun install cannot tell a working tool from a dead one."
                fi
            fi
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
