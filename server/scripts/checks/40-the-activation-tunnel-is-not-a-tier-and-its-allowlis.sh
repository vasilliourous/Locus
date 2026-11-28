#!/usr/bin/env bash
# Section 40 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_40() {
    echo "40. The activation tunnel is not a tier, and its allowlist is closed"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `09-activation-tunnel.sh` runs a Shadowsocks listener whose credential is
    # COMPILED INTO THE CLIENT, so it must be assumed public. Everything that keeps
    # that acceptable is a property of the forwarder: the set of paths it will carry.
    #
    # This path bypasses Caddy entirely — which is where `/api/admin/*`, `/admin/`
    # and `/updates/` are separated and where the activation rate limit lives. A
    # single careless allowlist entry would therefore expose a privileged handler
    # through a credential any student can extract, and nothing at runtime would say
    # so: it would look exactly like a working tunnel.
    #
    # The rate limit is the second half. Without it the public bootstrap key turns
    # the endpoint that mints entitlements into an unmetered one.
    #
    # Both were proven to fail before being kept (see the module header): the
    # allowlist predicate was run against `/api/admin/fetch-release`,
    # `/api/activate/../../admin/x` and `%2e%2e`-encoded traversal, and `rate_limited`
    # was run past its own limit and across two peer addresses.
    # ─────────────────────────────────────────────────────────────
    TUNNEL_MODULE="$REPO/server/modules/09-activation-tunnel.sh"

    if [ ! -f "$TUNNEL_MODULE" ]; then
        bad "09-activation-tunnel.sh is missing — the module list in setup.sh references it"
    else
        # 1. The allowlist is exactly the three enrolment endpoints, and nothing that
        #    reaches a privileged or administrative surface.
        allowlist=$(sed -n 's/^ALLOWED = (\(.*\))$/\1/p' "$TUNNEL_MODULE" | head -1)
        if [ -z "$allowlist" ]; then
            bad "could not read ALLOWED from the forwarder — the allowlist guard is blind"
        else
            case "$allowlist" in
                *admin*|*release*|*updates*|*/_/*)
                    bad "the activation tunnel's allowlist names a privileged path: ${allowlist}"
                    ;;
                *) ok "the activation tunnel's allowlist names no privileged path" ;;
            esac

            for want in /api/activate /api/code-lookup /api/heartbeat; do
                case "$allowlist" in
                    *"\"$want\""*) ok "allowlist carries ${want}" ;;
                    *) bad "allowlist does not carry ${want} — enrolment needs it" ;;
                esac
            done
        fi

        # 2. The rate limit exists and matches Caddy's, because a laxer limit here is
        #    the weakest link on the entitlement-minting endpoint.
        # Parsed with awk rather than two sed ranges: the first `/}/` after
        # `zone activation {` closes the `key {remote_host}` block, not the zone, so a
        # sed range ended before `events`/`window` were ever seen — and the section
        # then WARNed instead of comparing. A guard whose input it cannot find must
        # not read as a pass.
        caddy_rate=$(awk '
            /zone activation \{/ { inzone = 1; next }
            inzone && /events/ { ev = $2 }
            inzone && /window/ { win = $2; sub(/m$/, "", win); print ev, win; exit }
        ' "$REPO/server/modules/05-caddy.sh")
        caddy_events=$(printf '%s' "$caddy_rate" | awk '{print $1}')
        caddy_window=$(printf '%s' "$caddy_rate" | awk '{print $2}')

        tunnel_events=$(sed -n 's/^RATE_EVENTS = \([0-9][0-9]*\).*/\1/p' "$TUNNEL_MODULE" | head -1)
        tunnel_window=$(sed -n 's/^RATE_WINDOW = \([0-9][0-9]*\).*/\1/p' "$TUNNEL_MODULE" | head -1)

        if [ -z "$tunnel_events" ] || [ -z "$tunnel_window" ]; then
            bad "the activation tunnel declares no rate limit — the bootstrap key would be unmetered"
        elif [ -z "$caddy_events" ] || [ -z "$caddy_window" ]; then
            warn "could not read Caddy's activation rate limit; cannot compare"
        else
            expected_secs=$((caddy_window * 60))
            if [ "$tunnel_events" -gt "$caddy_events" ]; then
                bad "the tunnel allows ${tunnel_events} activations where Caddy allows ${caddy_events}"
            elif [ "$tunnel_window" -lt "$expected_secs" ]; then
                bad "the tunnel's window (${tunnel_window}s) is shorter than Caddy's (${expected_secs}s)"
            else
                ok "the tunnel's rate limit is no laxer than Caddy's (${caddy_events}/${caddy_window}m)"
            fi
        fi

        # 3. The port is NOT a tier port. A collision here would take a live student's
        #    tunnel down to fix enrolment, or silently serve the wrong credential.
        tunnel_port=$(sed -n 's/.*ACTIVATION_TUNNEL_PORT:-\([0-9][0-9]*\).*/\1/p' "$TUNNEL_MODULE" | head -1)
        tunnel_port=${tunnel_port:-8453}
        tier_ports=$(grep -oE '^(value[[:space:]]*=[[:space:]]*\{ port = )[0-9]+' "$REPO/docs/state.toml" \
            | grep -oE '[0-9]+$' || true)
        clash=0
        for tp in $tier_ports 8446 8447; do
            [ "$tp" = "$tunnel_port" ] && clash=1
        done
        if [ "$clash" -eq 1 ]; then
            bad "the activation tunnel is on ${tunnel_port}, which is a tier/UoT port"
        else
            ok "the activation tunnel (${tunnel_port}) is not a tier port"
        fi

        # 4. The firewall must open the very port the module listens on, or the
        #    listener is unreachable and every school-network test silently times out
        #    — the confusion recorded on 2026-10-29.
        if grep -q 'ACTIVATION_TUNNEL_PORT' "$REPO/server/modules/08-firewall.sh"; then
            ok "the firewall opens the activation tunnel's port"
        else
            bad "08-firewall.sh does not open the activation tunnel's port"
        fi

        # 5. It is not advertised to any client: no hook may emit it as a tier.
        if grep -rlE '8453|activation_tunnel' "$REPO/server/pb_hooks" 2>/dev/null | grep -q .; then
            bad "a pb_hook mentions the activation tunnel — it must never be advertised as a tier"
        else
            ok "no hook advertises the activation tunnel"
        fi

        # 6. The listener config must NOT claim an inbound forward it does not have.
        #    `ssserver` decrypts and then connects to whatever destination the CLIENT
        #    named; `local_addr`/`local_port` are `sslocal`'s SOCKS listen options and
        #    the server ignores them. The config carried them until 2026-11-01, and the
        #    effect was that the path filter — and so the allowlist that bounds a public
        #    bootstrap key — was never in the path at all. Nothing failed, because
        #    nothing checked.
        if grep -qE '"local_(addr|port)"' "$TUNNEL_MODULE"; then
            bad "09-activation-tunnel.sh sets local_addr/local_port in the ssserver config — ssserver does not forward; the path filter would never be reached"
        else
            ok "the tunnel config claims no inbound forward (ssserver has none)"
        fi

        # 7. The client and the hub must agree on the port the tunnelled request is
        #    ADDRESSED to. That destination is the path filter, and it is a different
        #    number from the listener: naming the listener would loop the decrypted
        #    stream back at the process that terminates it.
        client_filter_port=$(sed -n 's/^pub const FILTER_PORT: u16 = \([0-9][0-9]*\);/\1/p' \
            "$REPO/client/src-tauri/src/chrysalis/activation_tunnel.rs" 2>/dev/null | head -1)
        hub_filter_port=$(sed -n 's/.*ACTIVATION_FORWARD_PORT:-\([0-9][0-9]*\).*/\1/p' "$TUNNEL_MODULE" | head -1)
        hub_filter_port=${hub_filter_port:-18090}
        if [ -z "$client_filter_port" ]; then
            warn "could not read FILTER_PORT from the client; cannot compare with the hub"
        elif [ "$client_filter_port" != "$hub_filter_port" ]; then
            bad "the client addresses the filter on ${client_filter_port}, the hub listens on ${hub_filter_port}"
        elif [ "$client_filter_port" = "$tunnel_port" ]; then
            bad "the client addresses ${client_filter_port}, which is the SS listener — a tunnelled request would dial the listener back at itself"
        else
            ok "the client and the hub agree the filter is on ${client_filter_port}"
        fi

        # 8. A tunnelled request must reach PocketBase as a DISTINCT client.
        #
        #    The forwarder reaches PocketBase at 127.0.0.1:8090, so `remoteAddr`
        #    downstream is always loopback and both enrolment hooks key their budget on
        #    `fp || ip`. `/api/activate` does not require a fingerprint, so before this
        #    every student arriving through the tunnel shared ONE `activate_` bucket of
        #    5 attempts per 10 minutes: the first five in a class activated and the rest
        #    were told "too many attempts", which a client reports as a failure to reach
        #    the hub. Measured 2026-11-09; nothing on the hub reported it.
        #
        #    Three things must hold together, and each has been observed failing:
        #      (a) the forwarder SETS the identity header on the forwarded request;
        #      (b) both enrollment hooks READ it, and neither falls back to the bare
        #          loopback address when it is present;
        #      (c) the value is derived from the peer, not taken from the client — a
        #          client-supplied header would let one device drain another's budget.
        tunnel_identity_header="X-Chrysalis-Bootstrap"
        if ! grep -q "\"${tunnel_identity_header}\": identity(peer)" "$TUNNEL_MODULE"; then
            bad "09-activation-tunnel.sh does not set ${tunnel_identity_header} on the forwarded request — every tunnelled client would share one rate-limit bucket"
        else
            ok "the forwarder sets ${tunnel_identity_header} on every forwarded request"
        fi

        if ! grep -q "def identity(peer" "$TUNNEL_MODULE"; then
            bad "the forwarder derives no per-client identity"
        elif ! sed -n '/^def identity(peer/,/^def allowed/p' "$TUNNEL_MODULE" | grep -v '^\s*#' | grep -q 'client_address'; then
            bad "the forwarder's identity does not use the peer it observed — a client-supplied value would be forgeable"
        else
            ok "the identity is derived from the peer the forwarder observed, not from the client"
        fi

        for hook in activation code_lookup; do
            hook_file="$REPO/server/pb_hooks/${hook}.pb.js"
            if ! grep -q "x-chrysalis-bootstrap" "$hook_file" 2>/dev/null; then
                bad "${hook}.pb.js does not read ${tunnel_identity_header} — a tunnelled client would be bucketed by loopback"
            elif ! grep -qE 'rateKey = "(activate|lookup)_" \+ \(fp \|\| bootstrapId \|\| ip\)' "$hook_file"; then
                bad "${hook}.pb.js does not use the bootstrap identity in its rate key (expected \`fp || bootstrapId || ip\`)"
            else
                ok "${hook}.pb.js buckets by fingerprint, then the bootstrap identity, then the address"
            fi
        done
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
