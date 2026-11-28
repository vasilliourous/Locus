#!/usr/bin/env bash
# Section 15 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_15() {
    # ─────────────────────────────────────────────────────────────
    # 15. No platform's manifest entry may name the raw updater binary.
    #
    # WHY THIS EXISTS
    #
    # `v3.2.9` and `v3.2.10` both published `manifest.json` with
    #
    #     "windows": { "file": "locus-windows-amd64.exe", ... }
    #
    # and that is the RAW PE EXECUTABLE, not an installer. The client downloads the
    # entry named for its platform and hands the bytes to `tauri_plugin_updater`,
    # which accepts *any* PE as an NSIS installer (`extract_exe` asks only
    # `infer::app::is_exe(bytes)`) and ShellExecutes it.
    #
    # So the update did not install anything: it relaunched a copy of the client's
    # own binary, standalone, outside the install directory, and exited. With no
    # bundled web assets beside it, Tauri resolved the start page to a `file://`
    # path that does not exist and Chromium displayed, inside a Locus window:
    #
    #     File not found
    #     It may have been moved, edited, or deleted.
    #     ERR_FILE_NOT_FOUND
    #
    # Nothing named Locus. The install directory was never touched, so reinstalling
    # and restarting both changed nothing — and it recurred on every release, because
    # the client accepts whatever the manifest names as newest.
    #
    # WHY A GUARD AND NOT JUST A FIX
    #
    # The fix is a line or two in the workflow. What made this bug expensive is that
    # `locus-windows-amd64.exe` is a legitimate asset with a legitimate consumer —
    # the retired portable client — so "stage the binary" and "advertise it for
    # self-install" look identical in review, and this manifest was read several
    # times without anyone noticing. This check names the distinction.
    #
    # The two halves are deliberately independent:
    #   - the manifest must not advertise a raw updater payload for self-install;
    #   - the client must refuse one even if a manifest does (see
    #     `is_installer_payload` in `client/src-tauri/src/chrysalis/update/install.rs`).
    # Either alone would have prevented the bug; both together mean a future
    # regression has to defeat two mechanisms in two different languages.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "15. No manifest platform entry advertises a raw updater binary for self-install"

    # The name CI stages as the Windows updater payload. It is what the hub serves
    # to a client that is going to EXECUTE the bytes, so it must be an installer.
    if grep -qE '"windows"[[:space:]]*:[[:space:]]*"chrysalis-windows-amd64\.exe"' "$WORKFLOW" 2>/dev/null; then
        bad "the workflow's manifest platform map advertises 'locus-windows-amd64.exe' for windows"
        bad "  that is the raw updater executable, which a client cannot install from itself:"
        bad "  tauri_plugin_updater treats any PE as an NSIS installer and would ShellExecute it"
        bad "  advertise the NSIS setup executable (installer-Locus_*_x64-setup.exe) instead"
    else
        ok "the manifest does not advertise the raw Windows binary for self-install"
    fi

    # The positive half: the Windows entry must name a setup executable. Without
    # this, deleting the platform map entirely would pass the check above.
    if grep -qE 'installer-Locus_\*_x64-setup\.exe|windows_installer\(\)' "$WORKFLOW" 2>/dev/null; then
        ok "the Windows platform entry resolves to an installer"
    else
        bad "the workflow does not appear to advertise an installer for windows"
        bad "  the windows platform key must resolve to installer-Locus_*_x64-setup.exe"
    fi

    # The client-side half must still exist, or the guard above is decorative.
    #
    # This checks the CALL SITE, not the symbol. `grep -q 'is_installer_payload'`
    # over the whole file is a check that cannot fail: the function's definition and
    # its fifteen unit tests all contain the name, so deleting the call from
    # `ReadyInstall::install` — the one line that actually protects a student's
    # machine — leaves eighteen matches behind and the check still prints OK.
    # Demonstrated 2026-10-02: with `if true /* was: if !is_installer_payload(&bytes) */`
    # in install(), this section passed. The pattern below is anchored to the
    # negated call over the downloaded bytes, so it fails when the call goes away.
    INSTALL_GUARD="$REPO/client/src-tauri/src/chrysalis/update/install.rs"
    if [ ! -f "$INSTALL_GUARD" ]; then
        bad "client/src-tauri/src/chrysalis/update/install.rs is missing — the payload guard cannot be checked"
    elif grep -qE 'if[[:space:]]*![[:space:]]*is_installer_payload\(&bytes\)' "$INSTALL_GUARD"; then
        ok "the client refuses to execute a payload that is not an installer"
    else
        bad "the client no longer calls is_installer_payload on the downloaded bytes"
        bad "  without it, any manifest edit can hand a raw executable to ShellExecute"
    fi

    # The constant itself is the other half of the two-sided contract: the hub's
    # fetch-release.py and the client must agree on the same bytes, or a payload is
    # publishable and not installable (or worse, installable and not publishable).
    # This is the 2026-10-02 defect — both sides looked for `NullsoftInstaller`,
    # which no real installer contains.
    # Single-quoted, so the backslashes are literal: the Rust/Python source carries
    # the escape sequence `\xef\xbe\xad\xde` as TEXT in the file, and that text is
    # what must agree. Reading the actual bytes here instead would check the compiled
    # meaning, not the source, and this script only ever greps source.
    NSIS_SIGNATURE='\xef\xbe\xad\xdeNullsoftInst'
    if grep -qF "b\"$NSIS_SIGNATURE\"" "$INSTALL_GUARD"; then
        ok "the client looks for the NSIS firstheader (0xDEADBEEF + NullsoftInst)"
    else
        bad "the client's NSIS signature is not the firstheader magic + NullsoftInst"
        bad "  a bare 'NullsoftInstaller' string is not in any real installer: every"
        bad "  genuine Windows update is refused before it is executed"
    fi

    FETCH_SERVICE="$REPO/server/scripts/fetch-release.py"
    if [ ! -f "$FETCH_SERVICE" ]; then
        bad "server/scripts/fetch-release.py is missing — the hub's payload check cannot be verified"
    elif grep -qF "b\"$NSIS_SIGNATURE\"" "$FETCH_SERVICE"; then
        ok "the hub looks for the same NSIS firstheader as the client"
    else
        bad "the hub and the client disagree on the NSIS signature"
        bad "  the hub would publish a payload the client refuses (or the reverse)"
    fi

    echo
    echo "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
}
