#!/usr/bin/env bash
# Section 12 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_12() {
    # 12. The update install path must not reintroduce the buffering downloader.
    #
    # WHY THIS EXISTS
    #
    # The client's updater crashed on "Install Now": the progress spinner stayed at
    # 0% and the app died with nothing in the log. The cause was that the install
    # path used the Tauri plugin's `download_and_install`, which reads the ENTIRE
    # artifact into a `Vec<u8>` in memory before verifying it — enough to abort the
    # allocation on a school laptop, killing the process.
    #
    # The streaming, disk-hashing downloader that avoids this (`chrysalis/update/apply.rs`)
    # already existed and was DEAD CODE. The two modules were written from
    # incompatible plans and only the unsafe one got wired up. Nothing noticed,
    # because nothing checked.
    #
    # This guard is what notices. It fails the build if the live install path calls
    # the plugin's buffering downloader again, or if the streaming downloader loses
    # its only production caller.
    echo
    echo "12. Update install path uses the streaming downloader"

    INSTALL_RS="$REPO/client/src-tauri/src/chrysalis/update/install.rs"
    APPLY_RS="$REPO/client/src-tauri/src/chrysalis/update/apply.rs"
    CMD_RS="$REPO/client/src-tauri/src/cmd/chrysalis.rs"

    if [ ! -f "$INSTALL_RS" ] || [ ! -f "$APPLY_RS" ] || [ ! -f "$CMD_RS" ]; then
        bad "the update path files are missing — this guard cannot run"
    else
        # (a) The buffering call must not be present in the install module at all.
        #     Matched on the call, not the name, so a doc comment explaining why it
        #     is avoided does not trip it.
        if grep -qE '\.download_and_install[[:space:]]*\(' "$INSTALL_RS" "$CMD_RS"; then
            bad "the install path calls the plugin's download_and_install, which buffers the whole artifact in memory"
            bad "  use PendingInstall::download_to_file + ReadyInstall::install instead (see client/src-tauri/src/chrysalis/update/install.rs)"
        else
            ok "no call to the memory-buffering download_and_install in the install path"
        fi

        # (b) The streaming downloader must have a production caller. `pub mod apply`
        #     plus a re-export is not a caller — that is exactly the dead-code state
        #     that caused this bug.
        if grep -qE 'apply::download[[:space:]]*\(' "$INSTALL_RS"; then
            ok "the streaming downloader apply::download is called by the install path"
        else
            bad "apply::download is not called by the install path — the streaming downloader is dead code again"
            bad "  this is the state that produced the 0%-then-crash updater bug"
        fi

        # (c) The install command must log BEFORE it can block. The original bug was
        #     undiagnosable precisely because the path was silent until it failed.
        if grep -q 'update install requested' "$CMD_RS"; then
            ok "the install command logs at the start of the attempt"
        else
            bad "chrysalis_install_update has no start-of-attempt log line — a crash mid-download would again leave nothing behind"
        fi

        # (d) The hub's checksum must be read and enforced. Installing without a
        #     hash is installing unverified bytes.
        if grep -qE 'sha256_from_manifest|\.sha256\(\)' "$INSTALL_RS" \
           && grep -q 'CHRYSALIS_UPDATE_NO_CHECKSUM' "$CMD_RS"; then
            ok "the install path reads the hub checksum and refuses when it is absent"
        else
            bad "the install path does not enforce the hub's sha256 — an unverified install would be possible"
        fi
    fi
}
