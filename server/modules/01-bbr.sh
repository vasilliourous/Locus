#!/usr/bin/env bash
# Module 01: BBR + Kernel Tuning
# Enables BBR as the system default congestion control algorithm.
set -euo pipefail

log()  { echo "[01-bbr] $*"; }
warn() { echo "[01-bbr][WARN] $*"; }
fail() { echo "[01-bbr][FAIL] $*"; exit 1; }

log "Configuring BBR congestion control..."

# ── Check if BBR module exists ──
if ! modinfo tcp_bbr &>/dev/null; then
    fail "tcp_bbr module not available in kernel. Expected Ubuntu 22.04+ with default kernel."
fi

# ── Load module now ──
modprobe tcp_bbr 2>/dev/null || warn "tcp_bbr already loaded or failed to load"
log "✓ tcp_bbr module loaded"

# ── Persist module load on boot ──
if [ ! -f /etc/modules-load.d/tcp_bbr.conf ]; then
    echo "tcp_bbr" > /etc/modules-load.d/tcp_bbr.conf
    log "Created /etc/modules-load.d/tcp_bbr.conf"
else
    log "tcp_bbr.conf already exists"
fi

# ── Apply sysctl settings now ──
sysctl -w net.core.default_qdisc=fq 2>/dev/null || warn "Could not set fq qdisc"
sysctl -w net.ipv4.tcp_congestion_control=bbr 2>/dev/null || warn "Could not set BBR immediately"

# ── Persist sysctl settings ──
if [ ! -f /etc/sysctl.d/90-bbr.conf ]; then
    cat > /etc/sysctl.d/90-bbr.conf << 'EOF'
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
EOF
    log "Created /etc/sysctl.d/90-bbr.conf"
    sysctl --system &>/dev/null
else
    log "90-bbr.conf already exists"
fi

# ── Verify ──
CURRENT_CC=$(sysctl -n net.ipv4.tcp_congestion_control 2>/dev/null)
if [ "$CURRENT_CC" = "bbr" ]; then
    log "✓ BBR is active (current CC: ${CURRENT_CC})"
else
    warn "BBR should be active but current CC is: ${CURRENT_CC}"
fi

# ── Additional TCP optimizations ──
SYSCTL_TCP="/etc/sysctl.d/91-tcp-tune.conf"
if [ ! -f "$SYSCTL_TCP" ]; then
    cat > "$SYSCTL_TCP" << 'EOF'
# TCP buffer auto-tuning
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728

# TCP fast open (client + server)
net.ipv4.tcp_fastopen = 3

# Reduce TIME_WAIT sockets
net.ipv4.tcp_fin_timeout = 15

# Increase backlog
net.core.netdev_max_backlog = 5000
net.core.somaxconn = 4096

# TCP keepalive
net.ipv4.tcp_keepalive_time = 300
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.tcp_keepalive_intvl = 15

# Gaming/latency (added 2026-08-14): keep cwnd after idle so games that
# alternate quiet/burst don't re-slow-start every round; MTU probing helps
# if the path fragments. See FIXES.md Strike gaming optimisation.
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_mtu_probing = 1
EOF
    sysctl --system &>/dev/null
    log "Created ${SYSCTL_TCP} with TCP optimizations"
else
    log "TCP tune file already exists"
fi

# ── UDP / gaming transport tuning ──
#
# A SEPARATE FILE ON PURPOSE. The block above is guarded by `[ ! -f … ]`, so on
# any already-deployed host its contents are frozen: editing 91-tcp-tune.conf in
# this repo changes nothing on the live box, which is a trap this project has hit
# before. A new filename has no such guard to defeat, so these settings reach the
# running host on the next `setup.sh` re-run as well as on a fresh deploy.
#
# What each one is for:
#
#   rmem_max / wmem_max — the UDP path is the one that actually carries games,
#     and the UoT listener on 8446 relays every datagram. The socket buffers must
#     be large enough to absorb a burst of small packets without the kernel
#     dropping them; a dropped datagram is invisible to TCP, so it shows up on
#     the device as a rubber-band rather than as an error.
#
#   udp_rmem_min / udp_wmem_min — raise the floor so a quiet game flow keeps a
#     usable buffer. The default floor is small enough that a burst arriving
#     after an idle moment can overflow before the reader drains it.
#
#   tcp_notsent_lowat — caps the bytes the kernel may hold unsent on a TCP
#     socket. The UoT outbound multiplexes every UDP flow over ONE TCP
#     connection, so a large send buffer lets the connection accumulate data
#     that can only drain at path speed; when it stalls, everything behind it
#     stalls too. A lower watermark keeps that queue short, which trades a little
#     throughput for much better latency under loss — the right trade for games
#     and the wrong one for bulk transfer, which is why it is applied here (the
#     hub serves no bulk traffic over 8446).
SYSCTL_UDP="/etc/sysctl.d/92-udp-gaming.conf"
cat > "$SYSCTL_UDP" << 'EOF'
# UDP socket buffers — the game path (Shadowsocks UDP + the UoT listener on 8446)
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.udp_rmem_min = 16384
net.ipv4.udp_wmem_min = 16384

# Keep the UoT TCP connection's unsent queue short: one TCP connection carries
# every multiplexed UDP flow, so head-of-line blocking is the latency cost.
net.ipv4.tcp_notsent_lowat = 131072
EOF
sysctl --system &>/dev/null
log "Created ${SYSCTL_UDP} with UDP/gaming transport tuning"

log "✓ BBR setup complete"
exit 0
