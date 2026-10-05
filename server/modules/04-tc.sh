#!/usr/bin/env bash
# Module 04: Traffic Shaping (tc)
# Applies bandwidth caps using tc HTB qdisc:
#   - Eco / Free (port 8443): 1 Mbps   (the legacy Eco slot, renamed customer-side)
#   - Full       (port 8445): 100 Mbps (the merged paid tier; on-box name `strike`)
#
# TWO tiers, not three. Stealth (port 8444) was retired when the paid ladder was
# merged into one plan: Stealth and Strike were the same tier at two rates, and
# the only real difference was Strike's UDP-over-TCP endpoint. See
# docs/business/04-tiers.md §4.2.4 and §4.7.
#
# RETIRING 8444 DOES NOT STOP THE OLD SERVICE. `create_tc_service` and
# `02-shadowsocks.sh`'s `write_service` are skip-if-exists (so a re-run is
# idempotent), which means `/etc/shadowsocks/stealth.json` and
# `shadowsocks-stealth.service` survive this module and keep serving 8444 with a
# working password. Disabling them is an EXPLICIT OPERATOR STEP — see
# docs/business/14-risks.md §14.8. This file only stops shaping the port.
#
# All tiers use BBR — the caps are enforced purely with tc.
#
# The 8443 cap was lowered from 5 Mbps to 1 Mbps when Eco became Free, and 8445
# from 200 Mbps to 100 Mbps in the merge. This header and the applied classes
# below must agree; `check-consistency.sh` §23(a)/(b) fails the build if they
# do not, and §23(g) fails it if this prose contradicts them.
#
# Uses a dedicated helper script for systemd oneshot services
# to avoid shell escaping bugs in ExecStart.
set -euo pipefail

log()  { echo "[04-tc] $*"; }
warn() { echo "[04-tc][WARN] $*"; }
fail() { echo "[04-tc][FAIL] $*"; exit 1; }

TC_HELPER="/usr/local/bin/myvpn-tc-apply.sh"
TC_CONF_DIR="/etc/myvpn/tc"

# ── Detect primary interface ──
IFACE=$(ip -4 route show default | awk '{print $5}' | head -1)
if [ -z "$IFACE" ]; then
    fail "Could not detect primary network interface."
fi
log "Primary interface: ${IFACE}"

# ── Write the tc helper script (used by systemd oneshot services) ──
# This avoids inline shell escaping bugs in ExecStart= lines.
# NOTE: always rewritten (not skip-if-exists) so helper updates actually
# reach already-deployed VPSs — previously a stale helper persisted forever
# (hit 2026-08-14: fq_codel + filter-flush changes never applied).
write_tc_helper() {
    mkdir -p "$TC_CONF_DIR"

    cat > "$TC_HELPER" << 'HELPER'
#!/usr/bin/env bash
# Locus tc apply helper — called by systemd oneshot services
# Usage: myvpn-tc-apply.sh <port> <rate> <classid>
# Example: myvpn-tc-apply.sh 8443 1mbit 1:10
set -euo pipefail

PORT="${1:?Usage: $0 <port> <rate> <classid>}"
RATE="${2:?Usage: $0 <port> <rate> <classid>}"
CLASSID="${3:?Usage: $0 <port> <rate> <classid>}"
IFACE=$(ip -4 route show default | awk '{print $5}' | head -1)

if [ -z "$IFACE" ]; then
    echo "[tc-apply] ERROR: Could not detect default interface"
    exit 1
fi

# Ensure root qdisc exists (htb)
tc qdisc add dev "$IFACE" root handle 1: htb default 30 2>/dev/null || true

# Add or update the class
if ! tc class add dev "$IFACE" parent 1: classid "$CLASSID" htb rate "$RATE" ceil "$RATE" 2>/dev/null; then
    tc class change dev "$IFACE" parent 1: classid "$CLASSID" htb rate "$RATE" ceil "$RATE" 2>/dev/null || true
fi

# Low-latency leaf qdisc under this class (gaming optimisation 2026-08-14):
# HTB classes default to FIFO queues -> bufferbloat -> latency spikes when
# the tier is under load. fq_codel gives per-flow fairness + CoDel AQM,
# keeping latency flat (the recommended pairing with BBR). Handle is the
# class minor (1:10 -> 10:, 1:20 -> 20:, 1:30 -> 30:).
HANDLE="${CLASSID#1:}:"
tc qdisc replace dev "$IFACE" parent "$CLASSID" handle "$HANDLE" fq_codel 2>/dev/null || \
    echo "[tc-apply][WARN] Could not attach fq_codel to ${CLASSID} (continuing with FIFO)"

# Add or replace the filter
# The MODULE flushes parent 1: once before applying both tiers; this
# helper is add-only (it also runs from systemd oneshot units at boot where
# there are no stale filters to flush). A per-call flush would leave only
# the LAST tier's filter (hit 2026-08-14).
tc filter add dev "$IFACE" protocol ip parent 1: prio 10 u32 \
    match ip sport "$PORT" 0xffff flowid "$CLASSID" 2>/dev/null || true

echo "[tc-apply] Port $PORT → $CLASSID @ $RATE on $IFACE"
HELPER
    chmod +x "$TC_HELPER"
    log "✓ Created tc helper script: ${TC_HELPER}"
}

# ── Create systemd oneshot service ──
create_tc_service() {
    local name="$1"        # e.g. "eco"
    local classid="$2"     # e.g. "1:10"
    local rate="$3"        # e.g. "1mbit"
    local port="$4"        # e.g. "8443"
    local extra_deps="${5:-}"  # optional: "shadowsocks-eco.service"

    local service_file="/etc/systemd/system/tc-${name}-cap.service"

    if [ -f "$service_file" ]; then
        log "tc-${name}-cap.service already exists"
        return 0
    fi

    # `Before=` is emitted only when there IS a dependency. An unconditional
    # `Before=${extra_deps}` renders as a bare `Before=` line when the argument is
    # empty, which systemd treats as a malformed (empty) unit name — the unit
    # then fails to load and a REBOOT silently loses that cap while the running
    # class still reports the right rate. Found while wiring the free-UDP cap,
    # whose reboot unit passes no dependency.
    local before_line=""
    [ -n "$extra_deps" ] && before_line="Before=${extra_deps}"

    cat > "$service_file" << SERVICE
[Unit]
Description=Apply tc bandwidth cap for ${name^} tier (port ${port})
After=network.target
${before_line}

[Service]
Type=oneshot
ExecStart=${TC_HELPER} ${port} ${rate} ${classid}
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created ${service_file} (port ${port}, rate ${rate})"
}

# ── Apply tc rules immediately ──
apply_tc_now() {
    local port="$1"
    local classid="$2"
    local rate="$3"

    # Check if filter already exists for this port
    if tc filter show dev "$IFACE" parent 1: 2>/dev/null | grep -q "sport ${port}"; then
        log "tc filter for port ${port} already exists — updating rate"
    fi

    # Delegate to the helper script
    bash "$TC_HELPER" "$port" "$rate" "$classid" 2>&1 | while read -r line; do
        log "  ${line}"
    done

    log "✓ Applied tc: port ${port} → ${classid} @ ${rate}"
}

# ═══════════════════════════════════════════
# Main
# ═══════════════════════════════════════════

write_tc_helper

# ── Apply to live interface ──
log "Applying tc rules to ${IFACE}..."
# Flush stale filters ONCE (this parent holds only the tier-port filters),
# then let the three helpers add exactly one filter each. Without the flush,
# repeated `tc filter add` with an identical u32 spec silently creates
# DUPLICATES (observed 2026-08-14: 4 copies of each after repeated runs).
tc filter del dev "$IFACE" parent 1: 2>/dev/null || true
# Apply all tiers (order doesn't matter — different classids)
#
# 8443 is the FREE tier. It reuses the legacy Eco slot (port, password,
# systemd unit) on purpose — see docs/business/04-tiers.md §4.5. New codes
# minted against the `free` tier row get this 1mbit cap; existing `eco`-tier
# codes in the field keep it too, which is the intended downgrade (the free
# tier replaced Eco as a product; it was never a paid tier's promise).
# The customer-facing name is "Free"; the on-box name stays "eco".
apply_tc_now 8443 "1:10" "1mbit"
#
# 8447 is the FREE plan's UDP-over-TCP listener (the sing-box `eco` instance).
# It gets the SAME 1 Mbps class as the free TCP tunnel, and that is the point:
# UDP must not become a way around the free plan's ceiling. Without this class
# the free plan's UDP would be bounded only by the client's own cap, which a
# modified client controls — the hub would be trusting the component it is
# supposed to be shaping. See the header.
# 1:20 is reused here; it previously belonged to the retired Stealth tier, so no
# class id is left dangling and no stale filter survives.
apply_tc_now 8447 "1:20" "1mbit"
#
# 8445 is the PAID tier, called `strike` on the box and "Full" on the card. It
# absorbed Stealth when the paid ladder was merged, so its cap is 100mbit — the
# single rate the paid plan sells, and the rate Stealth already sold at. (A
# retired higher rate belonged to the tier the merge deleted; this comment
# deliberately does not restate its number, because a number in a comment is a
# number the next person copies back into the code. §27(c) enforces that.)
# Keeping the on-box name `strike` keeps its password and — critically — its
# UoT credentials, which the sing-box UDP-over-TCP endpoint serves. Renaming it
# would mean re-pointing those credentials: the one change that could break
# gaming without breaking anything visible.
# See docs/business/04-tiers.md §4.6.
#
# 8444 (Stealth) is RETIRED and deliberately absent. The flush above removes its
# stale filter on the next run; the SERVICE behind the port must be stopped by
# hand (header comment, and docs/business/14-risks.md §14.8).
apply_tc_now 8445 "1:30" "100mbit"

# ── Create systemd services for reboot persistence ──
#
# Unit names: `tc-eco-cap.service` (8443), `tc-eco-udp-cap.service` (8447) and
# `tc-strike-cap.service` (8445). The retired `tc-stealth-cap.service` is NOT
# recreated here — but note `create_tc_service` is skip-if-exists, so an
# upgraded box keeps its old unit until a human removes it.
#
# `tc-eco-udp-cap.service` is NEW, so a re-run on an upgraded box creates it.
# It carries `Before=sing-box-uot-eco.service`, so the cap is in place before the
# listener that carries the traffic it shapes. (Only `Before=` is emitted — the
# oneshot is ordered against the listener, not the reverse.)
create_tc_service "eco"     "1:10" "1mbit"   8443 "shadowsocks-eco.service"
create_tc_service "eco-udp" "1:20" "1mbit"   8447 "sing-box-uot-eco.service"

# The PAID UDP port is deliberately left unshaped here, and this comment exists
# so the asymmetry is not "fixed" by a later reader: the paid plan is sold on
# 100 Mbps, and a tc ceiling on a latency-sensitive flow trades the plan's whole
# premise (low latency for games) for bandwidth the plan is not short of.
# `check-consistency.sh` §27(h) fails the build if a class appears on 8446.
create_tc_service "strike"  "1:30" "100mbit" 8445 "shadowsocks-strike.service"

# ── Enable services ──
systemctl daemon-reload
systemctl enable tc-eco-cap.service 2>/dev/null || warn "Could not enable tc-eco-cap.service"
systemctl enable tc-eco-udp-cap.service 2>/dev/null || warn "Could not enable tc-eco-udp-cap.service"
systemctl enable tc-strike-cap.service 2>/dev/null || warn "Could not enable tc-strike-cap.service"

# Retire the Stealth cap unit if this box still has one. Best-effort and
# deliberately non-fatal: on a FRESH box it has never existed, and on an upgraded
# box this is exactly the "a merge removes something from the code but not from
# the box" step (docs/business/14-risks.md §14.8).
for stale in /etc/systemd/system/tc-stealth-cap.service; do
    if [ -f "$stale" ]; then
        warn "Retired unit present: ${stale} — disabling (the 8444 cap is gone from this module)"
        systemctl disable --now tc-stealth-cap.service 2>/dev/null || \
            warn "Could not disable tc-stealth-cap.service — do it by hand"
    fi
done

# ── Verify ──
log "Verifying tc configuration..."
tc -s class show dev "$IFACE" 2>/dev/null | head -20 || warn "No tc classes (normal for first boot without traffic)"

# Verify specific filters.
#
# WHY THIS DOES NOT GREP FOR "sport <port>", which is what it used to do:
# `tc filter show` prints a u32 filter as a HEX MATCH against the packet header,
# not as a decoded "sport" field. `match ip sport 8443 0xffff` displays as
# `match 20fb0000/ffff0000` — the port lives in the first two bytes of that hex,
# with no "sport" text anywhere in the output. So the old check printed
# "Filter for port 8443 not found" on a perfectly applied configuration, and
# warned about all three ports at once on 2026-10-05 while `tc class show`
# proved the caps were live (1/1/100 Mbps). A verification step that always
# cries wolf is one an operator learns to skip, which is worse than not checking
# at all — the §7 lesson from docs/reference/DEBUGGING-METHOD.md.
#
# The port is decoded from the match instead: "20fb" is 8443, "20ff" is 8447,
# "20fd" is 8445, and the filter's flowid must be the class this module intended.
FILTERS="$(tc filter show dev "$IFACE" parent 1: 2>/dev/null || true)"
for pair in "8443:20fb:1:10" "8447:20ff:1:20" "8445:20fd:1:30"; do
    fport="${pair%%:*}"; rest="${pair#*:}"
    fhex="${rest%%:*}"; fclass="${rest#*:}"
    if printf '%s' "$FILTERS" | grep -qi "match ${fhex}0000/ffff0000" \
       && printf '%s' "$FILTERS" | grep -qi "flowid ${fclass}"; then
        log "✓ Filter for port ${fport} → class ${fclass} is active"
    else
        warn "Filter for port ${fport} (class ${fclass}) NOT found — run: systemctl restart tc-eco-cap.service"
    fi
done

# The retirement check, stated as the property that matters: 8444 must not be
# shaped any more. Silent on a fresh box (never existed); loud on an upgraded one
# where the old unit survived.
if printf '%s' "$FILTERS" | grep -qi "match 20fc0000/ffff0000"; then
    warn "Retired Stealth port 8444 is STILL shaped — remove /etc/systemd/system/tc-stealth-cap.service by hand"
else
    log "✓ Retired Stealth port 8444 is not shaped"
fi

log "✓ Traffic shaping setup complete"
exit 0
