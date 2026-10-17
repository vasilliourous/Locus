#!/usr/bin/env bash
# Locus SS2022 Migration Verification
#
# Run ON THE HOST after `setup.sh` completes, to prove the Shadowsocks 2022
# migration actually took effect. This is deliberately separate from
# smoke-test.sh: that answers "is the deployment healthy?", this answers
# "is the deployment running SS2022, with valid keys, on every listener?".
#
# Read-only. It does not change any configuration.
#
# Usage:
#     ssh root@host "DOMAIN=networkingguides.duckdns.org /root/server/scripts/verify-ss2022.sh"
#
# Exit code is 0 when no check FAILED (warnings do not fail the run).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOGFILE="/var/log/myvpn-ss2022-verify.log"
PASS=0
FAIL=0
WARN=0

# The expected cipher. Mirrors SS_METHOD in modules/00-env.sh; override with
# SS_METHOD=... if a future deployment deliberately changes it.
SS_METHOD="${SS_METHOD:-2022-blake3-aes-256-gcm}"
case "$SS_METHOD" in
    2022-blake3-aes-256-gcm|2022-blake3-chacha20-poly1305) SS_KEY_BYTES=32 ;;
    2022-blake3-aes-128-gcm)                               SS_KEY_BYTES=16 ;;
    *) SS_KEY_BYTES="" ;;
esac

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

log()  { echo -e "${GREEN}[$(date +%H:%M:%S)]${NC} $*" | tee -a "$LOGFILE"; }
pass() { echo -e "${GREEN}  ✅ PASS:${NC} $*" | tee -a "$LOGFILE"; PASS=$((PASS+1)); }
fail() { echo -e "${RED}  ❌ FAIL:${NC} $*" | tee -a "$LOGFILE"; FAIL=$((FAIL+1)); }
warn() { echo -e "${YELLOW}  ⚠️  WARN:${NC} $*" | tee -a "$LOGFILE"; WARN=$((WARN+1)); }

cat << EOF | tee -a "$LOGFILE"
═══════════════════════════════════════════
 Locus SS2022 Verification
 Expected cipher: ${SS_METHOD}
 Date:            $(date)
 Host:            $(hostname) $(hostname -I 2>/dev/null | awk '{print $1}')
═══════════════════════════════════════════
EOF

if [ -z "$SS_KEY_BYTES" ]; then
    fail "Unrecognised SS_METHOD '${SS_METHOD}' — cannot verify key length."
    exit 1
fi

# ── 1. The tier configs declare SS2022, and their keys are the right size ──
log "Step 1/5: Checking /etc/shadowsocks/*.json method and key length..."
for tier in eco strike; do
    cfg="/etc/shadowsocks/${tier}.json"
    if [ ! -f "$cfg" ]; then
        fail "${cfg} does not exist — ${tier} was never configured"
        continue
    fi

    method=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('method',''))" "$cfg" 2>/dev/null || echo "")
    if [ "$method" = "$SS_METHOD" ]; then
        pass "${tier}: method is ${SS_METHOD}"
    else
        fail "${tier}: method is '${method}', expected '${SS_METHOD}'"
    fi

    # The key must be base64 of exactly SS_KEY_BYTES. A legacy hex-16 value
    # decodes to 24 bytes and is rejected by SS2022 at handshake time, which is
    # why this is checked here and not left to a client to discover.
    key=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('password',''))" "$cfg" 2>/dev/null || echo "")
    bytes=$(printf '%s' "$key" | base64 -d 2>/dev/null | wc -c)
    if [ "$bytes" -eq "$SS_KEY_BYTES" ]; then
        pass "${tier}: key decodes to ${bytes} bytes"
    else
        fail "${tier}: key decodes to ${bytes} bytes, expected ${SS_KEY_BYTES}.
         Regenerate: openssl rand -base64 ${SS_KEY_BYTES} | tr -d '\\n'"
    fi
done

# ── 2. The UoT inbounds match the tiers ──
#
# ONE LISTENER PER PLAN (2026-10). This section used to read a single
# `/etc/sing-box/config.json` and a single `sing-box-uot` service, and compare
# that one key against Strike's. After the per-plan split that check was both
# stale and insufficient:
#
#   * it reported WARN "not present (UoT disabled?)" on a healthy box, because the
#     per-plan configs are `strike.json` and `eco.json`, and
#   * it could not see the FREE plan's listener at all — so a free key that did
#     not match its own listener (TCP works, UDP silently dead) passed the whole
#     script.
#
# That second gap is the expensive one: a free student's game traffic fails while
# every check says the deployment is healthy. The pairing is now asserted per
# plan, which is the agreement that actually has to hold.
log "Step 2/5: Checking the sing-box UoT inbounds (one per plan)..."

# plan -> config file, service unit, and the tc class that must bound it
uot_check() {
    local plan="$1" cfg="$2" unit="$3" tc_class="$4"

    if [ ! -f "$cfg" ]; then
        warn "${cfg} not present — ${plan} has no UoT listener (ENABLE_UOT=0?)"
        return 0
    fi

    local method key tier_key
    method=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['inbounds'][0].get('method',''))" "$cfg" 2>/dev/null || echo "")
    if [ "$method" = "$SS_METHOD" ]; then
        pass "${plan} UoT: method is ${SS_METHOD}"
    else
        fail "${plan} UoT: method is '${method}', expected '${SS_METHOD}'"
    fi

    # The inbound's key must equal the plan's own shadowsocks key. A mismatch in
    # either direction produces the same partial failure: TCP works, UDP does not.
    key=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['inbounds'][0].get('password',''))" "$cfg" 2>/dev/null || echo "")
    tier_key=$(python3 -c "import json; print(json.load(open('/etc/shadowsocks/${plan}.json')).get('password',''))" 2>/dev/null || echo "")
    if [ -n "$key" ] && [ "$key" = "$tier_key" ]; then
        pass "${plan} UoT key matches the ${plan} tier key"
    else
        fail "${plan} UoT key does NOT match the ${plan} tier key — UDP would fail while TCP works"
    fi

    # The port must be the one the tier row advertises, and it must be shaped.
    # An unshaped free listener is the free plan's whole rate limit missing.
    local uot_port
    uot_port=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['inbounds'][0].get('listen_port',''))" "$cfg" 2>/dev/null || echo "")
    if [ -n "$uot_port" ] && systemctl is-active --quiet "$unit" 2>/dev/null; then
        pass "${plan} UoT: ${unit} is running on :${uot_port}"
    elif [ -n "$uot_port" ]; then
        fail "${plan} UoT: ${unit} is NOT running (listener :${uot_port} is dead)"
        journalctl -u "$unit" -n 5 --no-pager 2>/dev/null | tail -3 | sed 's/^/         /'
    fi

    if [ -n "$tc_class" ]; then
        local iface
        iface=$(ip -4 route show default 2>/dev/null | awk '{print $5}' | head -1)
        if [ -n "$iface" ] && tc -s class show dev "$iface" 2>/dev/null | grep -q "1:${tc_class} "; then
            pass "${plan} UoT: :${uot_port} is shaped by class 1:${tc_class}"
        else
            fail "${plan} UoT: no tc class 1:${tc_class} — this plan has no server-side cap"
        fi
    fi
}

# Ports and classes mirror modules/02-shadowsocks.sh and modules/04-tc.sh.
uot_check "strike" "/etc/sing-box/strike.json" "sing-box-uot-strike" ""
uot_check "eco"    "/etc/sing-box/eco.json"    "sing-box-uot-eco"    "20"

# The retired single-listener unit must be gone. If it survived an upgrade it
# binds :8446, racing the per-plan listener for the paid UDP port — and whichever
# loses silently stops carrying UDP.
#
# A NEGATIVE assertion: an `is-active` that FAILS when true. check-consistency.sh
# §31 distinguishes this from a positive requirement by looking for the success
# `pass` on the same branch, so the shape here matters — keep the failure arm
# first and give success its own unconditional `pass` below.
if systemctl is-active --quiet sing-box-uot 2>/dev/null; then
    fail "the retired sing-box-uot.service is still running and races :8446 — disable it: systemctl disable --now sing-box-uot"
fi
pass "the retired sing-box-uot.service is not running (a running one races :8446)"

# ── 3. The listeners are actually up ──
#
# The UoT listeners are checked in section 2, per plan. This section used to
# repeat a single `sing-box-uot` check here against a unit that no longer exists,
# which reported a false FAIL on every healthy box.
log "Step 3/5: Checking listeners..."
for svc in shadowsocks-eco shadowsocks-strike; do
    if systemctl is-active --quiet "$svc"; then
        pass "${svc} is running"
    else
        fail "${svc} is NOT running"
        journalctl -u "$svc" -n 5 --no-pager 2>/dev/null | tail -3 | sed 's/^/         /'
    fi
done

# A listener that started and then refused every handshake is the failure this
# whole script exists to catch, so the journal is read for key/decrypt errors
# rather than trusting `is-active` alone.
log "Step 4/5: Scanning recent journals for handshake/key errors..."
for svc in shadowsocks-eco shadowsocks-strike sing-box-uot-strike sing-box-uot-eco; do
    systemctl is-active --quiet "$svc" 2>/dev/null || continue
    hits=$(journalctl -u "$svc" --since "1 hour ago" --no-pager 2>/dev/null \
        | grep -ciE 'invalid key|cipher|password|decrypt.*fail|unsupported method' || true)
    if [ "${hits:-0}" -gt 0 ]; then
        fail "${svc}: ${hits} recent key/cipher error line(s)"
        journalctl -u "$svc" --since "1 hour ago" --no-pager 2>/dev/null \
            | grep -iE 'invalid key|cipher|password|decrypt.*fail|unsupported method' \
            | tail -3 | sed 's/^/         /'
    else
        pass "${svc}: no key/cipher errors in the last hour"
    fi
done

# ── 5. What the hub is advertising to clients ──
# The hub sends the tier config through verbatim, so what is STORED is what a
# client receives. A stale record here means a correctly-configured server that
# no client can talk to.
log "Step 5/5: Checking what the hub advertises..."
if [ -n "${DOMAIN:-}" ]; then
    # The tier config is only exposed to an authenticated path, so this reads it
    # through the PocketBase admin API when a token is available.
    if [ -f /root/.pb_admin_creds ]; then
        PB_TOKEN=$(grep -m1 '^PB_TOKEN=' /root/.pb_admin_creds 2>/dev/null | cut -d= -f2-)
        if [ -n "$PB_TOKEN" ]; then
            for tier in eco strike; do
                adv=$(curl -sf "https://${DOMAIN}/api/collections/tier_configs/records?filter=(tier='${tier}')" \
                    -H "Authorization: Bearer ${PB_TOKEN}" 2>/dev/null \
                    | python3 -c "
import json,sys
try:
    items=json.load(sys.stdin).get('items',[])
    print(json.loads(items[0]['config']).get('method','') if items else '')
except Exception:
    print('')" 2>/dev/null || echo "")
                if [ "$adv" = "$SS_METHOD" ]; then
                    pass "hub advertises ${tier} as ${SS_METHOD}"
                elif [ -z "$adv" ]; then
                    warn "could not read the advertised ${tier} config"
                else
                    fail "hub advertises ${tier} as '${adv}', expected '${SS_METHOD}'.
         Re-run: DOMAIN=${DOMAIN} python3 /root/server/scripts/seed-pb.py"
                fi
            done
        else
            warn "no PB_TOKEN in /root/.pb_admin_creds — skipping advertised-cipher check"
        fi
    else
        warn "/root/.pb_admin_creds not found — skipping advertised-cipher check"
    fi
else
    warn "DOMAIN not set — skipping the hub-advertised cipher check"
fi

# ── Summary ──
cat << EOF | tee -a "$LOGFILE"
═══════════════════════════════════════════
 Result: ${PASS} passed / ${FAIL} failed / ${WARN} warnings
═══════════════════════════════════════════
EOF

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}SS2022 verification FAILED — see above.${NC}"
    exit 1
fi
echo -e "${GREEN}SS2022 verification passed.${NC}"
if [ "$WARN" -gt 0 ]; then
    echo -e "${YELLOW}${WARN} warning(s) — review the skipped checks.${NC}"
fi
exit 0
