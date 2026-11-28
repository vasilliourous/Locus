#!/usr/bin/env bash
# Section 13 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_13() {
    # ─────────────────────────────────────────────────────────────
    # 13. No Windows path is interpolated raw into JSON that a build parses.
    #
    # WHY THIS EXISTS
    #
    # The Windows build died in the `tauri` build script with:
    #
    #     called `Result::unwrap()` on an `Err` value: invalid escape at line 1 column 54
    #
    # The cause was `TAURI_CONFIG` in `.github/workflows/client.yml`:
    #
    #     {"...","frontendDist":"${{ github.workspace }}/client/dist"}
    #
    # `TAURI_CONFIG` is JSON, parsed by `serde_json::from_str` in tauri-utils. On
    # Windows `github.workspace` is `D:\a\Locus\Locus` (the pre-rename repo name), so the raw interpolation
    # produced `"D:\a\Locus\..."` — where `\a` is an INVALID JSON ESCAPE (`\a` sits
    # at exactly column 54 for this path). Linux and macOS have forward slashes and
    # stayed green, so the failure existed only on the one runner the author could
    # not see.
    #
    # This is the same shape as §5's ':' path bug: a Windows-only break that looks
    # green on every other runner. The guard is a grep for the pattern, because the
    # value interpolated is a runner path that does not exist in this checkout.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "13. No raw Windows-path interpolation into build JSON"

    WORKFLOW="$REPO/.github/workflows/client.yml"
    if [ ! -f "$WORKFLOW" ]; then
        warn "no .github/workflows/client.yml — this guard cannot run"
    else
        # A JSON-looking line (has a "key":"value" shape) that interpolates
        # github.workspace directly. That is always wrong: the result carries
        # backslashes on Windows, which are not valid JSON escapes.
        json_path_hits=$(grep -nE '\$?\{\{?[[:space:]]*github\.workspace' "$WORKFLOW" \
            | grep -E '"' \
            || true)

        if [ -z "$json_path_hits" ]; then
            ok "no JSON line interpolates github.workspace directly"
        else
            bad "github.workspace is interpolated into a JSON-looking line — on Windows this is an invalid JSON escape"
            bad "  normalize to forward slashes and encode with a real JSON encoder (see the build job's TAURI_CONFIG)"
            printf '%s\n' "$json_path_hits" | head -5 | sed 's/^/         /'
        fi

        # The positive half: TAURI_CONFIG must still be exported somewhere, or the
        # build silently reverts to the interactive `beforeBuildCommand` rebuild.
        if grep -q 'TAURI_CONFIG' "$WORKFLOW"; then
            ok "TAURI_CONFIG is still constructed by the workflow"
        else
            warn "TAURI_CONFIG no longer appears in the workflow — if the frontend rebuild was reintroduced deliberately, ignore this"
        fi
    fi
}
