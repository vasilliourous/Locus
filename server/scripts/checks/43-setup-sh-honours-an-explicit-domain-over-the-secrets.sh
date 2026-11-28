#!/usr/bin/env bash
# Section 43 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_43() {
    echo "43. setup.sh honours an explicit DOMAIN over the secrets bundle"
    SETUP_SH="$REPO/server/setup.sh"
    if [ ! -f "$SETUP_SH" ]; then
        bad "server/setup.sh is missing"
    elif ! command -v bash >/dev/null 2>&1; then
        warn "bash unavailable — cannot evaluate setup.sh's DOMAIN precedence"
    else
        # Lift the real block out of setup.sh and run it, rather than restating it.
        #
        # RUN FROM A FILE, NOT `bash - <<HEREDOC`. The stdin form collides: the
        # heredoc IS the script, so the first `cat "$1"` or `source` consumes the
        # rest of the harness and the extraction silently returns nothing — which
        # is how this check first reported BAD against correct code. A guard that
        # misreports correct code is worse than no guard, so the harness is
        # materialised on disk and deleted.
        PREC_HARNESS=$(mktemp /tmp/.cc-prec-XXXXXX.sh)
        cat > "$PREC_HARNESS" <<'PREC'
src=$(cat "$1")
start=$(printf '%s' "$src" | grep -n 'DOMAIN_EXPLICIT="${DOMAIN:-}"' | head -1 | cut -d: -f1)
end=$(printf '%s' "$src" | grep -n 'unset DOMAIN_EXPLICIT' | head -1 | cut -d: -f1)
if [ -z "$start" ] || [ -z "$end" ]; then echo "MISSING"; exit 0; fi
block=$(printf '%s' "$src" | sed -n "${start},${end}p")
SECRETS_TMP="$2"
printf 'DOMAIN=networkingguides.duckdns.org\n' > "$SECRETS_TMP"
log() { :; }
fail() { echo "HARNESS_FAIL"; }
# (i) an explicit DOMAIN must survive sourcing the bundle
DOMAIN=networkingguides.jadedns.uk
eval "$block"
printf 'EXPLICIT|%s\n' "$DOMAIN"
# (ii) with nothing explicit, the bundle's value must still apply
#      Re-create it: the block deletes SECRETS_TMP after sourcing, so the
#      second run needs its own copy. (The first version reused one file and
#      got HARNESS_FAIL — the guard caught its own harness, which is the point.)
printf 'DOMAIN=networkingguides.duckdns.org\n' > "$SECRETS_TMP"
unset DOMAIN
eval "$block"
printf 'NONE|%s\n' "$DOMAIN"
PREC
        PREC_OUT=$(bash "$PREC_HARNESS" "$SETUP_SH" "/tmp/.cc-prec-$$.env" 2>/dev/null)
        rm -f "$PREC_HARNESS" "/tmp/.cc-prec-$$.env"
        case "$PREC_OUT" in
            MISSING*)
                bad "setup.sh no longer has the DOMAIN_EXPLICIT precedence block"
                bad "  Without it an explicit DOMAIN=... is silently overwritten by"
                bad "  secrets.env.age, and the hub deploys at the bundle's hostname." ;;
            *"EXPLICIT|networkingguides.jadedns.uk"*"NONE|networkingguides.duckdns.org"*)
                ok "an explicit DOMAIN overrides the bundle, and the bundle still supplies the default" ;;
            *)
                bad "setup.sh's DOMAIN precedence is not what it claims"
                bad "  expected explicit to win and the bundle to supply the default; got:"
                bad "  $(printf '%s' "$PREC_OUT" | tr '\n' ' ') " ;;
        esac

        # The bundle must be READ LITERALLY, not `source`d. Measured 2026-11-07 on a
        # real deploy: VPS_PW contains a literal '$', `source` expanded it, and under
        # `set -u` setup.sh died with "line 49: Zy6N: unbound variable" — after
        # installing age and decrypting, before installing anything. A generated
        # password may contain any printable character.
        #
        # Behavioural, not a grep: a value with a '$' must round-trip unchanged.
        LIT_OUT=$(bash -c '
          printf "%s\n" "VPS_PW=ab\$cd^ef" > /tmp/.cc-lit-$$.env
          set -u
          while IFS= read -r l || [ -n "$l" ]; do
            case "$l" in ""|"#"*) continue;; esac
            case "$l" in [A-Za-z_]*=*) k=${l%%=*}; v=${l#*=}; export "$k=$v";; esac
          done < /tmp/.cc-lit-$$.env
          printf "%s" "$VPS_PW"; rm -f /tmp/.cc-lit-$$.env' 2>/dev/null)
        if grep -qE '^\s*source\s+"\$SECRETS_TMP"' "$SETUP_SH"; then
            bad "setup.sh sources the decrypted bundle — a '\$' in VPS_PW aborts the deploy"
            bad "  under set -u ('unbound variable'). Read KEY=value literally instead."
        elif [ "$LIT_OUT" = 'ab$cd^ef' ]; then
            ok "the secrets bundle is read literally, so a '\$' in a password survives"
        else
            bad "a literal-ish read of the secrets bundle did not preserve a '\$' value"
        fi
    fi

    # ─────────────────────────────────────────────────────────────
    # 44. The backup timer must not be life-coupled to PocketBase
    #
    # Measured 2026-11-07 on the live hub: pocketbase-backup.timer was `enabled`
    # but `inactive (dead)`, "0 timers listed", deactivated at the exact second
    # PocketBase was stopped to restore the database. Its [Unit] had
    # `Requires=pocketbase.service`, and systemd's Requires= propagates STOP as
    # well as start: stopping the required unit stops the requiring one, and
    # nothing brings it back (`enabled` only re-arms at boot).
    #
    # So a single planned `systemctl stop pocketbase` — which restore.sh does on
    # EVERY restore, and any upgrade or DB swap does — silently disables backups.
    # The failure is invisible: is-enabled still says enabled, and the newest
    # backup in B2 just quietly ages. You find out when you need a backup.
    #
    # Two assertions, because there were two defects: the directive itself, and
    # the "already exists, leave it" early return that made the fix unreachable on
    # a host that had already run the module.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "════════════════════════════════════════════════════════════"
}
