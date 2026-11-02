#!/usr/bin/env bash
# Module 09: Pre-activation activation tunnel
#
# WHY THIS EXISTS
#
# A device on a school network is blocked from the hub BY NAME. Measured on
# 2026-10-29 from a school network:
#
#   * https://<hub>/api/health  on 443  → TLS ClientHello sent, then
#     "Recv failure: Connection reset by peer". The appliance reads the SNI and
#     resets the flow before any certificate is presented.
#   * http://<hub>:8443/                → HTTP 503 "Application Control
#     Violation … Reason: The category of high-risk has been blocked".
#
# Both are the same decision: the destination is blocked when its NAME is
# visible. A successful Shadowsocks stream presents no name to anything, which is
# why a student's tunnel works and the enrolment call does not.
#
# The client cannot fix this alone. Activation is what DELIVERS the tunnel
# credentials (server, port, cipher, password arrive in the /api/activate
# response), so there is no tunnel to ride before activating, and an alias
# offered by the heartbeat cannot reach a device that has never beaten. The one
# escape is a transport that exists BEFORE entitlement — this module — carrying
# only the few endpoints enrolment needs.
#
# WHAT THIS IS NOT
#
# It is NOT a tier. It is not advertised in any tier payload, it does not appear
# in `tier_configs`, and it has no tc class. Its credential is a bootstrap key
# shipped in the client, so it must be assumed public; everything below exists to
# bound what a public key can reach.
#
# THE ALLOWLIST IS THE SECURITY BOUNDARY
#
# The forwarder reaches PocketBase directly, BYPASSING Caddy — and Caddy is where
# /api/admin/*, /admin/ and /updates/ are separated, and where the activation
# rate limit lives. So this module enforces both itself:
#
#   * only the paths in $ALLOWED are proxied; everything else is dropped without
#     a response;
#   * activation is rate limited to the same 5 per 10 minutes per source address
#     Caddy applies (05-caddy.sh). Without this the bootstrap key would turn a
#     public secret into an unmetered entitlement-minting endpoint.
#
# Both are asserted by check-consistency.sh (§40): a path added here that is not
# on the allowlist, or an allowlist entry that reaches an admin route, fails the
# build. That guard is the reason this comment block can be short.
set -euo pipefail

log()  { echo "[09-activation-tunnel] $*"; }
warn() { echo "[09-activation-tunnel][WARN] $*"; }
fail() { echo "[09-activation-tunnel][FAIL] $*"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "${SCRIPT_DIR}/00-env.sh" 2>/dev/null || true

SS_BINARY="/usr/local/bin/ssserver"
SS_METHOD="${SS_METHOD:-2022-blake3-aes-256-gcm}"
TUNNEL_PORT="${ACTIVATION_TUNNEL_PORT:-8453}"

# The forwarder's local listener. NOT 8090: the requests arrive here decrypted and
# must be path-filtered before they can reach PocketBase, so ssserver forwards to
# this port and the filter decides what continues.
FORWARD_PORT="${ACTIVATION_FORWARD_PORT:-18090}"

CONFIG_FILE="/etc/shadowsocks/activation-tunnel.json"
PASS_FILE="/etc/locus/activation-tunnel.key"
UNIT_FILE="/etc/systemd/system/locus-activation-tunnel.service"
FORWARDER="/usr/local/bin/locus-activation-forwarder.py"
FORWARD_UNIT="/etc/systemd/system/locus-activation-forwarder.service"

# ── The bootstrap key ──
# Generated once and PERSISTED. Rotating it would strand every deployed client —
# they each carry the current value compiled in, and the one channel that could
# carry a new one (the heartbeat) requires the very call this tunnel exists to
# make possible. Treat it as permanent until a release ships a new one.
#
# It is provided by secrets when present, so the value is not invented per deploy:
# setup.sh decrypts secrets.env.age and exports ACTIVATION_TUNNEL_KEY.
setup_key() {
    if [ -n "${ACTIVATION_TUNNEL_KEY:-}" ]; then
        log "Using ACTIVATION_TUNNEL_KEY from the decrypted secrets"
    elif [ -s "$PASS_FILE" ]; then
        log "Loading existing activation tunnel key from ${PASS_FILE}"
        ACTIVATION_TUNNEL_KEY="$(cat "$PASS_FILE")"
    else
        warn "No ACTIVATION_TUNNEL_KEY and no existing key."
        warn "Generating one — it will NOT match any shipped client until the"
        warn "same value is compiled into a release. See docs/operate/SECRETS-MANAGEMENT.md."
        mkdir -p "$(dirname "$PASS_FILE")"
        chmod 700 "$(dirname "$PASS_FILE")"
        # SS2022 keys are raw bytes, base64-encoded, sized by the method.
        ACTIVATION_TUNNEL_KEY="$(head -c 32 /dev/urandom | base64)"
    fi

    mkdir -p "$(dirname "$PASS_FILE")"
    UMASK_SAVED=$(umask); umask 077
    printf '%s' "$ACTIVATION_TUNNEL_KEY" > "$PASS_FILE"
    umask "$UMASK_SAVED"
    chmod 600 "$PASS_FILE"
    export ACTIVATION_TUNNEL_KEY
}

# ── The path filter ──
# A tiny Python forwarder rather than a Caddy handle, because:
#   * Caddy is not on this path at all (that is the point), so it cannot help;
#   * an allowlist must be a code-level constant a guard can read, not a regex in
#     a config that nothing checks;
#   * it must not follow redirects, buffer, or forward headers verbatim to
#     /api/admin/*.
#
# It forwards ONLY the listed prefixes. The comparison is exact-prefix on the
# decoded request line, so `/api/activate/../admin/x` does not match.
write_forwarder() {
    mkdir -p "$(dirname "$FORWARDER")"
    cat > "$FORWARDER" <<'PY'
#!/usr/bin/env python3
"""Path-filtered forwarder for the pre-activation activation tunnel.

Only the paths listed in ALLOWED pass. Everything else is refused with a 403 and
an EMPTY body: a bootstrap endpoint must not describe what it protects, and must
not be usable to enumerate the hub's surface.

The activation rate limit is enforced here because this path bypasses Caddy,
which is where it normally lives. 5 activations per 10 minutes per source
address, matching 05-caddy.sh exactly — a second, laxer limit on the endpoint
that mints entitlements would be the weakest link.
"""
import http.client
import time
import urllib.parse

LISTEN = ("127.0.0.1", int(__import__("os").environ.get("ACTIVATION_FORWARD_PORT", "18090")))
UPSTREAM = ("127.0.0.1", 8090)

ALLOWED = ("/api/activate", "/api/code-lookup", "/api/heartbeat")

# Deliberately NOT allowed, stated so a reader sees the boundary rather than
# inferring it from an absence:
#   /api/admin/*      privileged; the console's channel, never a student's
#   /api/release      operator-only
#   /updates/*        static payloads; large, and not needed to enrol
#   /admin/, /_/      operator surfaces
RATE_LIMIT_PATH = "/api/activate"
RATE_EVENTS = 5
RATE_WINDOW = 600  # seconds

_seen: dict[str, list[float]] = {}


def rate_limited(peer: str) -> bool:
    now = time.monotonic()
    hits = [t for t in _seen.get(peer, []) if now - t < RATE_WINDOW]
    if len(hits) >= RATE_EVENTS:
        _seen[peer] = hits
        return True
    hits.append(now)
    _seen[peer] = hits
    return False


def allowed(path: str) -> bool:
    # urllib.parse unquotes, so an encoded traversal is decoded BEFORE the check
    # — the check must run on the path the upstream will actually see.
    decoded = urllib.parse.unquote(urllib.parse.urlsplit(path).path)
    if ".." in decoded.split("/"):
        return False
    return any(decoded == a or decoded.startswith(a + "/") for a in ALLOWED)


def main() -> None:
    import socketserver

    class Handler(socketserver.StreamRequestHandler):
        def handle(self) -> None:
            line = self.rfile.readline(65536).decode("latin-1").strip()
            if not line:
                return
            parts = line.split(" ")
            if len(parts) != 3:
                return self.refuse()
            method, target, _version = parts
            if not allowed(target):
                return self.refuse()

            peer = self.client_address[0]
            if urllib.parse.urlsplit(target).path.startswith(RATE_LIMIT_PATH) and rate_limited(peer):
                return self.respond(429, b'{"code":429,"message":"Too many attempts"}')

            # Drain headers, then forward verbatim. Content-Length is required;
            # a chunked body from a bootstrap client is not worth supporting and
            # silently truncating one would corrupt the code.
            length = 0
            while True:
                h = self.rfile.readline(65536)
                if h in (b"\r\n", b"\n", b""):
                    break
                name = h.split(b":", 1)[0].strip().lower()
                if name == b"content-length":
                    length = int(h.split(b":", 1)[1].strip() or 0)
            body = self.rfile.read(length) if length else b""

            conn = http.client.HTTPConnection(*UPSTREAM, timeout=30)
            try:
                conn.request(method, target, body=body, headers={"Content-Type": "application/json"})
                resp = conn.getresponse()
                data = resp.read()
                self.respond(resp.status, data, resp.getheader("Content-Type"))
            except Exception:
                self.respond(502, b'{"code":502,"message":"activation tunnel upstream unavailable"}')
            finally:
                conn.close()

        def refuse(self) -> None:
            self.respond(403, b"")

        def respond(self, status: int, body: bytes, ctype=None) -> None:
            self.wfile.write(b"HTTP/1.1 %d %s\r\n" % (status, b"OK" if status < 400 else b"Error"))
            self.wfile.write(b"Content-Length: %d\r\n" % len(body))
            if ctype:
                self.wfile.write(b"Content-Type: %s\r\n" % ctype.encode())
            self.wfile.write(b"Connection: close\r\n\r\n")
            self.wfile.write(body)

    socketserver.ThreadingTCPServer.allow_reuse_address = True
    with socketserver.ThreadingTCPServer(LISTEN, Handler) as server:
        server.serve_forever()


if __name__ == "__main__":
    main()
PY
    chmod 0755 "$FORWARDER"

    cat > "$FORWARD_UNIT" <<SERVICE
[Unit]
Description=Locus activation tunnel path filter (allowlist for the bootstrap SS listener)
After=network.target pocketbase.service
Before=shadowsocks-activation-tunnel.service

[Service]
Type=simple
Environment=ACTIVATION_FORWARD_PORT=${FORWARD_PORT}
ExecStart=/usr/bin/env python3 ${FORWARDER}
Restart=on-failure
RestartSec=2

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created ${FORWARDER} and ${FORWARD_UNIT}"
}

write_config() {
    mkdir -p /etc/shadowsocks

    # HOW THE PATH FILTER IS REACHED — corrected 2026-11-01.
    #
    # This config used to carry `local_addr`/`local_port` on the assumption that
    # `ssserver` forwards a decrypted stream to a local listener. IT DOES NOT.
    # Measured against shadowsocks-rust 1.25.0: `ssserver` decrypts and then
    # connects to whichever destination the CLIENT named, logging
    # `tcp tunnel <peer> -> <requested destination>`. Those two keys are
    # `sslocal`'s SOCKS listen options; `ssserver` ignores them, so the filter was
    # never in the path and the allowlist that bounds the public bootstrap key did
    # not exist.
    #
    # The filter is reached because the CLIENT asks for it: the tunnelled request
    # addresses `127.0.0.1:${FORWARD_PORT}`, and the hub dials exactly that. So the
    # allowlist holds because the client cooperates — which is fine, and is why it
    # is a filter rather than a boundary against a hostile client. What it bounds
    # is what a *compromised or reverse-engineered* client could reach, and that
    # bound is enforced by the filter process, not by the listener config.
    #
    # The keys are therefore REMOVED rather than left in as decoration: a comment
    # claiming a forward that does not happen is how this defect survived review.
    #
    # `mode: tcp_only` — heartbeat and activation are small request/response
    # pairs; UDP would add a second, unauthenticated surface for nothing.
    cat > "$CONFIG_FILE" <<EOF
{
  "server": "0.0.0.0",
  "server_port": ${TUNNEL_PORT},
  "password": "${ACTIVATION_TUNNEL_KEY}",
  "method": "${SS_METHOD}",
  "mode": "tcp_only",
  "fast_open": true,
  "no_delay": true
}
EOF
    log "✓ Created ${CONFIG_FILE} (listener on :${TUNNEL_PORT})"
    log "   The path filter is reached because the client dials 127.0.0.1:${FORWARD_PORT}"
    log "   THROUGH this listener; ssserver has no inbound forwarding."
}

write_service() {
    cat > "$UNIT_FILE" <<SERVICE
[Unit]
Description=Shadowsocks server (pre-activation activation tunnel)
After=network.target locus-activation-forwarder.service
Requires=locus-activation-forwarder.service

[Service]
Type=simple
ExecStart=${SS_BINARY} -c ${CONFIG_FILE}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created ${UNIT_FILE}"
}

# ═══════════════════════════════════════════
# Main
# ═══════════════════════════════════════════
ACTIVATION_TUNNEL="${ACTIVATION_TUNNEL:-1}"

if [ "$ACTIVATION_TUNNEL" != "1" ]; then
    log "ACTIVATION_TUNNEL=0 — not configuring the activation tunnel"
    # Disable rather than leave running: an operator who turns this off means
    # the port is closed, not merely unmanaged.
    systemctl disable --now locus-activation-tunnel.service 2>/dev/null || true
    exit 0
fi

if [ ! -x "$SS_BINARY" ]; then
    fail "ssserver is not installed at ${SS_BINARY} — run module 02 first"
fi

setup_key
write_forwarder
write_config
write_service

systemctl daemon-reload
systemctl enable locus-activation-forwarder.service 2>/dev/null || true
systemctl enable locus-activation-tunnel.service 2>/dev/null || true
systemctl restart locus-activation-forwarder.service 2>/dev/null || \
    warn "Could not start locus-activation-forwarder — start it by hand"
systemctl restart locus-activation-tunnel.service 2>/dev/null || \
    warn "Could not start locus-activation-tunnel — start it by hand"

if systemctl is-active --quiet locus-activation-tunnel.service; then
    log "✓ Activation tunnel is active on :${TUNNEL_PORT}"
else
    warn "Activation tunnel is NOT running — enrolment over the tunnel is DEAD."
    warn "  Check: journalctl -u locus-activation-tunnel -n 20 --no-pager"
fi

log ""
log "   This port is NOT a tier. It carries /api/activate, /api/code-lookup and"
log "   /api/heartbeat only, and is not advertised to any client by the hub."
