#!/usr/bin/env bash
# Locus VPS Restore — Provision + restore from B2 backup
# Usage:
#   From-scratch restore (age key file on VPS):
#     scp -r server age-key.txt root@new-vps:/root/server/
#     ssh root@new-vps "/root/server/restore.sh"
#
#   From-scratch restore (pipe with AGE_KEY):
#     AGE_KEY=$(cat age-key.txt) \
#       ssh root@new-vps 'bash -s' < restore.sh
#
#   Manual override (no secrets file):
#     B2_APPLICATION_KEY_ID=xxx \
#     B2_APPLICATION_KEY=xxx \
#     B2_BUCKET=my-vpn-backup-bucket \
#     DOMAIN=networkingguides.duckdns.org \
#     ssh root@new-vps 'bash -s' < restore.sh
#
#   Use specific backup:
#     ... BACKUP_PATH=backups/data-20260724-143000.db ... restore.sh
#
#   Automatic latest backup:
#     ... restore.sh   # (finds latest in B2 bucket automatically)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOGFILE="/var/log/myvpn-restore.log"

# ── Colors ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

log()  { echo -e "${GREEN}[$(date +%H:%M:%S)]${NC} $*" | tee -a "$LOGFILE"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*" | tee -a "$LOGFILE"; }
fail() { echo -e "${RED}[FAIL]${NC} $*" | tee -a "$LOGFILE"; exit 1; }

# ── Age-encrypted secrets auto-decrypt (same logic as setup.sh) ──
# Decrypts secrets.env.age so B2 credentials and DOMAIN are available.
#
# FIX: Uses temp file instead of process substitution (source <(cmd)).
# Process substitution is unreliable in non-interactive SSH sessions —
# variables may appear sourced (exit 0) without actually loading.
# See docs/reference/FIXES.md entry S9 for details.
SECRETS_FILE="${SCRIPT_DIR}/secrets.env.age"

# Ensure age is installed before attempting decryption
if [ -f "$SECRETS_FILE" ]; then
    if ! command -v age &>/dev/null; then
        log "Installing age (required for secrets decryption)..."
        apt-get update -qq && apt-get install -y -qq age 2>/dev/null || \
            fail "Failed to install age. Install manually: apt-get install age"
    fi

    log "Decrypting ${SECRETS_FILE}..."
    SECRETS_TMP="/tmp/.secrets-$$.env"
    if [ -n "${AGE_KEY:-}" ]; then
        echo "$AGE_KEY" > "/tmp/age-key-$$.tmp"
        age -d -i "/tmp/age-key-$$.tmp" "$SECRETS_FILE" > "$SECRETS_TMP" 2>/dev/null || \
            { rm -f "/tmp/age-key-$$.tmp" "$SECRETS_TMP"; fail "Failed to decrypt secrets. Check AGE_KEY."; }
        rm -f "/tmp/age-key-$$.tmp"
    elif [ -f "${SCRIPT_DIR}/age-key.txt" ]; then
        age -d -i "${SCRIPT_DIR}/age-key.txt" "$SECRETS_FILE" > "$SECRETS_TMP" 2>/dev/null || \
            { rm -f "$SECRETS_TMP"; fail "Failed to decrypt secrets. Check age-key.txt."; }
    else
        cat >&2 << 'KEYERR'

╔══════════════════════════════════════════════════════════════╗
║  FATAL: secrets.env.age found but no age key available.     ║
║                                                            ║
║  Provide the key in one of these ways:                     ║
║    1. Set AGE_KEY environment variable (CI/CD / pipe)      ║
║    2. Place age-key.txt in the server/ directory            ║
║                                                            ║
║  See SECRETS-MANAGEMENT.md in docs/operate/ for setup.     ║
╚══════════════════════════════════════════════════════════════╝
KEYERR
        fail "No age key available for secrets.env.age"
    fi

    # Source from temp file (reliable in all shell environments)
    set -a
    source "$SECRETS_TMP" || { rm -f "$SECRETS_TMP"; fail "Failed to source decrypted secrets."; }
    set +a
    rm -f "$SECRETS_TMP"
    log "✓ Secrets decrypted from ${SECRETS_FILE}"
fi

: "${DOMAIN:?DOMAIN is required}"
export DOMAIN
: "${B2_APPLICATION_KEY_ID:?B2 application key ID required}"
: "${B2_APPLICATION_KEY:?B2 application key required}"
: "${B2_BUCKET:?B2 bucket name required}"
: "${BACKUP_PATH:=}"  # Optional: specific backup path. Auto-detects latest if empty.

log "══════════════════════════════════════════"
log " Locus VPS — Full Restore from B2 Backup"
log " Domain: ${DOMAIN}"
log " Bucket: ${B2_BUCKET}"
log "══════════════════════════════════════════"

# ── Step 1: Run full setup ──
log "Step 1/4: Provisioning VPS with full setup..."
FORCE=1 bash "${SCRIPT_DIR}/setup.sh" | tee -a "$LOGFILE"

# ── Step 2: Install b2 CLI and authenticate ──
log "Step 2/4: Authenticating to Backblaze B2..."
if ! command -v b2 &>/dev/null; then
    pip3 install b2 2>/dev/null || {
        apt-get update -qq && apt-get install -y -qq python3-pip
        pip3 install b2
    }
fi

b2 authorize-account "$B2_APPLICATION_KEY_ID" "$B2_APPLICATION_KEY" 2>&1 | tee -a "$LOGFILE"

# ── Step 3: Download and restore latest backup ──
log "Step 3/4: Downloading backup..."

if [ -z "$BACKUP_PATH" ]; then
    log "No specific backup given. Finding latest in b2://${B2_BUCKET}/backups/..."
    # NOTE: current b2 CLI requires b2:// URIs; plain bucket names fail with
    # "Invalid B2 URI". Only .db.gz files are backups (.sha256 companions ignored).
    BACKUP_PATH=$(b2 ls --recursive "b2://${B2_BUCKET}/backups/" 2>/dev/null | grep '\.db\.gz$' | sort | tail -1)
    if [ -z "$BACKUP_PATH" ]; then
        fail "No backups found in b2://${B2_BUCKET}/backups/. Check B2 credentials and bucket name."
    fi
    log "Latest backup: ${BACKUP_PATH}"
fi

# Download to temp location.
#
# Backups are stored gzipped (`.db.gz`), and the `.sha256` companion hashes the
# GZIPPED bytes — so the checksum is verified against the download, and only
# then is the file decompressed for the SQLite checks below. Doing it in this
# order matters: hashing the decompressed bytes against a gzip-era checksum
# would fail on a perfectly good backup.
TMP_DIR=$(mktemp -d)
RESTORE_GZ="${TMP_DIR}/restore.db.gz"
RESTORE_FILE="${TMP_DIR}/restore.db"
SHA256_FILE="${TMP_DIR}/restore.sha256"

log "Downloading ${BACKUP_PATH}..."
b2 file download "b2://${B2_BUCKET}/${BACKUP_PATH}" "$RESTORE_GZ" 2>&1 | tee -a "$LOGFILE"

# Try to download SHA256 if it exists
SHA256_PATH="${BACKUP_PATH}.sha256"
if b2 file info "b2://${B2_BUCKET}/${SHA256_PATH}" &>/dev/null; then
    b2 file download "b2://${B2_BUCKET}/${SHA256_PATH}" "$SHA256_FILE" 2>&1 | tee -a "$LOGFILE"
    log "Verifying SHA256 checksum..."
    EXPECTED=$(cat "$SHA256_FILE" | awk '{print $1}')
    ACTUAL=$(sha256sum "$RESTORE_GZ" | awk '{print $1}')
    if [ "$EXPECTED" != "$ACTUAL" ]; then
        fail "Checksum mismatch! Expected ${EXPECTED}, got ${ACTUAL}. Backup may be corrupted."
    fi
    log "✓ Checksum verified."
else
    warn "No SHA256 file found for backup. Skipping verification."
fi

# ── Decompress ──
#
# NEW (2026-10-02): the backup script writes `data-<ts>.db.gz`, but this script
# used to check the SQLite magic number on and install the still-gzipped file —
# so every restore failed with "Downloaded file is not a valid SQLite database"
# and, on the backups whose first bytes happened to look plausible, would have
# installed a gzip stream as data.db. Only the checksum above is over the
# compressed bytes; everything after this point needs the raw database.
case "$BACKUP_PATH" in
    *.gz)
        log "Decompressing $(basename "$BACKUP_PATH")..."
        if ! gunzip -c "$RESTORE_GZ" > "$RESTORE_FILE" 2>/dev/null; then
            fail "Could not gunzip ${BACKUP_PATH}. Backup may be corrupted or not gzip."
        fi
        log "✓ Decompressed to $(stat -c%s "$RESTORE_FILE" 2>/dev/null || echo '?') bytes."
        ;;
    *)
        # A plain .db backup (older layout, or a hand-placed file). Use as-is.
        mv "$RESTORE_GZ" "$RESTORE_FILE"
        ;;
esac

# ── Step 4: Restore PocketBase database ──
log "Step 4/4: Restoring PocketBase database..."

PB_DATA_DIR="/opt/pocketbase/pb_data"

# Verify the (now decompressed) backup is a valid SQLite database
if ! head -c 16 "$RESTORE_FILE" | grep -q "SQLite format 3"; then
    fail "Downloaded file is not a valid SQLite database. Backup may be corrupted."
fi

systemctl stop pocketbase
sleep 1

# Backup current (fresh) DB just in case
if [ -f "${PB_DATA_DIR}/data.db" ]; then
    cp "${PB_DATA_DIR}/data.db" "${PB_DATA_DIR}/data.db.pre-restore"
    log "Backed up fresh DB to data.db.pre-restore"
fi

cp "$RESTORE_FILE" "${PB_DATA_DIR}/data.db"
# Stale WAL/shm from the fresh-seeded DB would be replayed against the
# restored file (corruption/lost data) — remove them.
rm -f "${PB_DATA_DIR}/data.db-wal" "${PB_DATA_DIR}/data.db-shm"
chown pocketbase:pocketbase "${PB_DATA_DIR}/data.db"
chmod 600 "${PB_DATA_DIR}/data.db"

# ── Reconcile migrations (NEW 2026-10-02) ──
#
# WHY THIS EXISTS
#
# setup.sh seeds a FRESH database (via seed-pb.py) and stamps its own migration
# files into /opt/pocketbase/pb_migrations/. The backup we just installed came
# from a database seeded by an EARLIER seed-pb.py, whose migrations were named
# differently (e.g. `1790683077_created_codes.js` vs the current
# `1790900083_created_codes.js`). PocketBase compares migrations by FILENAME, so
# on the next start it sees every current-named file as unapplied, tries to
# CREATE a collection that the restored DB already has, and dies:
#
#   Error: Failed to apply migration 1790900083_created_code_events.js:
#   constraint failed: UNIQUE constraint failed: _collections.name (2067)
#
# The service then sits inactive and the hub is dead, which is a hard failure of
# exactly the command whose job is disaster recovery.
#
# THE FIX, AND WHY IT IS PER-MIGRATION
#
# A blanket "mark everything applied" would be wrong: a genuinely NEW collection
# (one the restored DB predates) must still be created, or the hooks that use it
# fail. So each on-disk migration is inspected: if the collection it creates
# already exists, the migration is marked applied; if not, it is left for
# PocketBase to run for real. Migrations that are not "create collection"
# (indexes, data fixes) are conservatively marked applied only when their file
# name already appears in the restored DB's _migrations — never guessed.
#
# This reads the *collection names* out of each migration file, which is stable:
# both generations create the same base collections, and the difference is the
# filename, not the intent.
if ls /opt/pocketbase/pb_migrations/*.js >/dev/null 2>&1; then
    log "Reconciling migration history against the restored database..."
    # Fail loudly if sqlite3 is missing rather than silently marking nothing:
    # a silent no-op here re-triggers the crash this block exists to prevent,
    # and looks identical to a clean reconcile. 07-backups.sh installs sqlite3
    # during setup.sh (step 1 of this script), so this should never fire.
    if ! command -v sqlite3 >/dev/null 2>&1; then
        fail "sqlite3 is required to reconcile migrations but is not installed. Run: apt-get install -y sqlite3"
    fi
    MIGRATIONS_DIR="/opt/pocketbase/pb_migrations"
    MARKED=0
    DEFERRED=0
    for mf in "$MIGRATIONS_DIR"/*.js; do
        [ -f "$mf" ] || continue
        base="$(basename "$mf")"

        # Already recorded in the restored DB's history → nothing to do.
        already=$(sqlite3 "${PB_DATA_DIR}/data.db" \
            "SELECT COUNT(*) FROM _migrations WHERE file='${base}';" 2>/dev/null || echo 0)
        if [ "${already:-0}" != "0" ]; then
            continue
        fi

        # Which collection does this migration create, if any? The seed
        # migrations name it in a `"name": "<collection>"` line.
        cname=$(grep -oE '"name"[[:space:]]*:[[:space:]]*"[a-zA-Z0-9_]+"' "$mf" 2>/dev/null \
            | head -1 | sed -E 's/.*"([a-zA-Z0-9_]+)"$/\1/')

        if [ -n "$cname" ]; then
            exists=$(sqlite3 "${PB_DATA_DIR}/data.db" \
                "SELECT COUNT(*) FROM _collections WHERE name='${cname}';" 2>/dev/null || echo 0)
            if [ "${exists:-0}" != "0" ]; then
                # Collection already present → mark the migration applied so PB
                # does not try to re-create it.
                sqlite3 "${PB_DATA_DIR}/data.db" \
                    "INSERT OR IGNORE INTO _migrations (file, applied) VALUES ('${base}', $(date +%s%N));" 2>/dev/null \
                    && { log "  · ${base} → already-applied (collection '${cname}' present)"; MARKED=$((MARKED+1)); }
                continue
            fi
            # Collection absent → let PocketBase create it.
            log "  · ${base} → will run (collection '${cname}' missing)"
            DEFERRED=$((DEFERRED+1))
        else
            # Cannot attribute it to a collection. If its collection-name-agnostic
            # file name is unknown and it creates something, letting PB run it is
            # the safe direction for an *absent* artefact — but running an unknown
            # migration blindly can also fail. Record it as a warning so the
            # operator sees it rather than the restore dying silently.
            warn "  · ${base} → not a collection migration; leaving for PocketBase"
            DEFERRED=$((DEFERRED+1))
        fi
    done
    log "✓ Migration reconcile: ${MARKED} marked already-applied, ${DEFERRED} left to run."
    # Ownership can change if sqlite3 ran as root.
    chown pocketbase:pocketbase "${PB_DATA_DIR}/data.db" 2>/dev/null || true
fi

# Align the restored admin password with the secrets file while PB is stopped.
# The restored DB's admin may predate the current secrets (old auto-generated
# password) — this guarantees the documented credentials work after restore.
if [ -n "${PB_ADMIN_EMAIL:-}" ] && [ -n "${PB_ADMIN_PASS:-}" ]; then
    if (cd /opt/pocketbase && ./pocketbase admin update "$PB_ADMIN_EMAIL" "$PB_ADMIN_PASS" >/dev/null 2>&1); then
        log "✓ Admin password aligned with secrets (${PB_ADMIN_EMAIL})"
    else
        warn "Could not update admin password — set it manually in the admin UI if login fails"
    fi
fi

systemctl start pocketbase
sleep 2

# The backup timer has Requires=pocketbase.service: stopping PocketBase above
# STOPPED the timer too (Requires propagates stops, not starts). Bring it back.
if systemctl restart pocketbase-backup.timer 2>/dev/null; then
    systemctl is-active --quiet pocketbase-backup.timer && log "✓ Backup timer active (hourly)" || warn "Backup timer exists but inactive — run: systemctl enable --now pocketbase-backup.timer"
else
    warn "Backup timer restart failed — run: systemctl enable --now pocketbase-backup.timer"
fi

# Verify restore
if systemctl is-active --quiet pocketbase; then
    log "✓ PocketBase started successfully after restore."
else
    fail "PocketBase failed to start after restore. Check journalctl -u pocketbase"
fi

# Quick sanity check: count records directly in the restored DB
# (the API requires an admin token and the collections are admin-only).
CODES_COUNT=$(sqlite3 "${PB_DATA_DIR}/data.db" "SELECT COUNT(*) FROM codes;" 2>/dev/null || echo "unknown")
log "Restored codes collection has ${CODES_COUNT} records."

# Verify the JS hooks actually loaded (a generic 400 means a hook load error).
HOOK_RESP=$(curl -s -o /tmp/hook-resp.json -w "%{http_code}" -X POST "http://127.0.0.1:8090/api/activate" -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "000")
if [ "$HOOK_RESP" = "400" ] && grep -q "Missing code" /tmp/hook-resp.json 2>/dev/null; then
    log "✓ Activation hook loaded (expected 400 'Missing code')"
else
    warn "Activation hook check unexpected (HTTP ${HOOK_RESP}) — check: journalctl -u pocketbase"
fi
rm -f /tmp/hook-resp.json

# Cleanup
rm -rf "$TMP_DIR"

log "══════════════════════════════════════════"
log "✅ Restore complete!"
log "   Domain:        ${DOMAIN}"
log "   PocketBase:    https://${DOMAIN}/_/"
log "   Records:       ~${CODES_COUNT} codes in database"
log ""
log "   NEXT STEP: Point your DNS A record"
log "   for ${DOMAIN} to THIS server's IP."
log "══════════════════════════════════════════"

# ── Test endpoints ──
log "Running smoke tests..."
sleep 1
curl -f -s -o /dev/null "http://127.0.0.1:8090/api/health" && log "✓ PocketBase health check OK" || warn "PocketBase health check failed"
curl -f -s -o /dev/null "https://${DOMAIN}/api/health" && log "✓ Public health endpoint OK" || warn "Public health check failed (DNS may not point here yet)"
