#!/usr/bin/env bash
# Section 16 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_16() {
    echo "16. No absolute frontendDist is compiled into the client"

    # WHY THIS EXISTS
    #
    # `frontendDist` is not only a build-time input. `tauri::generate_context!()`
    # resolves it at COMPILE time and BAKES the resolved path into the binary, where
    # it becomes the runtime asset root for `WebviewUrl::App(..)`. An absolute path
    # taken from the CI runner therefore ships inside the artifact.
    #
    # The shipped symptom, reported 2026-09-30 against an installed Windows client
    # with a clean config (`start_page: /`) and a fresh install:
    #
    #     File not found
    #     It may have been moved, edited, or deleted.
    #     ERR_FILE_NOT_FOUND
    #
    # with an Edge logo, in a window titled Locus, and this in `latest.log`:
    #
    #     [Window] page load started:  url=file:///D:/
    #
    # The `D:/` is the Windows CI runner's own workspace (`D:\a\Locus\Locus`),
    # compiled into the binary by an absolute `frontendDist` and reintroduced at run
    # time against a directory that does not exist. The window never becomes visible
    # (it is shown on page-load Finished, which never arrives), so the app looks
    # broken while every install is healthy — and no config, restart or reinstall
    # can affect it, because the bad value is in the executable.
    #
    # The path arrived here with a commit ("better workflow") that made CI faster by
    # building the frontend once and overriding `frontendDist` to an absolute path so
    # `tauri build` would reuse it. A follow-up normalized its separators to fix a
    # JSON-escape panic ("invalid escape at line 1 column 54") — which made the build
    # GREEN while leaving the defect intact, because the wrong thing (an absolute
    # runner path in a shipped binary) was never the escaping.
    #
    # The committed `tauri.conf.json` carries `"frontendDist": "../dist"`, relative
    # to the config file, and the build job downloads the bundle to `client/dist` —
    # exactly what `../dist` resolves to. No absolute path is ever needed. This check
    # fails if one is set again, in the workflow OR in the committed config.
    #
    # `frontendDist` may legitimately be a build-machine path in a form that never
    # ships (none today). The rule this enforces is narrow and mechanical: no
    # `frontendDist` value may be, or be built from, an absolute path.
    WORKFLOW="$REPO/.github/workflows/client.yml"
    TAURI_CONF="$REPO/client/src-tauri/tauri.conf.json"

    # (a) The workflow must not set an absolute `frontendDist` anywhere — not as a
    #     literal, and not by interpolating a workspace/root variable. The positive
    #     form is the risky one: a `TAURI_CONFIG` that looks assembled is exactly
    #     what shipped this bug, so both shapes are refused.
    # The rule is deliberately the SIMPLEST one that catches every shape: no LIVE
    # line of the workflow may mention `frontendDist`. The correct state sets none —
    # `tauri.conf.json`'s relative value is used as-is — so any live occurrence is an
    # override, and an override is the defect. Matching "any live mention" also means
    # the guard cannot be evaded by a shape I did not anticipate: the literal,
    # `${{ github.workspace }}`, `$GITHUB_WORKSPACE`, a node/jq-assembled value, and
    # anything future all fail the same way.
    #
    # COMMENT lines are excluded, and that is not a loophole: a `#` line cannot set
    # an environment variable, so it cannot reach the build. The comments above the
    # build step legitimately discuss this defect and name the key; the guard's job
    # is the config, not the prose. (Only full-line comments are stripped — a
    # trailing `# ...` on a live line is NOT, because `key: value # comment` is live
    # config and must be checked.)
    LIVE_WORKFLOW="$(grep -vE '^[[:space:]]*#' "$WORKFLOW" 2>/dev/null || true)"

    if printf '%s' "$LIVE_WORKFLOW" | grep -qF 'frontendDist' 2>/dev/null; then
        bad "the workflow sets 'frontendDist' — it must not"
        bad "  an absolute value is compiled into the binary as its runtime asset root"
        bad "  and ships: a Windows build then resolves its page against"
        bad "  D:/a/Locus/Locus/... and WebView2 shows ERR_FILE_NOT_FOUND on a healthy"
        bad "  install. Normalizing separators does NOT help."
        bad "  use the committed relative \"../dist\" and download the bundle to client/dist"
    else
        ok "the workflow sets no frontendDist override"
    fi

    # (b) The committed config must keep `frontendDist` relative. An absolute value
    #     here is the same defect with a longer fuse: it only breaks on machines that
    #     lack the author's path, and it is what the workflow override was copying.
    if [ ! -f "$TAURI_CONF" ]; then
        bad "client/src-tauri/tauri.conf.json is missing — frontendDist cannot be checked"
    elif grep -qE '"frontendDist"[[:space:]]*:[[:space:]]*"/' "$TAURI_CONF" 2>/dev/null; then
        bad "client/src-tauri/tauri.conf.json sets an absolute 'frontendDist'"
        bad "  it must stay relative (\"../dist\"): an absolute path is compiled into"
        bad "  the binary and breaks every machine that lacks that directory."
    elif grep -qE '"frontendDist"[[:space:]]*:[[:space:]]*"\.\./dist"' "$TAURI_CONF" 2>/dev/null; then
        ok "tauri.conf.json keeps a relative frontendDist (\"../dist\")"
    else
        warn "tauri.conf.json 'frontendDist' is not the expected \"../dist\" — confirm it is relative"
    fi

    # (c) The build job must download the bundle to the directory `../dist` resolves
    #     to. If these drift apart, the relative path silently points at nothing and
    #     `tauri::generate_context!()` panics (or, worse, bundles the wrong tree).
    if grep -qE 'path:[[:space:]]*client/dist' "$WORKFLOW" 2>/dev/null; then
        ok "the build job downloads the frontend bundle to client/dist"
    else
        bad "the build job does not download the frontend bundle to client/dist"
        bad "  the committed \"../dist\" (relative to client/src-tauri/) resolves there;"
        bad "  a different path means the bundled assets are not where tauri looks."
    fi

    echo "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
}
