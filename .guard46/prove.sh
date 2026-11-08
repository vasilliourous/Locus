#!/usr/bin/env bash
# Prove §46 fails against the OLD workflow (the arm64-only zip) and passes
# against the fixed one. Extracts the §46 block from the real checker so the
# thing being proved is the shipped code, not a copy.
set -uo pipefail
WF="$1"
FAIL=0
RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
ok()   { echo -e "  ${GREEN}OK${NC}   $*"; }
bad()  { echo -e "  ${RED}BAD${NC}  $*"; FAIL=1; }
WORKFLOW="$WF"
source /dev/stdin <<<"$(sed -n '/^echo "46\. The minisign used to sign on macOS/,/^echo "════════════════════════════════════════════════════════════"$/p' server/scripts/check-consistency.sh | sed '1d;$d')"
exit "$FAIL"
