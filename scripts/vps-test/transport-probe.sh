#!/bin/bash
# Full path-MTU map + UoT/SS transport probe against the LIVE hub and the
# candidate test box. Read-only except for /root/.ssh key provisioning, which is
# idempotent. Run via sshp.py --file.
#
# ─────────────────────────────────────────────────────────────────────────────
# THE HUB IS ADDRESSED BY NAME, NEVER BY ADDRESS.
#
# This script used to hardcode the hub's IP in seven places. That IP went stale
# three times (114.23.136.59 → 134.199.155.166 → 170.64.196.179) and the last of
# them stopped existing, so every probe below was quietly measuring nothing —
# "connection refused" and "no route to host" both look like a network finding
# rather than a dead literal. An address is a world claim about a moment; the
# domain is the only stable identifier. See docs/operate/CLAIMS.md §6.
#
# So: resolve the domain once, fail loudly if it does not resolve, and use the
# resolved address only as a variable. The candidate-box addresses ARE literals
# on purpose — those are throwaway test hosts, not the live hub.
# ─────────────────────────────────────────────────────────────────────────────
set +e

HUB_DOMAIN="${HUB_DOMAIN:-networkingguides.duckdns.org}"

# Capture every resolved address (A records), space-separated. Uses getent so it
# follows the system resolver, which is what a client on the box would do.
HUB_IPS="$(getent ahostsv4 "$HUB_DOMAIN" 2>/dev/null | awk '{print $1}' | sort -u | tr '\n' ' ')"
HUB_IP="$(printf '%s' "$HUB_IPS" | awk '{print $1}')"

if [ -z "$HUB_IP" ]; then
  echo "FATAL: $HUB_DOMAIN did not resolve — every hub probe below would be measuring nothing."
  echo "       Refusing to run rather than report a network fault that is really a DNS one."
  echo "       (If the box has just moved, this is the finding: fix DNS first.)"
  exit 2
fi

if [ "$(printf '%s' "$HUB_IPS" | wc -w)" -gt 1 ]; then
  echo "NOTE: $HUB_DOMAIN resolves to multiple addresses: $HUB_IPS"
  echo "      Using $HUB_IP for the single-target probes; /api/health below also runs by name."
fi

echo "[hub] $HUB_DOMAIN -> $HUB_IP   (probed $(date -u +%Y-%m-%dT%H:%M:%SZ))"

KB64="$(cat <<'K'
c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSU1rcVUxWGJIWmh3SlF4dHdEZC90UE5VM2l2MWM3eFk3M21pVE12SGM5a2sgd2hhbGUtc2FuZGJveAo=
K
)"
mkdir -p /root/.ssh && chmod 700 /root/.ssh
if ! grep -qF "whale-sandbox" /root/.ssh/authorized_keys 2>/dev/null; then
  echo "$KB64" | base64 -d >> /root/.ssh/authorized_keys
  echo "[setup] added whale-sandbox key to authorized_keys"
else
  echo "[setup] whale-sandbox key already present"
fi
chmod 600 /root/.ssh/authorized_keys

echo "=== SSH reachability matrix (from candidate box) ==="
# The hub appears here by its RESOLVED address; the other two are throwaway
# candidate test boxes kept as literals on purpose.
for ip in $HUB_IP 114.23.136.183 114.23.136.1; do
  for port in 22 443 8443 8445 8446; do
    if timeout 6 bash -c "exec 3<>/dev/tcp/$ip/$port" 2>/dev/null; then
      echo "  $ip:$port OPEN"
    else
      echo "  $ip:$port --"
    fi
  done
done

echo "=== PATH MTU map (DF sweep, find the ceiling per path) ==="
sweep() {
  local dst="$1" lo="$2" hi="$3"
  local best=0
  for ((s=lo; s<=hi; s+=4)); do
    if ping -M do -s "$s" -c1 -W2 "$dst" >/dev/null 2>&1; then best=$s; else break; fi
  done
  echo "  $dst -> max DF payload $best (path MTU $((best+28)))"
}
sweep 114.23.136.1  1300 1500
sweep 114.23.116.1  1300 1500
sweep 1.1.1.1       1300 1500
sweep 8.8.8.8       1300 1500

echo "=== Path MTU to the LIVE hub ($HUB_DOMAIN -> $HUB_IP) ==="
sweep "$HUB_IP" 1300 1500

echo "=== where the tunnel endpoints differ: TCP MSS advertised by hub:443 ==="
command -v tcpdump >/dev/null && echo "  tcpdump present" || echo "  (no tcpdump)"

echo "=== UoT / SS transport handshake probe ==="
echo "  --- TCP connect + TLS SNI to live hub (by resolved address, cert check off)"
timeout 15 curl -svk -o /dev/null --max-time 12 "https://$HUB_IP/api/health" 2>&1 | grep -E "Connected to|SSL connection|subject:|HTTP/" | sed 's/^/     /'
echo "  --- live hub via its DNS name (the path a client actually takes)"
timeout 15 curl -s -o /dev/null -w "     $HUB_DOMAIN/api/health -> %{http_code} (%{time_total}s)\n" --max-time 12 "https://$HUB_DOMAIN/api/health"

echo "=== UDP-over-TCP reachability to live hub :8446 ==="
# curl cannot speak to :8446 (a shadowsocks UoT listener, not HTTP), so this is a
# raw TCP connect by resolved address.
timeout 8 bash -c "exec 3<>/dev/tcp/$HUB_IP/8446 && echo '  :8446 TCP OPEN'" 2>/dev/null || echo "  :8446 unreachable"

echo "=== raw UDP egress to live hub :8445 (DNS-shaped payload) ==="
HUB_IP="$HUB_IP" python3 - <<'PY'
import os, socket
ip = os.environ["HUB_IP"]
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(5)
try:
    s.sendto(b'\x00'*64, (ip, 8445)); print(f'  UDP send to hub:8445 ({ip}) OK')
except Exception as e: print('  UDP send failed:', e)
PY

echo "=== timing: school-laptop analogue (NZ residential -> endpoints) ==="
echo "  (candidate box is on a residential NZ line; RTT to hub below)"
timeout 10 ping -c4 -q "$HUB_IP" 2>/dev/null | tail -1 | sed 's/^/     /'
timeout 10 ping -c4 -q 1.1.1.1 2>/dev/null | tail -1 | sed 's/^/     /'
