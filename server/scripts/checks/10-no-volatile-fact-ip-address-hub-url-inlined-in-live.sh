#!/usr/bin/env bash
# Section 10 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_10() {
    # ─────────────────────────────────────────────────────────────
    # 10. No volatile fact is written into live prose.
    #
    # WHY THIS EXISTS
    #
    # Section 7 above already enforces this for the VERSION: a live document may not
    # inline a version number that disagrees with the manifests. This is the same
    # rule generalised, because the same error was made for other values and nothing
    # caught it:
    #
    #   * An IP address, written as "Live hub host" in the file others were told to
    #     trust. It went stale three times and finished by naming a server that no
    #     longer existed.
    #   * A count of live activation codes with a distributor's name attached. Wrong
    #     the next time a code was sold, and customer-identifying in a product whose
    #     central design decision is to store no identity.
    #
    # Both are CLASS A world claims (docs/operate/CLAIMS.md). The rule this enforces
    # is the one that already worked for versions and generalises cleanly:
    #
    #   YOU CANNOT HAVE DRIFTED FROM A VALUE YOU NEVER WROTE DOWN.
    #
    # So a live document must not contain one at all — it links to state.toml
    # (derived) or CLAIMS.md §5 (world). This is a grep, deliberately: it does not try
    # to understand the sentence, only to notice an unowned value.
    #
    # WHAT IT DOES NOT DO: it cannot tell that "the hub is online" in prose is a claim
    # about the world. That class is covered by CLAIMS.md's rules and the §9 report,
    # not by a pattern — a regex broad enough to catch it would fire on correct
    # sentences, and a guard that cries wolf is worse than no guard (§7 learned this).
    #
    # EXCLUSIONS ARE EXACT PATHS, and each is justified. If you need to add one, add
    # the path here with a reason — do not widen the pattern, or this stops being able
    # to fail.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "10. No volatile fact (IP address / hub URL) inlined in live documentation"

    # Live documentation = everything except the three archives-of-record.
    #   docs/reference/FIXES.md        chronological log: cites the addresses that
    #                                  were live when an entry was written, on purpose
    #   docs/history/, docs/archive/   explicitly retired material
    #   docs/operate/CLAIMS.md         the ONE place a world claim may be described
    #                                  (its §5 register and §1 worked example)
    #   docs/state.toml                the derived-facts data file itself
    #
    # ROOT `*.md` JOINED THE SCAN ON 2026-11-22 (audit F-02, finding 9). The front
    # door made a claim about the landing-page hostname that this section forbids
    # anywhere else, and nothing could see it because the walk started at `docs`.
    # `AGENTS.md` and `CLAUDE.md`/`GEMINI.md` are scanned with it; they are entry
    # points, and an entry point that names an address is the same defect as a
    # document that does.
    #
    # `LICENSE` is not `*.md` and cannot appear. The audit's own scratch file is
    # excluded by name because it quotes the defects it found — including addresses.
    DOCS_TO_SCAN=$(cd "$REPO" && { find docs client/docs -name '*.md' 2>/dev/null; \
            find . -maxdepth 1 -name '*.md' 2>/dev/null; } \
        | grep -vE '^\./' \
        | grep -vE '^docs/reference/FIXES\.md$|^docs/history/|^docs/archive/|^docs/business/redesign/archive/|^docs/operate/CLAIMS\.md$|^DOC-AUDIT-[0-9-]+\.md$|^\.renovation-handover\.md$' \
        | sort)

    # An IPv4 literal. Deliberately does NOT flag addresses that are provably NOT the
    # hub, because those are legitimate and flagging them would train the reader to
    # ignore this check (§7's lesson). The exclusions are closed sets, each with a
    # reason — an address that does not match one of them is a finding:
    #
    #   127.0.0.0/8      loopback
    #   0.0.0.0          "unspecified", used in bind examples
    #   255.255.255.255  broadcast
    #   8.8.8.8 / 8.8.4.4 / 1.1.1.1 / 1.0.0.1   public DNS resolvers, cited in the UoT
    #                    and DoH verification steps as the thing being resolved. They
    #                    are somebody else's stable addresses, not our travelling one.
    #   203.0.113.0/24   RFC 5737 documentation range — reserved for examples,
    #                    guaranteed never routable. Its presence is proof the author
    #                    meant an example.
    #   192.0.2.0/24     RFC 5737 documentation range (same reason)
    #   198.51.100.0/24  RFC 5737 documentation range (same reason)
    #   77.88.8.8        Yandex DNS, used as a second resolver in one probe
    #
    # If you need one more, ADD IT HERE WITH A REASON, in this list. Do not widen the
    # pattern to a class — the next hub address to go stale would hide inside it.
    ALLOWED_IPS='127\.|0\.0\.0\.0$|255\.255\.255\.255$|^8\.8\.8\.8$|^8\.8\.4\.4$|^1\.1\.1\.1$|^1\.0\.0\.1$|^203\.0\.113\.|^192\.0\.2\.|^198\.51\.100\.|^77\.88\.8\.8$'
    IP_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
        [ -n "$f" ] || continue
        grep -nEo '\b([0-9]{1,3}\.){3}[0-9]{1,3}\b' "$f" 2>/dev/null \
            | while IFS=: read -r lineno ip; do
                printf '%s\n' "$ip" | grep -qE "($ALLOWED_IPS)" && continue
                echo "$f:$lineno:$ip"
            done
    done)

    if [ -z "$IP_HITS" ]; then
        ok "no live document inlines an IP address"
    else
        bad "these live documents inline an IP address — it is a world claim, not a fact:"
        printf '%s\n' "$IP_HITS" | sed 's/^/         /'
        bad "Remove it. Address the hub by its domain (docs/state.toml) or record the"
        bad "claim, dated, in docs/operate/CLAIMS.md §5. See CLAIMS.md §1 and §6."
    fi

    # A hub URL inlined in prose. The canonical value is in state.toml and CLAIMS.md;
    # a live doc should reference it rather than restate it. Command examples in the
    # operate guides are the grey area, and they are EXCLUDED by path below because a
    # runbook the operator copies must contain a real URL — that is its function.
    URL_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
        [ -n "$f" ] || continue
        # Exclude the operator runbooks, whose whole job is copy-pasteable commands.
        case "$f" in
            docs/operate/OPS.md|docs/operate/POCKETBASE-SETUP.md|docs/operate/RELEASING.md|docs/operate/DEPLOY.md|docs/operate/SECRETS-MANAGEMENT.md|docs/operate/UPDATE-SYSTEM.md|docs/reference/API.md)
                continue ;;
        esac
        grep -nE 'https?://[a-z0-9.-]*duckdns\.org' "$f" 2>/dev/null | sed "s|^|$f:|"
    done)

    if [ -z "$URL_HITS" ]; then
        ok "no live document inlines the hub URL outside the operator runbooks"
    else
        warn "these live documents inline the hub URL (allowed, but link state.toml where possible):"
        printf '%s\n' "$URL_HITS" | sed 's/^/         /'
    fi

    # A count of live codes / customers. There is no legitimate reason for this to
    # appear in the tree at all — see docs/operate/CLAIMS.md §4.
    COUNT_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
        [ -n "$f" ] || continue
        grep -nE '\b[0-9]+ (real|live|active|paying) (strike |activation )?(codes?|customers?|users?|subscribers?)\b' "$f" 2>/dev/null \
            | sed "s|^|$f:|"
    done)

    if [ -z "$COUNT_HITS" ]; then
        ok "no live document states a count of live codes or customers"
    else
        bad "these live documents state a live headcount — it decays and it identifies a customer:"
        printf '%s\n' "$COUNT_HITS" | sed 's/^/         /'
        bad "Delete the number, keep the rule. See docs/operate/CLAIMS.md §4."
    fi
}
