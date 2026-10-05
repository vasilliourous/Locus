#!/usr/bin/env bash
# Module 02: Shadowsocks Server — 2-tier instances
# Installs ssserver and creates two systemd services:
#   - Eco / Free (port 8443, BBR, TCP only)      — the free tier
#   - Strike / Full (port 8445, BBR, TCP+UDP)    — the merged paid tier
#
# Stealth (port 8444) was RETIRED when the paid ladder was merged into one plan
# (docs/business/04-tiers.md §4.2.4). This module no longer creates its config or
# its unit — but note `write_service` is skip-if-exists, so an ALREADY-DEPLOYED
# box keeps shadowsocks-stealth.service and /etc/shadowsocks/stealth.json and
# will keep serving 8444 with a working password until an operator removes them.
# That is an explicit operator step, not something a re-run does: see
# docs/business/14-risks.md §14.8.
set -euo pipefail

log()  { echo "[02-shadowsocks] $*"; }
warn() { echo "[02-shadowsocks][WARN] $*"; }
fail() { echo "[02-shadowsocks][FAIL] $*"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SS_VERSION="v1.23.0"
SS_BINARY="/usr/local/bin/ssserver"
PASS_FILE="/root/.tier_passwords"

# ── Shadowsocks cipher ──
# Module 00 owns the definition and forwards it via setup.sh's run_module.
# Defaulted here too because this file is runnable standalone
# (`bash 02-shadowsocks.sh`), and without the default a standalone run dies on
# `set -u` at the first ${SS_METHOD} reference. The default must mirror module
# 00's exactly — check-consistency.sh guards the "SS_METHOD is defined in
# modules/00-env.sh" half; this mirrors the value.
: "${SS_METHOD:=2022-blake3-aes-256-gcm}"
case "$SS_METHOD" in
    2022-blake3-aes-256-gcm|2022-blake3-chacha20-poly1305) : "${SS_KEY_BYTES:=32}" ;;
    2022-blake3-aes-128-gcm)                               : "${SS_KEY_BYTES:=16}" ;;
esac
export SS_METHOD SS_KEY_BYTES

# ── Install ssserver if not present ──
install_ssserver() {
    if [ -f "$SS_BINARY" ] && $SS_BINARY --version &>/dev/null; then
        log "ssserver already installed at ${SS_BINARY}"
        return 0
    fi

    log "Downloading shadowsocks-rust ${SS_VERSION}..."
    local tmpdir
    tmpdir=$(mktemp -d)
    cd "$tmpdir"

    local url="https://github.com/shadowsocks/shadowsocks-rust/releases/download/${SS_VERSION}/shadowsocks-${SS_VERSION}.x86_64-unknown-linux-gnu.tar.xz"
    wget -q "$url" -O shadowsocks.tar.xz 2>/dev/null || {
        log "Version ${SS_VERSION} download failed. Falling back to latest stable..."
        # Fetch latest release tag from GitHub API
        local latest_tag
        latest_tag=$(wget -q -O- "https://api.github.com/repos/shadowsocks/shadowsocks-rust/releases/latest" 2>/dev/null | \
            python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])" 2>/dev/null || echo "")
        if [ -n "$latest_tag" ]; then
            SS_VERSION="$latest_tag"
            url="https://github.com/shadowsocks/shadowsocks-rust/releases/download/${SS_VERSION}/shadowsocks-${SS_VERSION}.x86_64-unknown-linux-gnu.tar.xz"
            wget -q "$url" -O shadowsocks.tar.xz 2>/dev/null || fail "Download failed for latest version ${SS_VERSION} too."
        else
            fail "Download failed for ${SS_VERSION} and couldn't fetch latest. Check GitHub releases manually."
        fi
    }

    tar -xf shadowsocks.tar.xz
    cp ssserver "$SS_BINARY"
    chmod +x "$SS_BINARY"
    cd /
    rm -rf "$tmpdir"
    log "✓ ssserver installed"
}

# ── Tier key generation ──
#
# SS2022 keys are base64 of an EXACT number of bytes (SS_KEY_BYTES, set in
# module 00 and derived from SS_METHOD) — not passphrases. The `tr -d '\n'` is
# load-bearing: `openssl rand -base64 N` appends a newline, and a key carrying
# one fails at handshake time with no useful error, which is exactly the kind
# of fault that looks like "the server is down".
gen_tier_key() {
    openssl rand -base64 "${SS_KEY_BYTES:?SS_KEY_BYTES unset — module 00 must run first}" | tr -d '\n'
}

# ── Load or generate tier passwords ──
# Priority: 1) env vars from decrypted secrets  2) existing file on VPS  3) auto-generate
#
# Branch 3 is a DELIBERATE ACT, not a fallback. See module 00 for the
# precondition check and docs/operate/SECRETS-MANAGEMENT.md for why: these passwords
# are fleet-wide, so inventing them on one host silently forks it from every
# other host. It requires ALLOW_GENERATED_TIER_PASSWORDS=1, and when it runs
# the generated values must be written back to secrets.env.age afterwards.
setup_passwords() {
    ECO_PASS="${ECO_PASS:-}"
    STEALTH_PASS="${STEALTH_PASS:-}"
    STRIKE_PASS="${STRIKE_PASS:-}"

    if [ -n "$ECO_PASS" ] && [ -n "$STEALTH_PASS" ] && [ -n "$STRIKE_PASS" ]; then
        log "Using tier passwords from environment (decrypted secrets)."
        TIER_PASSWORDS_GENERATED=0
    elif [ -f "$PASS_FILE" ]; then
        log "Loading existing passwords from ${PASS_FILE}"
        . "$PASS_FILE"
        TIER_PASSWORDS_GENERATED=0
    elif [ "${ALLOW_GENERATED_TIER_PASSWORDS:-0}" = "1" ]; then
        warn "No secrets and no existing password file."
        warn "ALLOW_GENERATED_TIER_PASSWORDS=1 — GENERATING new tier passwords."
        warn "This host now DIVERGES from the fleet until you complete the"
        warn "write-back in docs/operate/SECRETS-MANAGEMENT.md."
        ECO_PASS=$(gen_tier_key)
        STEALTH_PASS=$(gen_tier_key)
        STRIKE_PASS=$(gen_tier_key)
        TIER_PASSWORDS_GENERATED=1
        warn "            (write these back to secrets.env.age, or this host forks)"
        warn "            ECO_PASS=$ECO_PASS"
        warn "            STEALTH_PASS=$STEALTH_PASS"
        warn "            STRIKE_PASS=$STRIKE_PASS"
    else
        # Should be unreachable: module 00 fails the deploy first. Kept as a
        # belt-and-braces guard so this module cannot fork the fleet if it is
        # ever run standalone (`bash 02-shadowsocks.sh`).
        fail "No tier passwords available and ALLOW_GENERATED_TIER_PASSWORDS is not set.
     Refusing to invent fleet-wide credentials. See docs/operate/SECRETS-MANAGEMENT.md."
    fi

    # Persist to file so seed-pb.py and later runs can find them
    cat > "$PASS_FILE" <<EOF
ECO_PASS=$ECO_PASS
STEALTH_PASS=$STEALTH_PASS
STRIKE_PASS=$STRIKE_PASS
EOF
    chmod 600 "$PASS_FILE"

    # Reject a key that SS2022 cannot use. The pre-migration values were
    # `openssl rand -hex 16` (16 ASCII chars), which SS2022 refuses — a host
    # carrying them would look deployed and serve a listener that rejects
    # every handshake. Checked here as well as in module 00 because this file
    # is runnable standalone (`bash 02-shadowsocks.sh`).
    for _k in ECO_PASS STEALTH_PASS STRIKE_PASS; do
        _v="${!_k}"
        _bytes=$(printf '%s' "$_v" | base64 -d 2>/dev/null | wc -c) || _bytes=0
        if [ "$_bytes" -ne "${SS_KEY_BYTES:-32}" ]; then
            fail "${_k} is not a valid ${SS_METHOD:-SS2022} key (${_bytes} decoded bytes).
     Generate:  openssl rand -base64 ${SS_KEY_BYTES:-32} | tr -d '\\n'"
        fi
    done

    # Export for use in config files
    export ECO_PASS STEALTH_PASS STRIKE_PASS
    export TIER_PASSWORDS_GENERATED
    log "✓ Tier passwords ready (eco, strike; stealth retired)"
}

# ── Create Shadowsocks config ──
write_config() {
    local tier="$1"    # the on-box name: eco (free) or strike (full)
    local port="$2"
    local pass_var="$3"
    local mode="$4"    # tcp_only or tcp_and_udp

    local config_file="/etc/shadowsocks/${tier}.json"
    local pass="${!pass_var}"

    if [ -f "$config_file" ]; then
        log "Config ${config_file} already exists"
        # Check if password matches
        CURRENT_PASS=$(python3 -c "import json; print(json.load(open('${config_file}'))['password'])" 2>/dev/null || echo "")
        if [ "$CURRENT_PASS" != "$pass" ]; then
            warn "Password mismatch in ${config_file}. Regenerating."
        else
            return 0
        fi
    fi

    mkdir -p /etc/shadowsocks

    cat > "$config_file" <<EOF
{
  "server": "0.0.0.0",
  "server_port": ${port},
  "password": "${pass}",
  "method": "${SS_METHOD}",
  "mode": "${mode}",
  "fast_open": true,
  "no_delay": true
}
EOF
    log "✓ Created ${config_file} (port ${port}, mode ${mode})"
}

# ── Create systemd service ──
write_service() {
    local tier="$1"
    local desc="$2"
    local extra_execstart="${3:-}"
    local extra_deps="${4:-}"
    local post_start="${5:-}"

    local service_file="/etc/systemd/system/shadowsocks-${tier}.service"

    if [ -f "$service_file" ]; then
        log "Service ${service_file} already exists"
        return 0
    fi

    cat > "$service_file" <<SERVICE
[Unit]
Description=Shadowsocks Server (${desc})
After=network.target${extra_deps}
${extra_deps:+Requires=${extra_deps}}

[Service]
Type=simple
ExecStart=${SS_BINARY} -c /etc/shadowsocks/${tier}.json
${extra_execstart}
Restart=on-failure
RestartSec=5
${post_start:+ExecStartPost=${post_start}}

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created ${service_file}"
}

# ═══════════════════════════════════════════
# Main
# ═══════════════════════════════════════════

install_ssserver
setup_passwords

mkdir -p /etc/shadowsocks

# ── Eco / Free (BBR, TCP only, 1 Mbps tc) ──
# The on-box name stays `eco`; the customer-facing name is Free. See
# docs/business/04-tiers.md §4.5.
write_config "eco" 8443 "ECO_PASS" "tcp_only"
write_service "eco" "Eco/Free — BBR, 1 Mbps tc cap"

# ── Strike / Full (BBR, TCP+UDP, 100 Mbps tc) ──
# The on-box name stays `strike` (a code in the field carries that string, and
# the sing-box UoT endpoint serves these credentials), the customer-facing name
# is Full, and the cap is 100 Mbps — it absorbed Stealth. See §4.6.
write_config "strike" 8445 "STRIKE_PASS" "tcp_and_udp"
write_service "strike" "Strike/Full — BBR, 100 Mbps tc cap, UDP"

# ── Reload systemd ──
systemctl daemon-reload

# ── Enable services (started by setup.sh after all modules complete) ──
systemctl enable shadowsocks-eco.service 2>/dev/null || true
systemctl enable shadowsocks-strike.service 2>/dev/null || true
log "   Services enabled (will start after all modules complete)"
log "   Eco/Free:     :8443 (TCP, BBR)"
log "   Strike/Full:  :8445 (TCP+UDP, BBR)"
log ""
log "   NOTE: shadowsocks-stealth (:8444) is retired. If this box previously ran"
log "   it, the service still exists and must be removed by hand:"
log "     systemctl disable --now shadowsocks-stealth"
log "     rm -f /etc/systemd/system/shadowsocks-stealth.service /etc/shadowsocks/stealth.json"
log ""
log "   Passwords saved to ${PASS_FILE} (chmod 600)"

# ═══════════════════════════════════════════
# UDP-over-TCP (sing-box servers) — ON BY DEFAULT, ONE PER PLAN
#
# Installs a sing-box server per plan, each listening on its own port with that
# plan's own SS2022 credentials, so game/voice UDP can ride inside an allowed
# TCP flow on networks that drop raw UDP (N4L school WiFi).
#
# TWO INSTANCES, NOT ONE, BECAUSE THE LISTENER IS BOUND TO A PASSWORD.
# A sing-box shadowsocks inbound is configured with ONE password. Pointing both
# plans at one listener would mean one plan's clients handshaking with the
# other's credential — so the free plan gets its own listener with ECO_PASS,
# exactly as the paid plan has one with STRIKE_PASS. This also lets each be
# shaped independently (see 04-tc.sh: the free instance carries the same 1 Mbps
# ceiling as the free TCP tier).
#
#   paid (strike) -> UOT_PORT       (default 8446)
#   free (eco)    -> UOT_PORT_FREE  (default 8447)
#
# UDP IS NOT A DIFFERENTIATOR ANY MORE. Both plans carry it; the paid plan is
# differentiated by RATE (100 Mbps vs 1 Mbps), which is the only property that
# actually separates them on this network. See docs/business/04-tiers.md §4.2.4.
#
# ADDITIVE: the shadowsocks-rust TCP services are untouched — TCP traffic never
# uses these instances. Clients only route UDP here when the tier's
# tier_configs config advertises "uot_port", which seed-pb.py does for BOTH
# plans (same defaults).
#
# Disable with ENABLE_UOT=0 (then re-run seed-pb.py so the tiers stop
# advertising uot_port, otherwise clients are told to use a dead port).
# ═══════════════════════════════════════════
UOT_ENABLED="${ENABLE_UOT:-1}"
UOT_PORT="${UOT_PORT:-8446}"
UOT_PORT_FREE="${UOT_PORT_FREE:-8447}"
SING_BOX_VERSION="${SING_BOX_VERSION:-v1.12.1}"
SING_BOX_BINARY="/usr/local/bin/sing-box"

install_singbox() {
    if [ -f "$SING_BOX_BINARY" ] && $SING_BOX_BINARY version &>/dev/null; then
        log "sing-box already installed at ${SING_BOX_BINARY}"
        return 0
    fi

    log "Downloading sing-box ${SING_BOX_VERSION}..."
    local tmpdir
    tmpdir=$(mktemp -d)
    cd "$tmpdir"

    local url="https://github.com/SagerNet/sing-box/releases/download/${SING_BOX_VERSION}/sing-box-${SING_BOX_VERSION#v}-linux-amd64.tar.gz"
    wget -q "$url" -O sing-box.tar.gz 2>/dev/null || {
        log "Version ${SING_BOX_VERSION} download failed. Aborting UoT setup (TCP tiers unaffected)."
        rm -rf "$tmpdir"
        return 1
    }

    tar -xzf sing-box.tar.gz
    cp "sing-box-${SING_BOX_VERSION#v}-linux-amd64/sing-box" "$SING_BOX_BINARY"
    chmod +x "$SING_BOX_BINARY"
    cd /
    rm -rf "$tmpdir"
    log "✓ sing-box installed"
}

# write_uot_config <name> <port> <password>
#
# One listener per plan, because a sing-box shadowsocks inbound is bound to ONE
# password. `name` is both the config filename and the inbound tag, so the two
# instances are distinguishable in `sing-box`'s own logs — which is the only
# place a cross-plan misconfiguration would show up.
write_uot_config() {
    local name="$1"
    local port="$2"
    local password="$3"
    local config_file="/etc/sing-box/${name}.json"
    mkdir -p /etc/sing-box

    cat > "$config_file" <<EOF
{
  "log": { "level": "info" },
  "inbounds": [
    {
      "type": "shadowsocks",
      "tag": "${name}-uot",
      "listen": "::",
      "listen_port": ${port},
      "method": "${SS_METHOD}",
      "password": "${password}"
    }
  ]
}
EOF
    log "✓ Created ${config_file} (port ${port}, ${name} creds, udp_over_tcp)"
    log "  NOTE: 'network' omitted — sing-box defaults to tcp+udp; the value"
    log "  'tcp_and_udp' (shadowsocks-rust syntax) is REJECTED by sing-box."
    log "  NOTE: no udp_over_tcp field on the INBOUND — sing-box <=1.12.1"
    log "  rejects it (unknown field). UoT magic-domain connections are"
    log "  handled automatically by the shadowsocks inbound; the option only"
    log "  exists on the client (outbound) side."
    log "  NOTE: deliberately NO extra tuning keys here. Every field added to"
    log "  this config is a field sing-box <=1.12.1 must already know, and an"
    log "  unknown key makes the daemon refuse to start — taking the whole"
    log "  Strike gaming path down for a cosmetic gain. Transport tuning lives"
    log "  in /etc/sysctl.d/92-udp-gaming.conf (module 01) instead, where the"
    log "  kernel applies it and a bad value cannot stop the listener booting."
    log "  NOTE: SS2022 needs NO version bump. v1.12.1 already supports"
    log "  ${SS_METHOD} (32-byte key), and the shadowsocks inbound has no"
    log "  padding field to set — SS2022 padding is applied internally by the"
    log "  implementation, not via config. Bumping sing-box for this migration"
    log "  would add risk without adding capability; if it is ever bumped, do"
    log "  it as its own change and re-test the UoT path end to end."
}

# write_uot_service <name>
#
# The unit name (`sing-box-uot-<name>.service`) carries the plan so an operator
# reading `systemctl` output can tell which listener they are looking at, and so
# retiring one plan cannot silently stop the other's UDP.
write_uot_service() {
    local name="$1"
    local desc="$2"
    local service_file="/etc/systemd/system/sing-box-uot-${name}.service"

    cat > "$service_file" <<SERVICE
[Unit]
Description=sing-box server (${desc} UDP-over-TCP)
After=network.target

[Service]
Type=simple
ExecStart=${SING_BOX_BINARY} run -c /etc/sing-box/${name}.json
Restart=on-failure
RestartSec=5

# Keep the file-descriptor limit generous.
#
# Every UDP flow a client carries becomes a socket on this listener, and
# UDP-over-TCP multiplexes all of them over relatively few TCP connections. The
# default soft limit is low enough that a handful of gaming clients can exhaust
# it, and the symptom is new UDP flows silently failing while TCP keeps working —
# a "game doesn't connect but the VPN is fine" report that is very hard to
# attribute. Raised here rather than left to the distro default.
#
# Raised on BOTH listeners. The free one carries Roblox/Minecraft-shaped traffic
# from a population that is expected to be much larger than the paid one, so it
# is if anything the more exposed of the two.
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created ${service_file}"
}

if [ "$UOT_ENABLED" = "1" ]; then
    if install_singbox; then
        # Paid first, then free — the historical order, and the paid listener is
        # the one already deployed on an upgraded box.
        write_uot_config "strike" "$UOT_PORT"      "$STRIKE_PASS"
        write_uot_service "strike" "paid"
        write_uot_config "eco"    "$UOT_PORT_FREE" "$ECO_PASS"
        write_uot_service "eco"    "free"
        systemctl daemon-reload
        systemctl enable sing-box-uot-strike.service 2>/dev/null || true
        systemctl enable sing-box-uot-eco.service 2>/dev/null || true
        # START them, not just enable them.
        #
        # `enable` only takes effect at the NEXT boot, and `write_uot_service`
        # rewrites the unit file without systemd being told to act on it. So a
        # module run on a live box that already had a listener leaves it
        # STOPPED: found 2026-10-05, when a deployment created both per-plan
        # units, retired the old one, and left BOTH plans with no UDP at all
        # while every check said "created". The enablements above mean a reboot
        # would have masked it — the failure only appears between deploy and
        # reboot, which is precisely when a student is most likely to notice.
        systemctl start sing-box-uot-strike.service 2>/dev/null || \
            warn "Could not start sing-box-uot-strike.service (:${UOT_PORT}) — start it by hand"
        systemctl start sing-box-uot-eco.service 2>/dev/null || \
            warn "Could not start sing-box-uot-eco.service (:${UOT_PORT_FREE}) — start it by hand"

        # Prove they are actually listening rather than assuming the start
        # worked. A unit that "started" and immediately died is the classic
        # sing-box failure (an unknown config key makes it exit), and the whole
        # point of this block is that UDP dies silently otherwise.
        sleep 1
        for pair in "strike:${UOT_PORT}" "eco:${UOT_PORT_FREE}"; do
            _n="${pair%%:*}"; _p="${pair#*:}"
            if systemctl is-active --quiet "sing-box-uot-${_n}.service"; then
                log "  ✓ sing-box-uot-${_n} is active on :${_p}"
            else
                warn "sing-box-uot-${_n} is NOT running — UDP for that plan is DEAD."
                warn "  Check: journalctl -u sing-box-uot-${_n} -n 20 --no-pager"
            fi
        done
        log "✓ UoT enabled on both plans:"
        log "    paid (strike creds): :${UOT_PORT}"
        log "    free (eco creds):    :${UOT_PORT_FREE}  <- capped at 1 Mbps by 04-tc.sh"
        log "  seed-pb.py advertises \"uot_port\" on BOTH tier rows, so every client"
        log "  picks its own endpoint up on the next heartbeat."

        # Retire the OLD single-listener unit if this box has one. It served the
        # paid plan on the same port the new per-plan unit now uses, so leaving it
        # running would mean two processes fighting for :8446 — one of them bound,
        # the other failing to start and flapping. Best-effort and non-fatal: a
        # fresh box has never had it.
        if [ -f /etc/systemd/system/sing-box-uot.service ]; then
            warn "Retiring the old combined sing-box-uot.service (replaced by the per-plan units)"
            systemctl disable --now sing-box-uot.service 2>/dev/null || \
                warn "Could not stop sing-box-uot.service — stop it by hand or :${UOT_PORT} will flap"
            rm -f /etc/systemd/system/sing-box-uot.service
            systemctl daemon-reload
        fi
    else
        warn "UoT setup aborted (download failed). TCP tiers unaffected."
    fi
else
    log "UoT disabled (ENABLE_UOT=0) — both plans continue on raw UDP."
    log "  NOTE: re-run seed-pb.py so the tiers stop advertising uot_port;"
    log "  otherwise clients are told to use a UDP port nothing is listening on."
fi

exit 0
