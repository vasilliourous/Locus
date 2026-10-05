#!/usr/bin/env bash
# Module 05: Caddy Reverse Proxy
# Installs Caddy with the caddy-ratelimit plugin and deploys Caddyfile with:
#   - TLS via Let's Encrypt
#   - Rate limiting zones (fingerprint-keyed for activation)
#   - Reverse proxy to PocketBase (127.0.0.1:8090)
#   - Static file serving for update.json
#
# Installation strategy:
#  1. Install base Caddy from official APT repo (for systemd integration)
#  2. Check if rate_limit module exists
#  3. If missing, build custom Caddy with xcaddy (adds caddy-ratelimit plugin)
#  4. Replace the binary with the custom build
set -euo pipefail

log()  { echo "[05-caddy] $*"; }
warn() { echo "[05-caddy][WARN] $*"; }
fail() { echo "[05-caddy][FAIL] $*"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATES_DIR="$(dirname "$SCRIPT_DIR")/templates"
CADDYFILE="/etc/caddy/Caddyfile"
CADDY_DATA_DIR="/var/www/html"

: "${DOMAIN:?DOMAIN is required}"

# ── The public landing page hostname ──
#
# A SECOND hostname on the SAME box, deliberately not the hub's. The hub is
# `networkingguides.duckdns.org` and stays that way: it is baked into every
# deployed client (the activation response hands out `server`, the updater
# resolves `update_url`), so renaming it would strand the fleet. The landing
# page therefore gets its own name, its own document root and its own Caddy
# block — and no access to anything the hub serves.
#
# DEFAULTING IS SAFE HERE, unlike VPS/DOMAIN in the deploy scripts. Those
# defaulted to production and that was the bug (see deploy-console.sh's header):
# pointed at a new host they silently acted on the wrong server. This is the
# opposite shape — it names a hostname that must be served, and getting it wrong
# produces a TLS failure for a name DNS does not point here, loudly, on the box
# you actually ran it on.
#
# Override with LANDING_DOMAIN=... ; set it empty to skip the landing block
# entirely (see the guard around the block below).
LANDING_DOMAIN="${LANDING_DOMAIN:-locusvpn.jadedns.uk}"

# ── Install or rebuild Caddy with rate_limit support ──
install_caddy() {
    # Check if Caddy already has rate_limit
    if command -v caddy &>/dev/null; then
        local ver
        ver=$(caddy version 2>/dev/null | head -1 | cut -d' ' -f1 || echo "unknown")
        log "Caddy already installed (${ver})"
        if [ "$(caddy list-modules 2>/dev/null | grep -c 'http.handlers.rate_limit')" -gt 0 ]; then
            log "✓ rate_limit module present"
            return 0
        fi
        log "rate_limit module missing. Will download custom build..."
    else
        log "Caddy not found. Will download custom build with rate_limit..."
    fi

    # Download pre-built Caddy with ratelimit plugin from official API
    # This is MUCH faster than building from source with xcaddy
    log "Downloading Caddy with ratelimit plugin from caddyserver.com..."

    local tmpfile
    tmpfile=$(mktemp /tmp/caddy-XXXXXX)

    # NOTE: the caddyserver.com download API ignores the version param and
    # serves the latest release (observed 2026-08-14: requested 2.8.4, got
    # 2.11.4), and occasionally returns a build WITHOUT the plugin (same
    # date). The xcaddy fallback below makes this deterministic.
    CADDY_VERSION="2.8.4"
    if ! curl -sL "https://caddyserver.com/api/download?os=linux&arch=amd64&p=github.com/mholt/caddy-ratelimit&version=${CADDY_VERSION}" \
        -o "$tmpfile"; then
        rm -f "$tmpfile"
        fail "Failed to download Caddy with ratelimit plugin"
    fi

    # Verify it's a valid binary
    if ! file "$tmpfile" | grep -q "ELF"; then
        rm -f "$tmpfile"
        fail "Downloaded file is not a valid ELF binary"
    fi

    chmod +x "$tmpfile"
    local ver
    ver=$("$tmpfile" version 2>/dev/null | head -1 || echo "unknown")
    log "Downloaded Caddy ${ver}"

    # Verify ratelimit module is included
    # NOTE: use grep -c (not -q) — under `set -euo pipefail`, grep -q closes
    # the pipe after the first match, caddy gets SIGPIPE, and pipefail turns
    # the whole check into a FALSE NEGATIVE (hit 2026-08-14: a good binary
    # was rejected and setup fell back/aborted).
    if [ "$("$tmpfile" list-modules 2>/dev/null | grep -c 'http.handlers.rate_limit')" -eq 0 ]; then
        log "API build lacks rate_limit module — falling back to xcaddy build (deterministic)..."
        rm -f "$tmpfile"
        if command -v go &>/dev/null; then
            local xcaddy_bin xcaddy_dir
            xcaddy_bin=$(mktemp /tmp/caddy-xcaddy-XXXXXX)
            xcaddy_dir="$(dirname "$xcaddy_bin")"
            log "Building Caddy ${CADDY_VERSION} + caddy-ratelimit with xcaddy (one-time, ~2-4 min)..."
            if GOBIN="$xcaddy_dir" go install github.com/caddyserver/xcaddy/cmd/xcaddy@latest 2>/dev/null \
                && "$xcaddy_dir/xcaddy" build "v${CADDY_VERSION}" --with github.com/mholt/caddy-ratelimit -o "$xcaddy_bin" 2>/dev/null; then
                if [ "$("$xcaddy_bin" list-modules 2>/dev/null | grep -c 'http.handlers.rate_limit')" -gt 0 ]; then
                    log "✓ xcaddy build has rate_limit (v${CADDY_VERSION})"
                    tmpfile="$xcaddy_bin"
                else
                    rm -f "$xcaddy_bin"
                    fail "xcaddy build succeeded but lacks rate_limit module"
                fi
            else
                rm -f "$xcaddy_bin"
                fail "API build lacked rate_limit AND xcaddy build failed. Install manually: xcaddy build v${CADDY_VERSION} --with github.com/mholt/caddy-ratelimit"
            fi
        else
            fail "API build lacked rate_limit and go/xcaddy is unavailable. Install manually: xcaddy build v${CADDY_VERSION} --with github.com/mholt/caddy-ratelimit"
        fi
    fi
    log "✓ rate_limit module confirmed in downloaded binary"

    # Replace the installed Caddy binary
    systemctl stop caddy 2>/dev/null || true
    cp "$tmpfile" /usr/bin/caddy
    chmod +x /usr/bin/caddy
    rm -f "$tmpfile"

    log "✓ Caddy replaced with ratelimit-enabled version: $(caddy version | head -1)"
}

# ── Deploy Caddyfile ──
deploy_caddyfile() {
    mkdir -p /etc/caddy "$CADDY_DATA_DIR"

    cat > "$CADDYFILE" << CADDY
# Locus Caddyfile — generated by modular setup
# Domain: ${DOMAIN}

{
    # rate_limit is configured per-handle below
}

# ── Main site block ──
${DOMAIN} {
    root * ${CADDY_DATA_DIR}
    file_server

    # ── Security headers ──
    header {
        -Server
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        Referrer-Policy "no-referrer"
        Permissions-Policy "geolocation=(), microphone=(), camera=()"
    }

    # ── Activation endpoint (rate limited: 5 per 10 min per IP) ──
    handle /api/activate {
        rate_limit {
            zone activation {
                key {remote_host}
                events 5
                window 10m
            }
        }
        reverse_proxy 127.0.0.1:8090
    }

    # ── Heartbeat endpoint (rate limited: 1 per 10s per IP) ──
    handle /api/heartbeat {
        rate_limit {
            zone heartbeat {
                key {remote_host}
                events 1
                window 10s
            }
        }
        reverse_proxy 127.0.0.1:8090
    }

    # ── Admin console ──
    # The console is a static SPA served at /admin/. A request for /admin
    # (no slash) is redirected so a typed URL or bookmark does not 404.
    #
    # Ordering matters: the more specific /api/admin/* handles must appear
    # BEFORE the general /api/* block, or the generic rate-limited proxy would
    # swallow them into PocketBase and reject them at the 100-requests/10s limit.
    # A redirect from /admin (no trailing slash) to /admin/ is done with an
    # exact-path matcher: putting the exact-path matcher inside a handle proved
    # unreliable here — the file_server handle below is a handle_path and won the
    # match. A route with an explicit path matcher is unambiguous.
    @admin_root path /admin
    redir @admin_root /admin/ permanent

    handle_path /admin/* {
        root * /var/www/admin
        header {
            # The console is an internal tool: never index, never cache the
            # shell (so a redeploy is picked up immediately).
            X-Robots-Tag "noindex, nofollow"
            Cache-Control "no-cache"
            X-Content-Type-Options "nosniff"
            X-Frame-Options "DENY"
        }
        # SPA fallback: any unknown path under /admin/ serves index.html so a
        # refresh on a sub-view still loads the app.
        try_files {path} /index.html
        file_server
    }

    # Release publishing goes to the dedicated fetch service, NOT PocketBase.
    #
    # The console no longer uploads binaries: it asks the hub to pull them from
    # the GitHub Release for a version. The service owns /var/www/updates and
    # returns the verified hashes, which the console then writes into
    # update_config via the releases.publish action.
    #
    # GET is a one-shot signed trigger link (?version&nonce&exp&sig); POST is
    # the console button. Both must reach the same handler.
    handle /api/admin/fetch-release {
        reverse_proxy 127.0.0.1:8091
    }

    # Mint a fresh one-shot trigger link.
    handle /api/admin/fetch-link {
        reverse_proxy 127.0.0.1:8091
    }

    # ── General API (rate limited: 100 per 10s per IP) ──
    handle /api/* {
        rate_limit {
            zone api_default {
                key {remote_host}
                events 100
                window 10s
            }
        }
        reverse_proxy 127.0.0.1:8090
    }

    # ── PocketBase admin UI (no rate limit) ──
    handle /_/* {
        reverse_proxy 127.0.0.1:8090
    }

    # ── Static files ──
    handle /update.json {
        file_server
    }

    # ── Release binaries for the auto-updater ──
    # Layout on disk: /var/www/updates/<version>/<filename>, e.g.
    #   /var/www/updates/1.1.0/locus-linux-amd64
    #   /var/www/updates/1.1.0/locus-linux-amd64.sha256
    # handle_path strips the /updates/ prefix, so root points AT the releases
    # directory. Directory browsing stays off: clients never need to enumerate
    # builds, and listings would expose the naming convention.
    # NOTE: this heredoc is UNQUOTED (it interpolates ${DOMAIN}), so no shell
    # substitution syntax may appear anywhere inside it — bash would execute it
    # as a command while writing the file. Keep comments backtick-free.
    handle_path /updates/* {
        root * /var/www/updates
        header {
            Cache-Control "public, max-age=300"
            X-Content-Type-Options "nosniff"
        }
        # Directory listing is off by default in Caddy; do NOT add a
        # "browse <arg>" directive — Caddy 2 parses that argument as a
        # TEMPLATE FILE path, so both "browse false" and "browse off" fail at
        # request time with HTTP 500 ("open off: no such file or directory").
        file_server
    }

    # ── Logging ──
    log {
        output file /var/log/caddy/access.log
        format json
    }

    # ── TLS via Let's Encrypt ──
    tls {
        protocols tls1.2 tls1.3
        curves x25519 secp384r1
    }
}
CADDY

    # ═══════════════════════════════════════════════════════════════
    # The public landing page — a SECOND, deliberately narrow site block.
    # ═══════════════════════════════════════════════════════════════
    #
    # WHY A SEPARATE BLOCK AND NOT A PATH ON THE HUB
    #
    # The obvious shortcut is to serve this at ${DOMAIN}/ and be done. That
    # would publish the hub's surface at a second, indexable hostname:
    #
    #   * handle_path /updates/* would expose every published build — the raw
    #     updater payloads AND the Windows installer — to anyone who guesses a
    #     version, on a page whose entire purpose is to be crawled. The hub's
    #     /updates/ exists for the updater, not for humans; the existing block's
    #     "do NOT add browse" comment is about the same instinct one step smaller.
    #   * /api/* would be reachable under a hostname whose traffic profile is
    #     "search engines and the public", pointed straight at activation and
    #     heartbeat endpoints that are rate-limited per IP precisely because they
    #     are abuse targets.
    #   * /_/ (the PocketBase admin UI) and /admin/ would be one typo away from
    #     the public site.
    #
    # So the landing host serves exactly one thing: static files from its own
    # document root. No handles, no proxies, no rewrites.
    #
    # WHY THE CSP MATTERS AND WHAT IT COSTS
    #
    # The hub block sets no CSP because it is an API plus an operator console and
    # a policy there would need to be loosened repeatedly. A public page has an
    # easy, strict policy: load nothing. `default-src 'none'` with only 'self'
    # for images, styles and fonts means the page cannot exfiltrate anything, and
    # it is enforced by the browser rather than by review.
    #
    # This is why server/site/ ships NO inline <style> and NO external requests:
    # a stylesheet at ./style.css satisfies `style-src 'self'`, whereas an inline
    # block would need `'unsafe-inline'` and quietly remove most of the policy's
    # value. An external font would be blocked outright — which is also why the
    # page uses the client's own system-font stack instead of a web font. If a
    # future edit adds a font <link> or an inline style, the page does not break
    # gracefully: it loses its typography, visibly, in a console nobody watches.
    #
    # `base-uri 'none'`, `form-action 'none'` and `frame-ancestors 'none'` close
    # the remaining three ways a static page can be made to do something it has
    # no script to do voluntarily.
    if [ -n "${LANDING_DOMAIN}" ]; then
        # QUOTED delimiter (<< 'CADDY'): this block's comments contain
        # backticks as PROSE (`default-src`, `script-src`), and an unquoted
        # heredoc runs command substitution on them — so bash tried to EXECUTE
        # `default-src` and `script-src`, printing
        #   "line 334: default-src: command not found"
        # on every deploy. The Caddyfile itself came out correct (the text
        # survives), which is why it went unnoticed: a cosmetic error on a
        # working output is easy to ignore, and it trains an operator to skim
        # deploy output. The domain is substituted explicitly below instead.
        cat >> "$CADDYFILE" << 'CADDY'

# ══════════════════════════════════════════════════════════════
# Landing page — __LANDING_DOMAIN__
#
# Independent of the hub block above. Nothing here is reachable from the
# hub hostname and nothing on the hub hostname is reachable from here.
# The document root is /var/www/site, deployed by scripts/deploy-site.sh
# and verified by deploy_site() below.
# ══════════════════════════════════════════════════════════════
__LANDING_DOMAIN__ {
    root * /var/www/site
    file_server

    header {
        -Server
        X-Content-Type-Options "nosniff"
        X-Frame-Options "DENY"
        Referrer-Policy "strict-origin-when-cross-origin"
        Permissions-Policy "geolocation=(), microphone=(), camera=()"
        # The page loads nothing but its own origin's files. See the note above
        # the block for why the shipped index.html is written to fit this policy
        # rather than this policy being widened to fit the page.
        #
        # script-src IS SET, AND IT IS NOT 'none'. The page runs no JavaScript,
        # so 'none' is the tempting answer — and it is WRONG here. `default-src`
        # is the fallback for `script-src`, so with default-src 'none' and no
        # script-src of its own, the `application/ld+json` block in index.html is
        # blocked before the browser ever reads it. That block is the structured
        # data naming this as "Locus VPN" for search engines, i.e. the one piece
        # of markup doing SEO work — blocking it is a silent loss of exactly the
        # thing the page exists for, and the browser console that would say so is
        # on a visitor's machine, not ours.
        #
        # A CSP hash would be stricter, but it has to be recomputed on every edit
        # to that JSON block and a stale one fails CLOSED (the data silently
        # stops being read again). 'unsafe-inline' for script-src on a page with
        # NO script and no user input is a far smaller concession than a hash
        # that breaks the build's honesty the first time someone reflows the JSON.
        Content-Security-Policy "default-src 'none'; script-src 'unsafe-inline'; img-src 'self'; style-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
        # NOT noindex. This is the one host here that WANTS to be found — the
        # console's X-Robots-Tag "noindex, nofollow" above is correct for the
        # console and must not be copied to this block.
        #
        # A short cache is deliberate: a redeploy should be visible within
        # minutes without a cache-busting dance, and the page is a few KB.
        Cache-Control "public, max-age=300"
    }

    # A heredoc note, because it is a documented trap in this file: this block
    # is written by a SECOND, UNQUOTED heredoc appended to the same Caddyfile —
    # unquoted so that LANDING_DOMAIN interpolates. That means the same rule that
    # applies to the first half applies here too: bash expands this text before
    # Caddy ever sees it, so a stray dollar-brace expression is a shell command
    # substitution, not a literal, and backticks are a command substitution in
    # both halves. Keep comments backtick-free and include only LANDING_DOMAIN as
    # a variable.
    log {
        output file /var/log/caddy/access.log
        format json
    }

    tls {
        protocols tls1.2 tls1.3
        curves x25519 secp384r1
    }
}
CADDY

        # Substitute the one real value into the block we just appended. Done
        # here rather than inside the heredoc so the heredoc could be QUOTED and
        # its prose comments left alone (see the note above it).
        #
        # `sed -i` on the whole file is safe: `__LANDING_DOMAIN__` appears only
        # in this block, and the pattern is a literal placeholder no Caddy
        # directive could contain.
        sed -i "s|__LANDING_DOMAIN__|${LANDING_DOMAIN}|g" "$CADDYFILE"

        # Prove the substitution happened. A Caddyfile that still contained the
        # placeholder would be written, pass syntax as a weird site address, and
        # silently serve nothing — the same class of silent no-op as the
        # unquoted-heredoc bug this replaces.
        if grep -q '__LANDING_DOMAIN__' "$CADDYFILE"; then
            warn "Caddyfile still contains the __LANDING_DOMAIN__ placeholder — the landing block will not serve"
        fi
        log "✓ Landing page block added for ${LANDING_DOMAIN}"
    else
        log "LANDING_DOMAIN empty — no landing page block written"
    fi

    log "✓ Caddyfile deployed to ${CADDYFILE}"
}

# ── Create update.json placeholder ──
deploy_update_json() {
    local update_file="${CADDY_DATA_DIR}/update.json"
    # Only write if missing or version changed
    if [ ! -f "$update_file" ]; then
        cat > "$update_file" <<'EOF'
{
  "version": "1.0.0",
  "rollout_percent": 0,
  "linux_amd64": null,
  "windows": null,
  "macos_intel": null,
  "macos_arm": null
}
EOF
        log "✓ Created placeholder update.json"
    else
        log "update.json already exists"
    fi
    chmod 644 "$update_file"
}

# ── Create systemd service (if not installed via APT) ──
create_systemd_service() {
    local service_file="/etc/systemd/system/caddy.service"
    if [ -f "$service_file" ]; then
        log "Caddy systemd service already exists"
        return 0
    fi

    # Create caddy user if not exists
    if ! id -u caddy &>/dev/null; then
        useradd --system --no-create-home --shell /usr/sbin/nologin caddy 2>/dev/null || true
    fi

    mkdir -p /var/lib/caddy /var/log/caddy /var/www/html /etc/caddy
    chown -R caddy:caddy /var/lib/caddy /var/log/caddy /var/www/html
    chmod 755 /var/www/html

    cat > "$service_file" << 'SERVICE'
[Unit]
Description=Caddy Web Server
Documentation=https://caddyserver.com/docs/
After=network.target network-online.target
Requires=network-online.target

[Service]
Type=notify
User=caddy
Group=caddy
ExecStart=/usr/bin/caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
ExecReload=/usr/bin/caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
TimeoutStopSec=5s
LimitNOFILE=1048576
LimitNPROC=512
PrivateTmp=true
Environment=XDG_CONFIG_HOME=/var/lib/caddy
Environment=XDG_DATA_HOME=/var/lib/caddy

[Install]
WantedBy=multi-user.target
SERVICE
    log "✓ Created systemd service: ${service_file}"

    # Grant capability to bind to low ports (<1024) as non-root
    setcap cap_net_bind_service=+ep /usr/bin/caddy 2>/dev/null && \
        log "✓ Granted cap_net_bind_service to caddy binary" || \
        warn "Could not set cap_net_bind_service (Caddy will need root to bind :80/:443)"
}

# ── Create the release-binaries directory ──
# Published update artifacts live here, served by the /updates/* handler above.
# Created unconditionally so a fresh deploy does not 404 the auto-updater before
# the first release is published. Ownership stays root:root and the directory is
# world-readable (0755) — these are public release binaries, but the deploy user
# needs write access to push new versions.
deploy_updates_dir() {
    local updates_dir="/var/www/updates"
    local admin_dir="/var/www/admin"
    mkdir -p "$updates_dir" "$admin_dir"
    chown root:root "$updates_dir" "$admin_dir"
    chmod 755 "$updates_dir" "$admin_dir"
    log "✓ Updates directory ready at ${updates_dir}"
    log "✓ Admin console directory ready at ${admin_dir}"
}

# ── Install the release fetch service ──
# Publishing no longer pushes binaries THROUGH the hub; the hub pulls them from
# the GitHub Release for a version. That is what removed the upload size
# problem entirely, so the old locus-upload service is retired here.
#
# The service still binds 127.0.0.1 and is reached only through Caddy, at
# /api/admin/fetch-release (POST from the console, GET for a one-shot signed
# link) and /api/admin/fetch-link (mint a link).
install_fetch_service() {
    local unit="/etc/systemd/system/locus-fetch.service"
    local tmpl
    tmpl="$(dirname "$SCRIPT_DIR")/templates/locus-fetch.service"

    if [ ! -f "$tmpl" ]; then
        fail "templates/locus-fetch.service not found — the release fetcher cannot be installed.
     Releases are published through it, so without it the console's Releases
     page cannot publish anything. Expected at ${tmpl}"
    fi
    # The fetch service itself must exist where the unit expects it.
    if [ ! -f /root/server/scripts/fetch-release.py ]; then
        fail "scripts/fetch-release.py not found at /root/server/scripts — the release
     fetcher cannot be installed. Re-upload the server tree (scp -r server)."
    fi

    # ── Retire the old uploader ──
    # Left running, it would keep binding 127.0.0.1:8091 and the new service
    # would fail to start with "address already in use" — a confusing failure
    # whose cause is a service that no longer has a Caddy route or a UI.
    if systemctl list-unit-files 2>/dev/null | grep -q '^locus-upload\.service'; then
        systemctl disable --now locus-upload 2>/dev/null || true
        rm -f /etc/systemd/system/locus-upload.service
        systemctl daemon-reload
        log "✓ Retired the old locus-upload service (superseded by locus-fetch)"
    fi

    # Only rewrite when the content differs, so a re-run does not restart a
    # service that is happily serving.
    if [ ! -f "$unit" ] || ! cmp -s "$tmpl" "$unit"; then
        cp "$tmpl" "$unit"
        systemctl daemon-reload
    fi
    systemctl enable locus-fetch 2>/dev/null || true
    systemctl restart locus-fetch 2>/dev/null || true
    sleep 2
    if systemctl is-active --quiet locus-fetch; then
        log "✓ Fetch service running on 127.0.0.1:8091"
    else
        fail "Fetch service is NOT running — releases cannot be published.
     Check: journalctl -u locus-fetch -n 20 --no-pager"
    fi
}

# ── Deploy the admin console bundle ──
# The console is part of the standard deployment, so a missing bundle is a
# deployment failure, not a detail: without it /admin/ 404s and every operator
# has to fall back to SSH + sqlite. It used to warn and continue, which made a
# half-deployed hub look finished.
#
# The bundle is a tarball at /root/server/console-dist.tar.gz, produced by
# scripts/deploy-console.sh (which builds it locally with npm and uploads it).
# We cannot build it here: the VPS has no node, and the console source is not
# shipped to the server tree.
deploy_console() {
    local bundle="/root/server/console-dist.tar.gz"
    local target="/var/www/admin"
    if [ ! -f "$bundle" ]; then
        if [ -f "$target/index.html" ]; then
            log "No console bundle provided — keeping the existing console at ${target}"
            return 0
        fi
        fail "Admin console bundle missing at ${bundle} and no console deployed.
     Fix: run  server/scripts/deploy-console.sh  from your workstation
     (it builds the SPA locally and uploads the tarball), then re-run setup.sh.
     To deploy without the console, set SKIP_CONSOLE=1 (not recommended)."
    fi
    mkdir -p "$target"
    # Extract to a temp dir and verify BEFORE touching the live console. A
    # corrupt bundle extracted straight into /var/www/admin would leave a
    # half-written SPA (missing assets, stale index.html) — the console would
    # be broken in a way that looks like a code bug rather than a bad upload.
    local staging
    staging="$(mktemp -d /tmp/locus-console-XXXXXX)"
    if ! tar xzf "$bundle" -C "$staging" 2>/dev/null; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Could not extract ${bundle} — the console bundle is corrupt.
     Re-run server/scripts/deploy-console.sh to rebuild and re-upload it."
    fi
    # A bundle that extracts but has no entrypoint is still a broken console —
    # e.g. a tarball made from the wrong directory level (dist/ vs dist/*).
    if [ ! -f "$staging/index.html" ]; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Console bundle has no index.html.
     The tarball was probably built from the wrong level — it must contain the
     CONTENTS of server/console/dist/, not the dist/ directory itself."
    fi
    # The Vite base must be /admin/ or every asset 404s behind the subpath.
    if ! grep -q '/admin/assets/' "$staging/index.html"; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Console bundle index.html does not reference /admin/assets/.
     Check 'base' in server/console/vite.config.ts — it must be '/admin/'."
    fi
    # Verified — replace the live console.
    find "$target" -mindepth 1 -delete 2>/dev/null
    cp -a "$staging"/. "$target"/
    find "$staging" -type f -delete 2>/dev/null
    find "$staging" -depth -type d -empty -delete 2>/dev/null
    chown -R root:root "$target"
    chmod -R a+rX "$target"
    log "✓ Admin console deployed to ${target}"
}

# ── Deploy the landing page bundle ──
#
# Deliberately the SAME SHAPE as deploy_console() above, for the same reason that
# function's header gives: "upload, extract, serve" is four steps that are easy
# to get half-right, and a half-deployed page is a blank page.
#
# The bundle is a tarball at /root/server/site-dist.tar.gz, produced by
# scripts/deploy-site.sh (which renders the download URLs from the release
# manifest locally and uploads it). We cannot render it here: the renderer reads
# release-artifacts/manifest.json, which is a WORKSTATION artifact and is not
# shipped to the server tree — and, more importantly, the hub must not be the
# thing that decides what a download button points at. See deploy-site.sh.
#
# WHY THIS FAILS HARD WHEN THE CONSOLE WOULD WARN, AND VICE VERSA
#
# Both follow the rule already established here: a missing bundle is a failure
# UNLESS something is already deployed, in which case keeping it is correct. The
# console learned this the hard way (it used to warn-and-continue, which made a
# half-deployed hub look finished). The landing page inherits the fixed shape
# rather than re-deriving it.
deploy_site() {
    local bundle="/root/server/site-dist.tar.gz"
    local target="/var/www/site"
    if [ ! -f "$bundle" ]; then
        if [ -f "$target/index.html" ]; then
            log "No landing bundle provided — keeping the existing page at ${target}"
            return 0
        fi
        fail "Landing page bundle missing at ${bundle} and no page deployed.
     Fix: run  server/scripts/deploy-site.sh  from your workstation
     (it renders the download URLs from the release manifest and uploads the
     tarball), then re-run this module.
     To deploy without the landing page, set LANDING_DOMAIN= (empty) so that no
     landing block is written either — leaving the block in place with no root
     would serve a 404 for a hostname that has a valid certificate."
    fi
    mkdir -p "$target"
    # Extract to a temp dir and verify BEFORE touching the live page, exactly as
    # the console does. A corrupt bundle extracted straight over /var/www/site
    # would leave a partially written page — and because the landing block has
    # no try_files fallback, a missing style.css does not 404 the page, it
    # silently serves unstyled HTML. That is harder to notice than a blank page,
    # which is why the checks below are named rather than counted.
    local staging
    staging="$(mktemp -d /tmp/locus-site-XXXXXX)"
    if ! tar xzf "$bundle" -C "$staging" 2>/dev/null; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Could not extract ${bundle} — the landing bundle is corrupt.
     Re-run server/scripts/deploy-site.sh to rebuild and re-upload it."
    fi

    # An entrypoint, a stylesheet, and the icon: the three files whose absence
    # produces a page that renders but is visibly wrong. The tarball must hold
    # the CONTENTS of server/site/, not the directory itself — the same
    # directory-level mistake the console bundle documents.
    local missing=""
    for f in index.html style.css icon.svg; do
        [ -f "$staging/$f" ] || missing="${missing} ${f}"
    done
    if [ -n "$missing" ]; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Landing bundle is missing:${missing}
     The tarball was probably built from the wrong level — it must contain the
     CONTENTS of server/site/, not the site/ directory itself."
    fi

    # The page must carry all four download slots. deploy-site.sh renders the
    # hrefs; this asserts they SURVIVED the round trip. Without it, a rendering
    # regression ships a page whose buttons are href="#", which looks like a
    # finished page to every check that only asks whether it loaded.
    local slots=0
    for key in linux windows macos_intel macos_arm; do
        grep -q "data-download=\"${key}\"" "$staging/index.html" && slots=$((slots + 1))
    done
    if [ "$slots" -ne 4 ]; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Landing bundle index.html carries ${slots} of 4 download slots.
     Expected one data-download for each of linux, windows, macos_intel, macos_arm
     — these are the frozen platform keys (see check-consistency.sh section 1)."
    fi

    # And they must be real URLs, not the placeholders the source file ships
    # with. This is the check that could not fail if it only counted attributes:
    # four slots pointing at "#" satisfy the loop above.
    # COUNT FIRST, TEST SECOND — and the `|| true` is load-bearing, not
    # defensive noise.
    #
    # This is the pipefail trap this repo has paid for before (see
    # docs/operate/OPS.md on `grep -q` and DEBUGGING-METHOD.md §7). `grep`
    # returns 1 when it finds NOTHING, and `set -o pipefail` makes the whole
    # pipeline inherit that 1 — so the assignment fails under `set -e` and the
    # script dies. Here "found nothing" is the GOOD case (no unwired slots), so
    # the check killed the deploy precisely when it was passing.
    #
    # Observed 2026-10-05: `setup.sh` ran modules 00→04, then died silently
    # inside module 05 with no message and exit 1, because the landing page had
    # correctly-wired download buttons. A deploy that aborts when its own
    # verification SUCCEEDS is the worst shape a guard can have — it was found
    # only by running the module under `bash -x` and seeing the trace stop at
    # `unwired=0`.
    local unwired
    unwired=$(grep -o 'data-download="[a-z_]*" href="#"' "$staging/index.html" 2>/dev/null | wc -l | tr -d ' ' || true)
    unwired="${unwired:-0}"
    if [ "$unwired" != "0" ]; then
        find "$staging" -type f -delete 2>/dev/null
        find "$staging" -depth -type d -empty -delete 2>/dev/null
        fail "Landing bundle has ${unwired} download slot(s) still pointing at '#'.
     The download URLs were not rendered — deploy-site.sh reads
     release-artifacts/manifest.json to build them, so the usual cause is a
     manifest whose platforms block is missing an entry."
    fi

    # Verified — replace the live page.
    find "$target" -mindepth 1 -delete 2>/dev/null
    cp -a "$staging"/. "$target"/
    find "$staging" -type f -delete 2>/dev/null
    find "$staging" -depth -type d -empty -delete 2>/dev/null
    chown -R root:root "$target"
    chmod -R a+rX "$target"
    log "✓ Landing page deployed to ${target} (4 download slots wired)"
}

# ═══════════════════════════════════════════
# Main
# ═══════════════════════════════════════════

install_caddy
create_systemd_service
deploy_caddyfile
deploy_update_json
deploy_updates_dir
install_fetch_service
if [ "${SKIP_CONSOLE:-0}" = "1" ]; then
    log "SKIP_CONSOLE=1 — not deploying the admin console (and not requiring its bundle)"
else
    deploy_console
fi

# The landing page is part of the standard deployment too, but — unlike the
# console — it is GATED ON THE BLOCK EXISTING. LANDING_DOMAIN="" means "this host
# has no landing page", so there is nothing to require a bundle for; requiring
# one anyway would make a deliberate opt-out fail the deploy. The two conditions
# are checked together and not separately, because a block without a page and a
# page without a block are both half-deployed states that look finished.
if [ -n "${LANDING_DOMAIN}" ]; then
    deploy_site
else
    log "LANDING_DOMAIN empty — not deploying the landing page (and not requiring its bundle)"
fi

# ── Enable and start Caddy ──
systemctl daemon-reload
systemctl enable caddy 2>/dev/null || true
systemctl restart caddy 2>&1 | tail -5 || {
    warn "Caddy restart failed. Checking config..."
    caddy validate --config "$CADDYFILE" 2>&1 | tail -5 || true
    journalctl -u caddy -n 10 --no-pager 2>&1 | tail -5 || true
}

# ── Verify ──
sleep 2
if systemctl is-active --quiet caddy; then
    log "✓ Caddy is running"
    log "  Caddy version: $(caddy version 2>/dev/null | head -1)"
    if [ "$(caddy list-modules 2>/dev/null | grep -c 'http.handlers.rate_limit')" -gt 0 ]; then
        log "✓ rate_limit module confirmed active"
    fi
else
    warn "Caddy may not be running. Check: journalctl -u caddy -n 20 --no-pager"
    warn "  Config: ${CADDYFILE}"
    warn "  Test: caddy validate --config ${CADDYFILE}"
fi

# ── Landing page smoke check ──
#
# Reported, never fatal. On a fresh host Let's Encrypt has not issued the
# certificate yet, so a failure here means "not yet", not "broken" — the same
# reasoning deploy-console.sh documents at length for its own HTTP check. The
# bundle is already on disk either way, which is the real guarantee.
if [ -n "${LANDING_DOMAIN}" ]; then
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "https://${LANDING_DOMAIN}/" || echo 000)
    if [ "$code" = "200" ]; then
        log "✓ Landing page served at https://${LANDING_DOMAIN}/"
        # The download surface must NOT be reachable here. This is asserted
        # rather than assumed because it is the one property of the split that a
        # future "just add a handle" edit would remove without any visible
        # symptom on the page itself.
        up=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "https://${LANDING_DOMAIN}/updates/" || echo 000)
        if [ "$up" = "404" ]; then
            log "✓ /updates/ is not exposed on the landing host (404, as intended)"
        else
            warn "https://${LANDING_DOMAIN}/updates/ returned ${up} — expected 404."
            warn "  The landing block must not serve the hub's build tree. Check that"
            warn "  no handle for /updates was added to the ${LANDING_DOMAIN} block."
        fi
    else
        warn "https://${LANDING_DOMAIN}/ returned HTTP ${code}."
        warn "  EXPECTED on a fresh host where the certificate is not issued yet."
        warn "  If the hub is already live, check that DNS points ${LANDING_DOMAIN}"
        warn "  at this host and that Cloudflare's SSL mode is Full (strict)."
    fi
fi

log "✓ Caddy setup complete"
exit 0
