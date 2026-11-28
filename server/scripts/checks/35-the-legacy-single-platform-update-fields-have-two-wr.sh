#!/usr/bin/env bash
# Section 35 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_35() {
    echo "35. The legacy single-platform update fields have two writers, and they agree"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `update_config` carries FOUR per-platform artifacts plus a LEGACY pair —
    # `update_url` / `update_sha256` — describing the Linux binary only, kept so a
    # client that cannot find its own platform key still has a fallback.
    #
    # Two code paths write that record:
    #
    #   * `server/scripts/publish-release.sh`  — the CLI
    #   * `admin_console.pb.js` → releases.publish — the console button
    #
    # Only the FIRST knew the legacy pair existed. The console path wrote version,
    # active, download_*, sha256_* and signature_* and left update_url/update_sha256
    # pointing at the PREVIOUS release — so the record claimed 3.3.1 while its legacy
    # URL said 3.2.30, and `update_sha256` hashed a file that was not the one the URL
    # named. Observed live on 2026-10-18. A client falling back to those fields would
    # download the old binary and fail the hash (fail-closed), so an otherwise perfect
    # release would simply never install.
    #
    # Asserting the AGREEMENT is the point. Checking each writer separately would pass
    # with one of them still blind — which is exactly how this shipped.
    # ─────────────────────────────────────────────────────────────
    PUBLISH_SH="$SCRIPTS/publish-release.sh"

    # Count ACTUAL WRITES, not mentions.
    #
    # `grep -q update_url` passes on a comment that merely names the field — the
    # first version of this guard did exactly that, and passed against a hook with
    # every write deleted. Observed failing before it was rewritten. The patterns
    # below match an assignment (`"update_url":` in the Python heredoc,
    # `recP.set("update_url"` in the hook), which prose does not produce.
    pub_writes=$(grep -cE '"update_url" *:|update_url *=' "$PUBLISH_SH" 2>/dev/null || true)
    hook_url_writes=$(grep -cE 'recP\.set\("update_url"' "$ADMIN_HOOK" 2>/dev/null || true)
    hook_sha_writes=$(grep -cE 'recP\.set\("update_sha256"' "$ADMIN_HOOK" 2>/dev/null || true)
    pub_writes="${pub_writes:-0}"
    hook_url_writes="${hook_url_writes:-0}"; hook_sha_writes="${hook_sha_writes:-0}"

    if [ "$pub_writes" -lt 1 ]; then
        bad "publish-release.sh no longer writes update_url — the legacy fallback would go stale"
    elif [ "$hook_url_writes" -lt 1 ] || [ "$hook_sha_writes" -lt 1 ]; then
        bad "admin_console.pb.js releases.publish writes update_url ${hook_url_writes}x and update_sha256 ${hook_sha_writes}x; each needs at least one write"
        bad "  Both update_url AND update_sha256 must be written on every publish."
        bad "  A stale update_url advertises the PREVIOUS release's Linux binary while"
        bad "  update_sha256 hashes a different file — the updater then fails closed on"
        bad "  a release that is otherwise perfectly published (observed 2026-10-18)."
    elif ! grep -q 'linuxArt' "$ADMIN_HOOK" 2>/dev/null; then
        bad "admin_console.pb.js sets update_url from something other than the linux artifact"
        bad "  The legacy pair describes the LINUX binary; sourcing it elsewhere would"
        bad "  pair one platform's URL with another's hash."
    else
        ok "both update_config writers set the legacy update_url/update_sha256 pair"
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
