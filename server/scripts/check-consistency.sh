#!/usr/bin/env bash
# Chrysalis cross-language consistency check.
#
# WHY THIS EXISTS
#
# This project's worst bugs have not been logic errors. They have been the same
# shape, repeatedly:
#
#   * `uot_port` vs `server_port_uot` — the hub emitted one name, the client
#     declared another, and the mismatch was a SILENT no-op. UDP-over-TCP was
#     dead fleet-wide while both sides were individually "correct".
#   * `download_<platform>` vs `update_<platform>` — two publish scripts
#     disagreed, and a Windows client was handed a Linux binary.
#   * `unbound_at` / `unbind_reason` — written by a hook for months against
#     columns that did not exist. PocketBase silently discarded them, so an
#     audit trail looked implemented and recorded nothing.
#   * `aes-256-gcm` duplicated in ten places with nothing binding them.
#
# Every one is a value that must agree across languages (Rust / goja JS /
# Python / shell) with NO mechanism enforcing agreement. A comment saying
# "keep these in sync" is not a mechanism.
#
# This script is the mechanism. It is deliberately dumb and greppy: it does not
# try to understand the code, it checks that the constants agree and fails loudly
# when they do not. Run it before any change that touches a wire name, a
# platform key, the schema, or the cipher.
#
# Usage:  server/scripts/check-consistency.sh        (from anywhere)
# Exit:   0 = consistent, 1 = drift found.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOKS="$REPO/server/pb_hooks"
SCRIPTS="$REPO/server/scripts"
RUST="$REPO/client/src-tauri/src/chrysalis"

FAIL=0
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

# ─────────────────────────────────────────────────────────────
# OUTPUT VOLUME IS A COST, AND IT WAS THE LARGEST ONE.
#
# Measured on this checkout: a full run printed 40 KB across 530 lines, of which
# 27 KB was `OK` lines and 5 KB was the `══` rules -- because EVERY step echoes a
# banner and every check echoes a verdict. An agent runs this several times per
# change, and each run re-read 40 KB (~10k tokens) to learn "nothing is wrong".
# That is the expense, not the code it was changing.
#
# So the default is QUIET: print only the sections that produced something a
# reader must act on -- a BAD, a WARN, or a section that failed to run. A clean
# run costs three lines. `--verbose` restores the full transcript for a human
# reading it in a terminal or for CI logs, where volume is free.
#
# WHAT THIS MUST NOT DO: hide a failure. The summary line still counts what ran,
# and any section with a BAD always prints in full. A quiet mode that could
# swallow a red would be exactly the `SKIP != PASS` defect the docs warn about.
# `--verbose` output is byte-identical to the historical format, so nothing that
# parsed this script's output breaks.
# ─────────────────────────────────────────────────────────────
case "${1:-}" in
    --verbose|-v) QUIET=0 ;;
    --quiet|-q|"") QUIET=1 ;;
    *) QUIET=1 ;;
esac

# Counters exist so the runner can decide whether to release a section's buffer
# WITHOUT grepping the buffer for the string "BAD" -- a section whose message
# happens to contain that word would otherwise look like a failure, and a real
# failure whose message avoided the word could look like a pass.
_bad_seen=0
_warn_seen=0

ok()   { echo -e "  ${GREEN}OK${NC}   $*"; }
bad()  { _bad_seen=$((_bad_seen + 1)); echo -e "  ${RED}BAD${NC}  $*"; FAIL=1; }
warn() { _warn_seen=$((_warn_seen + 1)); echo -e "  ${YELLOW}WARN${NC} $*"; }

# Buffered per-section output. Each section's banner (its `echo "N. …"`) is
# printed to a buffer rather than the terminal, then released only if that
# section produced a BAD/WARN. This is what lets a clean run print nothing.
SECTION_BUF=$(mktemp)
trap 'rm -f "$SECTION_BUF"' EXIT

# Every `echo` inside a section writes here instead of stdout. The heuristic is
# sound because sections print their own banner and nothing else at top level --
# the orchestrator's own echo is suppressed by wrapping this around the call.
sec_echo() { echo -e "$*" >>"$SECTION_BUF"; }

if [ "$QUIET" -eq 1 ]; then
    echo "Chrysalis consistency check (quiet; --verbose for the full transcript)"
else
    echo "Chrysalis cross-language consistency check"
    echo "════════════════════════════════════════════════════════════"
fi

# ─────────────────────────────────────────────────────────────
# HOW THIS FILE IS ORGANISED (and why it is no longer one file)
#
# This was 9,482 lines. Changing one guard meant reading the whole thing, and
# every repository-wide grep landed in it. The sections now live in
# `checks/NN-<slug>.sh`, one function each, listed in `checks/ORDER`.
#
# THE ORDERING IS NOT ALPHABETICAL AND MUST NOT BE "FIXED". Section 9 is followed
# by 1c then 1g-bis because those were inserted there rather than renumbered --
# renumbering would invalidate every `§N` reference in the docs and in FIXES.md.
# `checks/ORDER` is generated from the original file and is the authority.
#
# A BROKEN SECTION FILE IS THE ONE WAY THIS CAN NOW LIE TO YOU. Each file is
# `source`d into THIS shell, so a syntax error aborts the `source` builtin and
# the loop dies -- and with `set -u` (not `set -e`) control falls through to the
# summary, which then prints "All consistency checks passed" having run only the
# sections above the breakage. That is not hypothetical: it is how the §53
# comment records a guard already dying silently in this repository.
#
# So the syntax is checked BEFORE anything runs, and a failure exits non-zero
# without claiming to have passed. `lib/verify-split.py` is what proves that
# pre-check is not decorative.
#
# TO CHANGE A GUARD: edit the one file under `checks/`. To add a section, add
# its file AND a line to `checks/ORDER` in the position it must run.
# ─────────────────────────────────────────────────────────────

CHECKS_DIR="$SCRIPTS/checks"

if [ ! -f "$CHECKS_DIR/ORDER" ]; then
    echo -e "${RED}FATAL${NC}  $CHECKS_DIR/ORDER is missing -- no sections to run."
    echo "        Regenerate with server/scripts/lib/extract-sections.py."
    exit 1
fi

# Pre-check: every section must parse. Reported as a list, not first-failure,
# so one pass finds all of them.
syntax_bad=0
while read -r _number _fn; do
    case "$_number" in ''|'#'*) continue ;; esac
    _file=$(ls "$CHECKS_DIR/$_number"-*.sh 2>/dev/null | head -1)
    if [ -z "$_file" ]; then
        echo -e "${RED}FATAL${NC}  ORDER names section $_number, but no checks/$_number-*.sh exists"
        syntax_bad=1
        continue
    fi
    if ! bash -n "$_file" 2>/dev/null; then
        echo -e "${RED}FATAL${NC}  syntax error in ${_file#"$REPO"/}"
        bash -n "$_file" 2>&1 | sed 's/^/        /'
        syntax_bad=1
    fi
done < "$CHECKS_DIR/ORDER"

if [ "$syntax_bad" -ne 0 ]; then
    echo
    echo -e "${RED}A section does not parse. NOT running: a broken section would let the"
    echo -e "summary claim \"passed\" after silently skipping everything below it.${NC}"
    exit 1
fi

# ── Run the sections, in ORDER. ──
#
# Each section runs with BOTH streams redirected into a buffer, then the buffer is
# released only if the section said something that needs acting on. stderr is
# captured with stdout on purpose: five sections emit their evidence for a BAD via
# `>&2`, so capturing only stdout would print the verdict without the reason.
#
# The buffer is released when ANY of these is true, and the list is deliberately
# generous -- a quiet mode that can swallow a red is worse than no quiet mode:
#   * the section produced a BAD or a WARN (counted before/after, not by grepping
#     the buffer for the word, so a section whose MESSAGE contains "BAD" cannot
#     fake it)
#   * the section could not be sourced at all
#   * FAIL changed during the section, whatever the text said
_sections_run=0
_sections_quiet=0

while read -r number fn; do
    case "$number" in ''|'#'*) continue ;; esac
    _file=$(ls "$CHECKS_DIR/$number"-*.sh 2>/dev/null | head -1)
    : > "$SECTION_BUF"

    # Track BAD and WARN SEPARATELY. An earlier version compared their SUM, which
    # is wrong in a way that silently hid failures: once an earlier section had set
    # FAIL=1, the `FAIL changed` test was already false, and a section that added a
    # BAD while removing a WARN left the sum unchanged -- so its buffer was never
    # released and the BAD vanished from the quiet output. Caught by diffing quiet
    # against --verbose and finding 2 of 18 BADs missing.
    _fail_before=$FAIL
    _bad_before=$_bad_seen
    _warn_before=$_warn_seen

    {
        # shellcheck disable=SC1090
        if ! . "$_file"; then
            bad "section $number could not be sourced ($_file) -- its checks DID NOT RUN"
        fi
        "$fn"
    } >"$SECTION_BUF" 2>&1

    _sections_run=$((_sections_run + 1))

    if [ "$QUIET" -eq 0 ]; then
        cat "$SECTION_BUF"
    elif [ "$FAIL" -ne "$_fail_before" ] || [ "$_bad_seen" -ne "$_bad_before" ] \
         || [ "$_warn_seen" -ne "$_warn_before" ]; then
        # The section has something to act on, so release it -- but WITHOUT its
        # OK lines. Those are the majority of the remaining volume: §53 alone emits
        # 34 of them before reaching its BAD, and the reader needs none. Only the
        # OK prefix is dropped; every other line (the banner, BADs, WARNs, their
        # evidence, and anything a section prints without a marker) survives.
        # Strip ANSI FIRST, then match the plain `  OK   ` prefix. Matching against
        # `$GREEN` directly does not work: the variable holds the four characters
        # `\033` as written, not the escape byte, so neither `-E` (where `[` starts
        # a bracket expression and the pattern matches nothing -- which silently
        # let every line through) nor `-F` (which compares the literal backslash
        # sequence) does what it looks like it does.
        # 26 sections end with a `══` rule, which exists to SEPARATE sections in
        # the verbose transcript. With the OK lines gone there is nothing left for
        # it to separate, and it reads as a second verdict line. Drop blank and
        # rule-only lines too; anything with content survives.
        sed 's/\x1b\[[0-9;]*m//g' "$SECTION_BUF" \
            | grep -vF "  OK   " \
            | grep -vE '^[[:space:]]*$' \
            | grep -vE '^[[:space:]]*═+[[:space:]]*$'
    else
        _sections_quiet=$((_sections_quiet + 1))
    fi
done < "$CHECKS_DIR/ORDER"

# ── The verdict line, and the whole of a clean run's output. ──
#
# On a clean run this paragraph is ALL the reader gets: one line, saying the
# sections ran and how many were silent. Anything that went wrong has already
# printed above it, with its banner and its evidence.
if [ "$FAIL" -eq 0 ]; then
    if [ "$QUIET" -eq 1 ]; then
        echo "PASS  ${_sections_run} sections ran, $_sections_quiet silent"
    else
        echo "════════════════════════════════════════════════════════════"
        echo -e "${GREEN}All consistency checks passed.${NC}"
        echo "════════════════════════════════════════════════════════════"
    fi
else
    echo "════════════════════════════════════════════════════════════"
    echo -e "${RED}Consistency FAILED — see BAD lines above.${NC}"
    echo "════════════════════════════════════════════════════════════"
fi
exit "$FAIL"