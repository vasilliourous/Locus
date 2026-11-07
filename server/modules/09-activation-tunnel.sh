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

# ── RUN 00-env, DO NOT SOURCE IT ──
#
# 00-env.sh ends with `exit 0` (it is written to be RUN as setup.sh's first
# module). Sourcing a file that calls `exit` terminates the CALLING shell — the
# `|| true` here could not catch it, because `exit` is not a failed command, it
# is the end of this process. Measured 2026-11-07 on the first real deploy of
# the school-network fix: module 09 printed 00-env's validation, then exited 0
# having installed nothing — no unit, no listener, no key. setup.sh reported
# "✓ Module 09-activation-tunnel completed successfully", so the deploy was
# green while the one transport built for the censored network was absent.
#
# This is a FOURTH defect of the same kind as the three in URGENT-FIX-NOW.md,
# and it survived because the tree is green: nothing in a test suite can see a
# module that exits before it starts.
#
# So the validation still runs (it is a real safety check) but as a CHILD
# process, whose exit status is inspected here rather than ending this shell.
# Its log lines still reach the deploy log.
if [ -f "${SCRIPT_DIR}/00-env.sh" ]; then
    if ! bash "${SCRIPT_DIR}/00-env.sh"; then
        fail "00-env.sh environment validation failed — refusing to configure the activation tunnel"
    fi
fi


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

    # ── Prove what was stored is what will be configured ──
    # The key reaches two places that must never disagree: $PASS_FILE (the copy an
    # operator reads to compile into a release) and the `password` field of
    # $CONFIG_FILE (what ssserver actually runs). They are written by separate
    # steps, so a mismatch is silent — clients just fail to connect, with no
    # error anywhere on the hub. A trailing newline or a stray quote in the
    # secrets value is enough to cause it.
    #
    # This compares at write time, not at use time, so it fails before any service
    # is touched. `verify_key_consistency` re-checks the on-disk state after the
    # config exists.
    ACTIVATION_KEY_SHA="$(printf '%s' "$ACTIVATION_TUNNEL_KEY" | sha256sum | cut -d' ' -f1)"
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
import sys
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
#   /api/release-entitlement
#                     the student sign-out record. NOT here on purpose, and it
#                     is the closest call in this list. The case FOR adding it is
#                     real: a student signing out on a network that blocks the
#                     hub by name is in the same situation as one activating.
#                     The case AGAINST wins — this listener's credential is
#                     compiled into every client, so its allowlist must stay as
#                     small as the product allows, and a sign-out already works
#                     WITHOUT the hub (the client forgets the code locally and
#                     tells the student it could not reach us). Adding it would
#                     widen the surface of a public key to make an optional
#                     audit row more reliable.
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
    # Bind BEFORE entering the serve loop, and say so on the log. A forwarder that
    # loses the bind race (ssserver holds 127.0.0.1:$FORWARD_PORT while it starts)
    # dies with a traceback between ssserver's own restarts; without this line that
    # looks exactly like a healthy service that silently refuses everything.
    try:
        server = socketserver.ThreadingTCPServer(LISTEN, Handler)
    except OSError as exc:
        print(f"locus-activation-forwarder: cannot bind {LISTEN[0]}:{LISTEN[1]}: {exc}",
              file=sys.stderr, flush=True)
        raise SystemExit(1)
    print(f"locus-activation-forwarder: listening on {LISTEN[0]}:{LISTEN[1]} -> {UPSTREAM[0]}:{UPSTREAM[1]}",
          flush=True)
    with server:
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

# ── The stored key and the configured key must be the same bytes ──
# $PASS_FILE is what an operator reads to compile the key into a release.
# $CONFIG_FILE's `password` is what ssserver authenticates against. If they differ,
# every client is stranded and nothing on the hub reports an error — the symptom
# is "the tunnel is up but activation never completes", which is indistinguishable
# from a network fault.
#
# Runs AFTER both files exist and BEFORE any service restart, so a mismatch stops
# the deploy with the services still in their previous state.
verify_key_consistency() {
    [ -f "$CONFIG_FILE" ] || fail "verify_key_consistency: ${CONFIG_FILE} does not exist"
    [ -f "$PASS_FILE" ]   || fail "verify_key_consistency: ${PASS_FILE} does not exist"

    local stored configured stored_sha configured_sha
    # Read the file WITHOUT command substitution: `$(cat f)` strips trailing
    # newlines, so a stored key ending in "\n" would hash identically to the clean
    # one and this guard would pass on exactly the corruption it exists to catch.
    # `read -r -d ''` keeps every byte after the value intact.
    stored=""
    IFS= read -r -d '' stored < "$PASS_FILE" || true

    # Read exactly the JSON string value, without sourcing the file or matching a
    # greedy regex across lines.
    configured="$(python3 - "$CONFIG_FILE" <<'PY'
import json, sys
with open(sys.argv[1]) as fh:
    print(json.load(fh)["password"], end="")
PY
)" || fail "verify_key_consistency: could not read password from ${CONFIG_FILE}"

    stored_sha="$(printf '%s' "$stored" | sha256sum | cut -d' ' -f1)"
    configured_sha="$(printf '%s' "$configured" | sha256sum | cut -d' ' -f1)"

    if [ "$stored_sha" != "$configured_sha" ]; then
        fail "activation tunnel key MISMATCH between ${PASS_FILE} and ${CONFIG_FILE}.
  stored (${PASS_FILE}):     sha256 ${stored_sha} (${#stored} bytes)
  configured (${CONFIG_FILE}): sha256 ${configured_sha} (${#configured} bytes)
  Clients carrying the stored value would be refused with no error on the hub.
  Services were NOT restarted. Fix the value in server/secrets.env.age (it must
  be a single base64 line with no trailing whitespace or quotes), re-encrypt,
  and re-run."
    fi

    # A trailing newline is the specific corruption that survives every `cat`-based
    # comparison, so it gets its own message: whitespace in a base64 key is
    # invisible in a diff and in `tail -f`, and it changes the authenticated bytes.
    if printf '%s' "$stored" | grep -qE '[[:space:]]$'; then
        fail "${PASS_FILE} ends in whitespace.
  A Shadowsocks key is the literal bytes: a trailing newline silently authenticates
  a different key. Rewrite it with printf '%s' (not echo), or fix the value in
  server/secrets.env.age and re-encrypt."
    fi

    # Belt and braces: the value that went into the config must be the one sourced
    # in this run, not merely equal to the file we just wrote from it.
    if [ -n "${ACTIVATION_KEY_SHA:-}" ] && [ "$ACTIVATION_KEY_SHA" != "$configured_sha" ]; then
        fail "activation tunnel key was MUTATED between setup_key and write_config
  (expected sha256 ${ACTIVATION_KEY_SHA}, config has ${configured_sha}).
  Services were NOT restarted."
    fi

    log "✓ Activation tunnel key verified: ${PASS_FILE} == ${CONFIG_FILE} (sha256 ${configured_sha:0:12}…)"
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

# Before any service is touched: the key an operator will compile into a release
# and the key ssserver will run must be the same bytes. A mismatch here is silent
# at runtime, so it is checked while everything is still on disk and restorable.
verify_key_consistency

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
