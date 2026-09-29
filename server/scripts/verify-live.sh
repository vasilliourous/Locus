#!/usr/bin/env bash
# verify-live.sh — check the Class A world claims in docs/operate/CLAIMS.md §5.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT THIS IS FOR
#
# docs/operate/CLAIMS.md §5 is a register of claims about the world: the hub is
# up, the domain resolves, a release is published, a preview is current. Those
# cannot be proven by reading the tree — that is what makes them Class A — so
# they are recorded with a date and a method instead of being asserted in prose.
#
# This script IS the method. It is the only thing permitted to advance a
# `last-verified` date in that register.
#
# ─────────────────────────────────────────────────────────────────────────────
# THE THREE-VALUED RESULT, AND WHY "SKIP" IS NOT "PASS"
#
# This is the important design decision, and getting it wrong is how the original
# problem was created. There are three outcomes, and they are NOT collapsible:
#
#   PASS  — we reached the thing and it answered. The claim is verified today.
#   FAIL  — we reached it and it did not answer. The claim is FALSE. Someone
#           should act.
#   SKIP  — we could not reach the network to find out (no DNS, no route, no
#           credentials). The claim is UNKNOWN. It is NOT verified, and nothing
#           records it as verified.
#
# SKIP must never be reported as a pass. If it were, then "the hub is online"
# would be written down as a fact every time it ran in an environment with no
# route to the hub — which is exactly how the stale IP in the old STATE.md came
# to be trusted. An environment that cannot see the hub has not learned that the
# hub is up; it has learned nothing at all.
#
# Exit codes:  0 = all claims PASS  1 = at least one FAIL  2 = SKIP (nothing
#                                                                  verified)
#
# Writes nothing, anywhere. Advancing a date in CLAIMS.md is a manual,
# deliberate act by a person who read the output — see the note at the end.
#
# Usage:  server/scripts/verify-live.sh [--json]
# Read-only. Safe to run at any time.
# ─────────────────────────────────────────────────────────────────────────────

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLAIMS="$REPO/docs/operate/CLAIMS.md"
STATE="$REPO/docs/state.toml"

JSON=0
[ "${1:-}" = "--json" ] && JSON=1

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

# Results are accumulated as "ID<TAB>OUTCOME<TAB>detail" lines; the verdict is
# computed at the end so a later PASS can never mask an earlier FAIL.
RESULTS=""
record() { RESULTS="${RESULTS}${1}"$'\n'; }

have()     { command -v "$1" >/dev/null 2>&1; }
now_utc()  { date -u +%Y-%m-%dT%H:%M:%SZ; }

# ── Read the domain from its single home rather than restating it. ──
# This script must not become the fourth place the hub's identity is written
# down. state.toml is the home; if it cannot be read, that is a real finding.
DOMAIN=""
if [ -f "$STATE" ]; then
    DOMAIN=$(python3 - "$STATE" <<'PY' 2>/dev/null || true
import sys
try:
    import tomllib
    with open(sys.argv[1], "rb") as fh:
        print(tomllib.load(fh)["hub"]["domain"]["value"])
except Exception:
    pass
PY
)
fi

if [ -z "$DOMAIN" ]; then
    # Fall back to the documented default so the script still explains itself,
    # but say so loudly — this is a degraded mode, not normal operation.
    DOMAIN="networkingguides.duckdns.org"
    echo -e "${YELLOW}WARN${NC} could not read hub.domain from docs/state.toml; using the documented default."
fi

echo "verify-live.sh — checking the world claims in CLAIMS.md §5"
echo "════════════════════════════════════════════════════════════"
echo "  time:    $(now_utc)"
echo "  domain:  $DOMAIN"
echo

# ── A1 — the domain resolves and the hub answers /api/health ──
# DNS-shaped on purpose: the domain is the only stable identifier (CLAIMS.md §6),
# so resolution is the first thing that must work and the first thing to fail
# when the box moves.
A1_RESOLVED=""
A1_IP=""
if have getent; then
    A1_IP="$(getent ahostsv4 "$DOMAIN" 2>/dev/null | awk '{print $1}' | sort -u | head -1)"
elif have dig; then
    A1_IP="$(dig +short A "$DOMAIN" 2>/dev/null | head -1)"
fi

if [ -n "$A1_IP" ]; then
    A1_RESOLVED="yes"
    echo -e "  ${GREEN}A1${NC} $DOMAIN resolves (address observed this run, deliberately not recorded)"
else
    echo -e "  ${BLUE}A1${NC} $DOMAIN did not resolve here"
fi

if [ -z "$A1_RESOLVED" ]; then
    # No DNS at all — we cannot learn anything about the hub from this
    # environment. This is SKIP, not FAIL: the hub may be perfectly healthy and
    # this box simply cannot see it.
    record "A1"$'\t'"SKIP"$'\t'"$DOMAIN did not resolve from this environment — claim UNKNOWN, not verified"
else
    if ! have curl; then
        record "A1"$'\t'"SKIP"$'\t'"curl is not installed, cannot probe /api/health"
    else
        # `|| echo 000` must NOT be appended to curl's own output: curl already
        # prints 000 on a connection failure, and the `||` then appends a second
        # one, producing "000000" — which matched no branch and was misreported as
        # a hub fault. Capture curl's output alone and let a non-zero exit be
        # reported by the value it prints.
        code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 \
                "https://$DOMAIN/api/health" 2>/dev/null)"
        code="${code:-000}"
        if [ "$code" = "200" ]; then
            record "A1"$'\t'"PASS"$'\t'"/api/health returned 200"
            echo -e "  ${GREEN}A1${NC} /api/health -> 200"
        elif [ "$code" = "000" ]; then
            # Resolved but unreachable. Could be a down hub OR a local egress
            # block. We cannot tell them apart from here, so this is SKIP with
            # the reason stated — NOT a silent pass, and not charged to the hub.
            record "A1"$'\t'"SKIP"$'\t'"/api/health unreachable (no response) — could be the hub or this environment's egress; UNKNOWN"
            echo -e "  ${BLUE}A1${NC} /api/health unreachable — cannot distinguish hub-down from egress-blocked"
        else
            # We reached something and it answered with an error: that is a real
            # finding about the hub.
            record "A1"$'\t'"FAIL"$'\t'"/api/health returned $code (reached, and it is unhealthy)"
            echo -e "  ${RED}A1${NC} /api/health -> $code"
        fi
    fi
fi

# ── A3 — the rolling preview release is present ──
# Verifies EXISTENCE and when it was published, not that it is current for any
# particular commit. "Current for commit X" is a claim about a specific push and
# belongs to the person making that push; this can only answer "does a preview
# exist, and how old is it".
if ! have gh; then
    record "A3"$'\t'"SKIP"$'\t'"gh is not installed — cannot inspect the build-preview release"
    echo -e "  ${BLUE}A3${NC} gh not installed — preview status unknown"
else
    if ! gh auth status >/dev/null 2>&1; then
        record "A3"$'\t'"SKIP"$'\t'"gh is not authenticated — cannot inspect the build-preview release"
        echo -e "  ${BLUE}A3${NC} gh not authenticated — preview status unknown"
    else
        prev="$(gh release view build-preview --json tagName,isPrerelease,publishedAt \
                 --jq '"\(.tagName) \(.isPrerelease) \(.publishedAt)"' 2>/dev/null || true)"
        if [ -z "$prev" ]; then
            record "A3"$'\t'"FAIL"$'\t'"no build-preview release exists (a push to main should have created one)"
            echo -e "  ${RED}A3${NC} no build-preview release"
        else
            pre_flag="$(printf '%s' "$prev" | awk '{print $2}')"
            if [ "$pre_flag" != "true" ]; then
                record "A3"$'\t'"FAIL"$'\t'"build-preview exists but is NOT marked prerelease — it would be offered as the latest release"
                echo -e "  ${RED}A3${NC} build-preview is not a prerelease"
            else
                record "A3"$'\t'"PASS"$'\t'"build-preview exists and is marked prerelease ($(printf '%s' "$prev" | awk '{print $3}'))"
                echo -e "  ${GREEN}A3${NC} build-preview present and correctly marked prerelease"
            fi
        fi
    fi
fi

# ── A4 — the client workflow has run recently ──
if ! have gh || ! gh auth status >/dev/null 2>&1; then
                record "A4"$'\t'"SKIP"$'\t'"gh unavailable or unauthenticated — cannot inspect workflow runs"
    echo -e "  ${BLUE}A4${NC} gh unavailable — workflow-run status unknown"
else
    runs="$(gh run list --workflow=client.yml --limit 1 \
             --json conclusion,headSha,createdAt \
             --jq '.[0] | "\(.conclusion) \(.headSha[0:7]) \(.createdAt)"' 2>/dev/null || true)"
    if [ -z "$runs" ]; then
        record "A4"$'\t'"FAIL"$'\t'"the client workflow has no runs at all"
        echo -e "  ${RED}A4${NC} client.yml has never run"
    else
        concl="$(printf '%s' "$runs" | awk '{print $1}')"
        if [ "$concl" = "success" ]; then
            record "A4"$'\t'"PASS"$'\t'"latest run on $(printf '%s' "$runs" | awk '{print $2}') concluded success"
            echo -e "  ${GREEN}A4${NC} latest run ($(printf '%s' "$runs" | awk '{print $2}')): success"
        else
            record "A4"$'\t'"FAIL"$'\t'"latest run on $(printf '%s' "$runs" | awk '{print $2}') concluded: $concl"
            echo -e "  ${RED}A4${NC} latest run: $concl"
        fi
    fi
fi

# ── A2 and A5 are deliberately NOT probed ──
# A2 (codes in active use) needs the admin console and a real credential, and
# counting rows is precisely what CLAIMS.md §4 removed from the repo. A5 (an
# update installs end to end) needs Windows/macOS hardware. Both are reported as
# SKIP with the reason, so the register's honest state is visible rather than
# quietly absent.
record "A2"$'\t'"SKIP"$'\t'"needs the admin console and a credential, and the count is deliberately not recorded (CLAIMS.md §4)"
record "A5"$'\t'"SKIP"$'\t'"needs real Windows/macOS hardware (STILL-OPEN.md)"

echo
echo "────────────────────────────────────────────────────────────"
printf '%-5s %-6s %s\n' "CLAIM" "STATE" "DETAIL"
printf '%s\n' "$RESULTS" | while IFS=$'\t' read -r id outcome detail; do
    [ -n "$id" ] || continue
    case "$outcome" in
        PASS) col="$GREEN" ;;
        FAIL) col="$RED" ;;
        SKIP) col="$BLUE" ;;
        *)    col="$NC" ;;
    esac
    printf "%-5s ${col}%-6s${NC} %s\n" "$id" "$outcome" "$detail"
done
echo "────────────────────────────────────────────────────────────"

n_pass=$(printf '%s\n' "$RESULTS" | grep -c $'\tPASS\t' || true)
n_fail=$(printf '%s\n' "$RESULTS" | grep -c $'\tFAIL\t' || true)
n_skip=$(printf '%s\n' "$RESULTS" | grep -c $'\tSKIP\t' || true)

if [ "$JSON" -eq 1 ]; then
    echo
    printf '%s\n' "$RESULTS" | python3 -c '
import sys, json
rows = []
for line in sys.stdin:
    line = line.rstrip("\n")
    if not line: continue
    parts = line.split("\t")
    if len(parts) == 3:
        rows.append({"claim": parts[0], "outcome": parts[1], "detail": parts[2]})
print(json.dumps(rows, indent=2))
'
fi

echo
echo "  pass=$n_pass  fail=$n_fail  skip=$n_skip"
echo

if [ "$n_fail" -gt 0 ]; then
    echo -e "${RED}FAIL — a claim was reached and is FALSE, or is structurally broken.${NC}"
    echo "Act on it before believing anything in CLAIMS.md §5."
    echo "This script does NOT update the register; that is a deliberate manual step."
    exit 1
elif [ "$n_pass" -eq 0 ]; then
    echo -e "${BLUE}SKIP — nothing was verified.${NC}"
    echo "This environment could not reach the hub. The claims in CLAIMS.md §5 remain"
    echo "UNVERIFIED and their dates must NOT be advanced. 'We could not check' is not"
    echo "'it is up' — that confusion is the bug this script exists to prevent."
    exit 2
else
    echo -e "${GREEN}PASS — the claims that could be checked are green.${NC}"
    [ "$n_skip" -gt 0 ] && echo "($n_skip claim(s) still SKIPPED — those stay unverified in the register.)"
    echo
    echo "To record this: update the 'Last verified' column in docs/operate/CLAIMS.md §5"
    echo "for the claims marked PASS, with today's date ($(date -u +%Y-%m-%d))."
    echo "Leave every SKIPPED claim's date exactly as it was."
    exit 0
fi
