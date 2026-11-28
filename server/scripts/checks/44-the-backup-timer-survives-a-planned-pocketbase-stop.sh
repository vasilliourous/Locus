#!/usr/bin/env bash
# Section 44 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_44() {
    echo "44. The backup timer survives a planned PocketBase stop"
    BACKUP_MOD="$REPO/server/modules/07-backups.sh"
    if [ ! -f "$BACKUP_MOD" ]; then
        bad "07-backups.sh is missing"
    else
        # (a) No Requires= on the timer unit. Anchored to the TIMER heredoc so a
        #     Requires= elsewhere (the service may legitimately want one) is not
        #     mistaken for this one.
        if awk "/<< 'TIMER'/,/^TIMER\$/" "$BACKUP_MOD" | grep -qE '^Requires='; then
        bad "the backup TIMER carries a Requires= directive"
        bad "  systemd propagates STOP through Requires=, so one planned"
        bad "  'systemctl stop pocketbase' (every restore does this) disables"
        bad "  backups permanently, invisible to is-enabled. Use After= instead."
    else
        ok "the backup timer does not Requires= PocketBase (a planned stop cannot kill it)"
    fi

    # (b) The timer file must still be REWRITTEN when it carries the old
    #     directive, or the fix above never reaches an already-deployed host.
    if grep -q "grep -q '\^Requires=pocketbase.service'" "$BACKUP_MOD"; then
        ok "an already-deployed timer with the old Requires= is rewritten, not left alone"
    else
        bad "07-backups.sh leaves an existing timer file untouched"
        bad "  A host that has already run this module keeps the broken unit and"
        bad "  re-running setup.sh reports success while changing nothing."
    fi
fi

echo
echo "════════════════════════════════════════════════════════════"
}
