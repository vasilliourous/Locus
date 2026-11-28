#!/usr/bin/env bash
# Section 2 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_2() {
    # ─────────────────────────────────────────────────────────────
    echo
    echo "2. Frozen wire names"

    # The hub passes tier_configs.config through verbatim, so the stored key is the
    # wire key. `uot_port` must appear in the hook, the seeder, and the client.
    if grep -q 'uot_port' "$HOOKS/heartbeat.pb.js" 2>/dev/null; then
        ok "heartbeat hook references uot_port"
    else
        bad "heartbeat hook no longer references uot_port (clients read it)"
    fi

    if grep -q 'uot_port' "$SCRIPTS/seed-pb.py" 2>/dev/null; then
        ok "seed-pb.py writes uot_port"
    else
        bad "seed-pb.py no longer writes uot_port"
    fi

    if grep -qE 'rename = "uot_port"' "$RUST/contract.rs" 2>/dev/null; then
        ok "client deserialises uot_port"
    else
        bad "client no longer deserialises uot_port (UDP-over-TCP would go silent)"
    fi

    # The record->heartbeat rename is load-bearing: deployed clients read
    # `update_<platform>`, the record stores `download_<platform>`.
    if grep -q 'update_' "$HOOKS/heartbeat.pb.js" 2>/dev/null; then
        ok "heartbeat hook still emits update_<platform>"
    else
        bad "heartbeat hook no longer emits update_<platform> (deployed clients read it)"
    fi

    if grep -q 'download_' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
        ok "publish-release.sh still writes download_<platform>"
    else
        bad "publish-release.sh no longer writes download_<platform>"
    fi
}
