#!/usr/bin/env bash
# Section 32 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_32() {
    echo "32. The hub alias and the client's hub override agree"

    # ── WHY THIS GUARD EXISTS ──
    #
    # A school network (N4L) classified the hub's hostname as uncategorised and
    # blocked it. That blocks activation, heartbeat AND the update path at once,
    # because all three are served from one name — so the device cannot fetch the
    # build that would relieve it.
    #
    # Two halves had to be added for that to be escapable, and each is useless alone:
    #
    #   * `modules/05-caddy.sh` serves a second hostname (`HUB_ALIAS_DOMAIN`) that
    #     redirects to the canonical hub — the SERVER half;
    #   * `chrysalis::contract::resolve_hub_url` honours a stored override — the CLIENT
    #     half, which only helps if a build carrying it is already installed.
    #
    # The failure this guard prevents is the two halves disagreeing: an alias block
    # that redirects somewhere other than the canonical domain would send a stranded
    # fleet to a name that does not serve the API. Anchored on the redirect target,
    # which is the only value that has to be right.
    #
    # 2026-11-07 — THE ALIAS NOW SERVES ONE HANDLE, `/updates/*`, ON PURPOSE.
    # This section previously asserted the alias served NO handles at all, and that
    # assertion was right for the API and wrong for downloads. The alias is used by
    # a device on a network where the CANONICAL NAME IS BLOCKED, so redirecting its
    # download sent the file to a name that does not resolve: the API worked and the
    # update — the only route off that network — did not. URGENT-FIX-NOW.md 1.6.
    #
    # The rule is therefore narrowed, not dropped: the alias may serve downloads and
    # nothing else. What the old check was actually protecting — that the API
    # surface, its rate-limit zones and the admin check exist in exactly one place —
    # is still enforced below, by rejecting any handle other than /updates/*.
    # ─────────────────────────────────────────────────────────────
    CADDY_MODULE="$REPO/server/modules/05-caddy.sh"
    if [ ! -f "$CADDY_MODULE" ]; then
        bad "modules/05-caddy.sh is missing"
    else
        if grep -q 'HUB_ALIAS_DOMAIN' "$CADDY_MODULE"; then
            ok "the Caddy module can serve a second hub hostname"
        else
            bad "modules/05-caddy.sh no longer mentions HUB_ALIAS_DOMAIN — the alias half is gone"
        fi

        # The alias must redirect to the canonical domain, not to itself and not to a
        # hardcoded literal: a literal would drift from DOMAIN the first time the box
        # moves, which is the exact class of fact CLAIMS.md §1 exists for.
        if grep -q 'redir https://__HUB_CANONICAL_DOMAIN__{uri}' "$CADDY_MODULE"; then
            ok "the alias redirects to the canonical hub domain (substituted, not literal)"
        else
            bad "the hub alias redirect target is not __HUB_CANONICAL_DOMAIN__{uri}"
            bad "  A hardcoded host here would drift from DOMAIN; a self-redirect loops."
        fi

        # The redirect must be in a CATCH-ALL `handle {` , not a sibling top-level
        # `redir`. A bare `redir` and a `handle_path` in the same site block do not
        # compose: Caddy evaluates the handle chain first, and a top-level directive
        # runs as its own route, so the redirect could win for /updates/ too and
        # silently restore the 308 — the exact bug, with the /updates/ handle still
        # present and this section's first check still green.
        if awk '/^__HUB_ALIAS_DOMAIN__ \{/,/^\}/' "$CADDY_MODULE" | grep -qE '^[[:space:]]*handle \{'; then
            ok "the alias's redirect is a catch-all handle, so /updates/* is matched first"
        else
            bad "the alias's redirect is not inside a catch-all 'handle {' block"
            bad "  A top-level 'redir' is a separate route and may still catch /updates/,"
            bad "  restoring the 308 while the /updates/ handle sits there looking correct."
        fi

        # ...and that catch-all must REDIRECT, not proxy. Found by mutating this
        # guard's own subject: replacing the catch-all's `redir` with a
        # `reverse_proxy` to PocketBase left all three checks above green while the
        # alias became a second, unrated API surface — the duplication this section
        # exists to prevent, wearing the shape it was told to expect. The `redir`
        # check higher up greps the WHOLE file, so the canonical block's own
        # redirect satisfied it.
        if awk '/^__HUB_ALIAS_DOMAIN__ \{/,/^\}/' "$CADDY_MODULE" \
            | grep -qE '^[[:space:]]*reverse_proxy'; then
            bad "the hub alias block contains a reverse_proxy — it proxies instead of redirecting"
            bad "  That is a second API surface with no rate limit and no admin check."
            bad "  Only /updates/* is served here; everything else must redirect."
        else
            ok "the hub alias proxies nothing — the API stays in one place"
        fi

        # The alias must not be enabled by default: a deployment that has not been
        # given a second name must serve exactly what it served before.
        if grep -q 'HUB_ALIAS_DOMAIN="\${HUB_ALIAS_DOMAIN:-}"' "$CADDY_MODULE"; then
            ok "the hub alias is opt-in (empty by default)"
        else
            bad "the hub alias defaults to something — it must be opt-in"
        fi

        # The alias carries the API, so it must NOT silently extend the landing block,
        # whose whole design reason is to be starved of handles (§26 pins that).
        #
        # EXTRACT THE BLOCK ONCE and assert on it. The previous version piped awk
        # through grep, so "no handle found" and "awk matched nothing at all" were
        # the same answer — a deleted or renamed alias block passed. Both checks now
        # read one extracted block and can tell those apart.
        ALIAS_BLOCK=$(awk '/^__HUB_ALIAS_DOMAIN__ \{/,/^\}/' "$CADDY_MODULE")
        if [ -z "$ALIAS_BLOCK" ]; then
            bad "no hub alias block found — anchored on the line '__HUB_ALIAS_DOMAIN__ {'"
            bad "  Either it was deleted (the school-network escape is gone) or its"
            bad "  opening line changed, which this guard cannot follow."
        else
            # (a) Downloads must be SERVED here, not redirected. This is the 1.6 fix.
            if printf '%s\n' "$ALIAS_BLOCK" | grep -q 'handle_path /updates/\*'; then
                ok "the hub alias SERVES /updates/* itself (downloads escape the block)"
            else
                bad "the hub alias does not serve /updates/* — every download on the"
                bad "  alias would 308 to the canonical name, which is the name a filter"
                bad "  on that network blocks. The device could activate and not update."
                bad "  Fix: a handle_path /updates/* in the alias block before its redirect."
            fi

            # (b) ...and nothing else. The API, its rate-limit zones and the admin
            #     check stay in exactly one place. Checked by listing every handle
            #     in the block and requiring the set to be exactly {/updates/*}.
            OTHER_HANDLES=$(printf '%s\n' "$ALIAS_BLOCK" \
                | grep -oE '^[[:space:]]*handle(_path)?([[:space:]]+[^ {]+)?' \
                | awk '{print $2}' | grep -v '^$' | grep -vx '/updates/\*' | sort -u | tr '\n' ' ')
            if [ -z "$OTHER_HANDLES" ]; then
                ok "the hub alias serves no API handle — no duplicated rate limit or admin check"
            else
                bad "the hub alias serves handle(s) beyond /updates/*: ${OTHER_HANDLES}"
                bad "  That duplicates the rate-limit zones and the admin check. Only"
                bad "  downloads may be served here; extract a Caddy snippet for more."
            fi
        fi
    fi

    # The client half must exist, or the server half has nothing to be reached by.
    if grep -q 'pub fn resolve_hub_url' "$RUST/contract.rs"; then
        ok "the client can resolve a hub override (contract::resolve_hub_url)"
    else
        bad "contract::resolve_hub_url is gone — a filtered fleet cannot be repointed"
    fi

    # ── The LEGACY hub name must keep being SERVED, not redirected ──
    #
    # THE FAILURE THIS PREVENTS, and it is not hypothetical: the canonical name moved
    # off duckdns because N4L blocked it. Every build before 3.3.2 has the OLD name
    # compiled in and no way to learn a new one — the heartbeat is the only channel
    # that carries a hub move, and reaching the heartbeat needs a working hub. So a
    # deploy that emits only the new name strands every earlier device at the
    # activation gate, permanently, with no path to the release that would fix it.
    #
    # Anchored on the SITE-BLOCK ADDRESS LINE, which is the thing that decides what is
    # served. Two weaker checks would both pass while the fleet was stranded: grepping
    # for the legacy string anywhere (a comment mentioning it proves nothing) and
    # checking Caddy still has *a* site block (the canonical one alone is not enough).
    LEGACY_CONST=$(grep -oE 'pub const LEGACY_HUB_URL: &str = "https://[^"]+"' "$RUST/contract.rs" | grep -oE '[a-z0-9.-]+\.[a-z]{2,}' | head -1)
    # Anchored on the ASSIGNMENT at the start of a line, not on the bare name.
    # `grep LEGACY_DOMAIN` matched a COMMENT the first time this guard was written,
    # so the check passed while the default had been deleted — the "grep the string
    # anywhere" failure this whole block warns about, committed by the block itself.
    # Observed failing against the deleted-default case before it was kept.
    LEGACY_ASSIGN=$(grep -cE '^LEGACY_DOMAIN=' "$CADDY_MODULE")
    if [ -z "$LEGACY_CONST" ]; then
        bad "contract.rs declares no LEGACY_HUB_URL — the previous hub name is not recorded where the client owns wire names"
    elif [ "$LEGACY_ASSIGN" -eq 0 ]; then
        bad "modules/05-caddy.sh no longer assigns LEGACY_DOMAIN — a DOMAIN move would stop serving the old name"
        bad "  Deployed pre-3.3.2 builds have it compiled in and cannot be repointed."
    elif ! grep -q '\${HUB_BLOCK_NAMES} {' "$CADDY_MODULE"; then
        bad "the hub site block is not addressed by \${HUB_BLOCK_NAMES}"
        bad "  That variable is what puts BOTH names on one block; using \${DOMAIN} alone"
        bad "  serves only the canonical name and strands every earlier build."
    else
        ok "the hub site block is addressed by \${HUB_BLOCK_NAMES} (canonical + legacy)"
        ok "the legacy hub name is recorded in contract.rs ($LEGACY_CONST)"
    fi

    # And the legacy name must NOT be a redirect: a 308 needs the old name to hold a
    # valid certificate for itself, which is the very thing that failed.
    #
    # Written POSITIVELY — extract the alias block's address line and require it to be
    # exactly the placeholder. The first version of this check ran awk over a range
    # that began with the block header, so when the header changed the range matched
    # NOTHING and the guard passed on an empty string: a check that only looks for an
    # absence cannot fail when the thing it looks at is gone. Observed passing against
    # `__HUB_ALIAS_DOMAIN__, ${LEGACY_DOMAIN} {` before it was rewritten.
    ALIAS_ADDR=$(grep -oE '^__HUB_ALIAS_DOMAIN__[^{]*' "$CADDY_MODULE" | sed 's/[[:space:]]*$//')
    if [ -z "$ALIAS_ADDR" ]; then
        bad "could not find the __HUB_ALIAS_DOMAIN__ site block address — the alias guard cannot check anything"
    elif [ "$ALIAS_ADDR" != "__HUB_ALIAS_DOMAIN__" ]; then
        bad "the hub ALIAS block is addressed by '$ALIAS_ADDR', not __HUB_ALIAS_DOMAIN__ alone"
        bad "  If that includes the legacy name, the legacy fleet is being REDIRECTED —"
        bad "  and a redirect needs the old name's certificate, which is what broke."
    else
        ok "the alias serves only __HUB_ALIAS_DOMAIN__; the legacy name is served, not redirected"
    fi

    # ── The two-sided contract for the alias VALUE ──
    #
    # The hub validates the address before storing it and the client validates it
    # before adopting it. Those are two implementations of one rule, in two
    # languages, which is exactly the shape that has silently diverged three times in
    # this repository. The rule is: absolute https, no path, no query.
    #
    # A domain name is NOT matched by checking for the string "https://" in both
    # files — that would pass on a broken regex in either. Instead the hub's regex is
    # extracted and RUN against the same cases the client's unit test asserts, so a
    # hub-side pattern that accepts a path fails here.
    ADMIN_HOOK="$HOOKS/admin_console.pb.js"
    if grep -q 'hub_alias' "$ADMIN_HOOK" 2>/dev/null; then
        ok "the hub accepts a hub_alias from the console"
    else
        bad "admin_console.pb.js does not accept hub_alias — an operator cannot set it"
    fi

    if grep -q 'response.hub_alias' "$HOOKS/heartbeat.pb.js" 2>/dev/null; then
        ok "the heartbeat delivers hub_alias to clients"
    else
        bad "heartbeat.pb.js does not send hub_alias — a stored override would never reach a device"
    fi

    alias_rule_report=$(node -e '
        const fs = require("fs");
        const src = fs.readFileSync(process.argv[1], "utf8");
        // Pull the exact regex literal the hook applies, so this checks the SHIPPED
        // rule rather than a paraphrase of it. Deliberately a loose capture up to
        // `/` followed by `.test(alias)` — a stricter character class here is how
        // the first version of this check reported a pattern bug that did not exist
        // (the harness, not the hook, was wrong).
        const line = src.split("\n").find((l) => l.includes(".test(alias)"));
        const m = line && line.match(/!\/(.+)\/\.test\(alias\)/);
        if (!m) { console.log("NOPATTERN"); process.exit(0); }
        const re = new RegExp(m[1]);
        const cases = [
          ["https://hub.example.org", true],
          ["http://hub.example.org", false],
          ["hub.example.org", false],
          ["https://hub.example.org/api", false],
          ["https://hub.example.org/?x=1", false],
        ];
        for (const [value, expected] of cases) {
          // The hook stores the address as written; a trailing slash is accepted there
          // because the client strips it. Everything else must be refused by BOTH.
          if (re.test(value) !== expected) console.log("MISMATCH\t" + value);
        }
    ' "$ADMIN_HOOK" 2>/dev/null)

    if [ "$alias_rule_report" = "NOPATTERN" ]; then
        bad "could not find the hub_alias validation rule in admin_console.pb.js"
        bad "  Without it the console would store any string, and a client would"
        bad "  silently ignore a value the operator believes is saved."
    elif [ -n "$alias_rule_report" ]; then
        while IFS="$(printf '\t')" read -r _kind value; do
            bad "the hub's hub_alias rule disagrees with the client on ${value}"
        done <<EOF
$alias_rule_report
EOF
        bad "  The client's rule is contract::resolve_hub_url; see its unit test."
    else
        ok "the hub and the client agree on what a usable hub address is"
    fi
    echo
    echo "════════════════════════════════════════════════════════════"
}
