#!/usr/bin/env bash
# deploy-console.sh — build the admin console and push it to the live hub.
#
# Usage:
#   VPS=root@host DOMAIN=host server/scripts/deploy-console.sh
#
# Both VPS and DOMAIN are required and have NO default. VPS is the host to
# upload to; DOMAIN is the hostname the script verifies against afterwards.
# They are usually the same host, but are separate because a box can be
# reached by IP before its DNS name is live (set DOMAIN to whatever resolves
# to that box, or the verification step will test a different server).
#
# Why a script: the console is a static SPA that Caddy serves from
# /var/www/admin. Getting a new build there means "build, tar, upload, extract,
# reload" — four steps that are easy to get half-right, and a half-deployed
# console is a blank page. This does all four atomically and verifies the result.
#
# The bundle is uploaded to /root/server/console-dist.tar.gz so that re-running
# `modules/05-caddy.sh` on the host (e.g. during a full setup re-run) picks up
# the same version rather than reverting to whatever was there before.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONSOLE_DIR="${REPO_ROOT}/server/console"
REMOTE_BUNDLE="/root/server/console-dist.tar.gz"
REMOTE_DIR="/var/www/admin"

log()  { printf '\033[0;32m[console]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[console][WARN]\033[0m %s\n' "$*"; }
fail() { printf '\033[0;31m[console][FAIL]\033[0m %s\n' "$*" >&2; exit 1; }

# ── Target must be explicit ──
# This script used to default VPS/DOMAIN to the production DigitalOcean hub.
# That default fails in the worst possible direction: pointed at a new host
# without the env vars set, it would upload the bundle to production AND then
# verify https://<production>/admin/ — which answers 200, so the script
# reported success while having done nothing to the host you asked for.
# Requiring both explicitly makes the target a deliberate choice.
: "${VPS:?VPS is required, e.g. VPS=root@hub.example.com $0}"
: "${DOMAIN:?DOMAIN is required, e.g. DOMAIN=hub.example.com $0}"
log "Target VPS:    ${VPS}"
log "Verify domain: ${DOMAIN}"

[ -d "$CONSOLE_DIR" ] || fail "console directory not found: $CONSOLE_DIR"

# ── Build ──
log "Building console in ${CONSOLE_DIR}"
cd "$CONSOLE_DIR"
if [ ! -d node_modules ]; then
    log "Installing dependencies (first run)"
    npm ci --no-audit --no-fund 2>/dev/null || npm install --no-audit --no-fund
fi
npm run build || fail "console build failed"

[ -f dist/index.html ] || fail "build produced no dist/index.html"

# Sanity: the bundle must reference /admin/ assets, or it will 404 behind the
# /admin path. This is the single easiest thing to break in vite.config.ts.
if ! grep -q '/admin/assets/' dist/index.html; then
    fail "dist/index.html does not reference /admin/assets/ — check 'base' in vite.config.ts"
fi
log "✓ Build OK ($(du -sh dist | cut -f1))"

# ── Package ──
# `-C dist .` packs the CONTENTS of dist/ with index.html at the archive root.
# This level matters: 05-caddy.sh extracts into /var/www/admin and requires
# index.html at the top level. Packing `dist` itself (i.e. `tar czf x dist`)
# yields `./dist/index.html` and fails the deploy with "Console bundle has no
# index.html". Assert it here so the mistake cannot leave this machine.
TARBALL="$(mktemp -t locus-console-XXXXXX.tar.gz)"
trap 'rm -f "$TARBALL"' EXIT
tar czf "$TARBALL" -C dist .
if ! tar tzf "$TARBALL" | grep -qx './index.html'; then
    fail "packed bundle has no ./index.html at its root — wrong directory level.
     Expected: tar czf BUNDLE -C dist .   (NOT: tar czf BUNDLE dist)"
fi
log "✓ Packaged $(basename "$TARBALL") (index.html at root, $(tar tzf "$TARBALL" | wc -l | tr -d ' ') entries)"

# ── Upload ──
log "Uploading to ${VPS}"
scp -q "$TARBALL" "${VPS}:${REMOTE_BUNDLE}" || fail "upload failed"

ssh "$VPS" "
    set -e
    rm -rf '${REMOTE_DIR}'/* 2>/dev/null || true
    mkdir -p '${REMOTE_DIR}'
    tar xzf '${REMOTE_BUNDLE}' -C '${REMOTE_DIR}'
    chown -R root:root '${REMOTE_DIR}'
    chmod -R a+rX '${REMOTE_DIR}'
    systemctl reload caddy 2>/dev/null || systemctl restart caddy
" || fail "remote deploy failed"
log "✓ Deployed to ${REMOTE_DIR}"

# ── Verify ──
# The upload + extract above is the real guarantee: the bundle is on the host
# and every asset is in place. This HTTP check is a BONUS that only means
# something once Caddy is serving the domain — which on a BLANK box is not yet
# true, because that is exactly what the following `setup.sh` run installs.
#
# It must therefore be a warning, not a failure, on that path. It used to be a
# hard `fail`, which produced a genuinely confusing blank-box deploy: the
# operator followed DEPLOY.md's documented order (stage -> deploy-console.sh ->
# setup.sh), the bundle uploaded and extracted correctly, and then the script
# exited non-zero with "console not serving (HTTP 000)" — making a SUCCESSFUL
# console upload look like a failure at the step before the one that would have
# made it serve. See docs/operate/DEPLOY.md "Staging the server tree".
sleep 2
CODE=$(curl -s -o /dev/null -w '%{http_code}' "https://${DOMAIN}/admin/" || echo 000)
if [ "$CODE" != "200" ]; then
    warn "https://${DOMAIN}/admin/ returned HTTP ${CODE}."
    warn "This is EXPECTED on a fresh host where Caddy is not up yet — the bundle"
    warn "is uploaded to ${REMOTE_BUNDLE} and will be served once you run setup.sh."
    warn "If this host is ALREADY running the hub, a non-200 here is a real fault."
    log "✓ Bundle staged. Continuing without the live-serving check."
    exit 0
fi

ASSET=$(curl -s "https://${DOMAIN}/admin/" | grep -o '/admin/assets/[^"]*\.js' | head -1 || true)
if [ -n "$ASSET" ]; then
    ACODE=$(curl -s -o /dev/null -w '%{http_code}' "https://${DOMAIN}${ASSET}" || echo 000)
    [ "$ACODE" = "200" ] || fail "console asset ${ASSET} returned HTTP ${ACODE}"
    log "✓ Serving at https://${DOMAIN}/admin/ (asset OK)"
else
    warn "Could not find an asset reference to verify — check the console manually"
fi

log "Done. Open https://${DOMAIN}/admin/ and sign in with the admin token."
