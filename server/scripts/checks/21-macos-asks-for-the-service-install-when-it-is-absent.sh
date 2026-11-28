#!/usr/bin/env bash
# Section 21 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_21() {
    echo "21. macOS asks for the Service install when it is absent"
    # ─────────────────────────────────────────────────────────────
    # WHY THIS EXISTS
    #
    # macOS has no installer that registers the Chrysalis Service. On Windows the NSIS
    # setup does it during install, so a fresh install is never `NotInstalled` — but
    # a `.app` dragged out of the `.dmg` is, on first launch, with `enable_tun_mode`
    # defaulting to false. The only writer of that flag was the Connect path, which
    # sits behind the `tun_capable()` refusal an absent Service causes:
    #
    #     no Service -> Connect refused -> TUN never enabled
    #     TUN disabled -> install never requested -> no Service
    #
    # Nothing broke the loop, so a student who installed from the `.dmg` could never
    # connect, and the install affordance never appeared either (the dialog is driven
    # by `serviceNeedsAttention`, and an absent Service was deliberately excluded
    # from it). A real student hit this and there was no in-app route out at all.
    #
    # The fix is that `prepare_startup` requests the install for an absent Service on
    # macOS. This guard exists because that behaviour is INVISIBLE from the tree: it
    # cannot be exercised without macOS, the deadlock is silent, and reverting it
    # restores a state where the app looks healthy and simply never connects. Every
    # separate piece below was independently necessary, so each is asserted:
    #   - the predicate that decides it,
    #   - its macOS gate,
    #   - that it is actually called from the startup path,
    #   - and that the refusal no longer points at a Settings control macOS lacks.
    # ─────────────────────────────────────────────────────────────
    LIFECYCLE="$REPO/client/src-tauri/src/core/manager/lifecycle.rs"
    if [ ! -f "$LIFECYCLE" ]; then
        warn "no lifecycle.rs — this guard cannot run"
    else
        # The existence checks below read the file, so an unreadable file is the
        # GUARD failing, not the predicate disappearing. `grep` exits 2 on a read
        # error; only exit 1 (no match) is evidence about the code.
        predicate_grep=0
        grep -q 'fn should_request_install_for_absent_service' "$LIFECYCLE" || predicate_grep=$?
        if [ "$predicate_grep" = 2 ]; then
            warn "lifecycle.rs is unreadable — these guards cannot run"
        elif [ "$predicate_grep" = 0 ]; then
            ok "the absent-Service install predicate exists"
        else
            bad "should_request_install_for_absent_service is gone from lifecycle.rs"
            bad "  without it nothing requests the Service install on macOS, and a DMG"
            bad "  install can never connect — see the comment above it for the loop"
        fi

        # The call site, not just the definition. A predicate nothing calls is inert,
        # which is the exact shape of the original bug (the pieces were all present).
        #
        # Read through a python extractor that strips comments, because this file
        # explains all three pieces in prose at the point of contact — a bare grep
        # matches the explanation, so deleting the call while keeping its comment
        # would read as a pass. That blind spot was found by falsifying this guard
        # rather than by writing it.
        # THE EXTRACTOR MUST NOT BE ABLE TO LIE ABOUT THE CODE.
        #
        # This check produced a FALSE BAD line ("prepare_startup does not CALL ...")
        # on a tree where lifecycle.rs demonstrably called the predicate at line 512,
        # and then went green on the identical tree seconds later.
        #
        # The mechanism: `$()` captures stdout, but a python traceback goes to
        # stderr. If the extractor fails — an unreadable path, a permission problem,
        # a truncated read while another tool writes the file — stdout is empty, an
        # empty string does not contain the call, and the guard reports a defect it
        # never looked for. That is DEBUGGING-METHOD §3's failure mode wearing a
        # guard's clothes: the check cannot distinguish "the code is wrong" from
        # "I could not read the code", so both read as BAD.
        #
        # So: keep stderr (2>&1) where a reader can see the traceback, and turn an
        # empty result or a traceback into a WARN about the GUARD, never a BAD about
        # the code.
        lifecycle_code="$(python3 - "$LIFECYCLE" 2>&1 <<'PY'
import sys
# Read with a stability check. A single read can catch the file mid-write
# (cargo, rustfmt, an editor) and return a truncated body; the call site
# then looks absent through no fault of the code. Read twice with a short
# pause and accept only when both reads agree AND the text looks whole (a
# trailing brace is cheap evidence). If it never stabilises, exit non-zero
# so the caller reports "cannot read" rather than "is wrong".
import sys, time

path = sys.argv[1]
text = None
for _attempt in range(6):
    first = open(path, encoding="utf-8", errors="replace").read()
    time.sleep(0.05)
    second = open(path, encoding="utf-8", errors="replace").read()
    if first == second and first.rstrip().endswith("}"):
        text = first
        break
if text is None:
    # Emit a marker on STDOUT, not a SystemExit string. stderr is captured by
    # the caller (2>&1), so a message there becomes a non-empty "result" and the
    # guard falls through to reporting a code defect it never looked for — the
    # exact failure this retry loop was added to prevent.
    print("__EXTRACTOR_FAILED__")
    raise SystemExit(0)
kept = []
for line in text.splitlines():
    if line.strip().startswith("//"):
        continue
    kept.append(line)
print("\n".join(kept))
PY
    )"

        extractor_broken=0
        if [ -z "$lifecycle_code" ]; then
            extractor_broken=1
            warn "the lifecycle extractor returned nothing — this guard cannot run"
        fi
        case "$lifecycle_code" in
            *__EXTRACTOR_FAILED__*)
                extractor_broken=1
                warn "the lifecycle extractor could not read the file whole — this guard cannot run" ;;
        esac
        case "$lifecycle_code" in
            *Traceback*)
                extractor_broken=1
                warn "the lifecycle extractor failed (traceback above) — this guard cannot run" ;;
        esac

        # ── MATCHING IS DONE WITH `case`, NOT `printf | grep -q` ──
        #
        # Two sub-checks in this section used the natural-looking
        # `printf '%s' "$x" | grep -q PATTERN`. That idiom CANNOT FAIL CORRECTLY here,
        # and it produced a FALSE BAD in CI on 2026-11-02 against a tree where the
        # call was present at lifecycle.rs:508.
        #
        # The mechanism, reproduced: `grep -q` exits the moment it matches, which
        # closes the pipe while `printf` is still writing. `printf` dies on SIGPIPE,
        # and because this script runs under `set -uo pipefail` the PIPELINE takes
        # printf's status — 141 — not grep's 0. So a MATCH reads as a non-zero
        # pipeline, the `if` takes its else branch, and the guard reports a defect it
        # just found. The log shows its footprint: `printf: write error: Broken pipe`
        # on the BAD line itself.
        #
        # It is input-size dependent, which is why it passed locally and in earlier
        # runs: 37 KB of stripped lifecycle.rs is small enough to fit in the pipe
        # buffer on this machine, so `printf` finishes before `grep` exits and no
        # SIGPIPE arises. A guard whose verdict depends on how much came before it in
        # the file is worse than no guard — and only a broken guard fails this way,
        # so the shape of the fix is to stop writing to a pipe at all.
        #
        # `case` is a builtin: no process, no pipe, no signal, and it reports exactly
        # "does this string contain this literal". Verified in both directions before
        # being kept — BAD against input lacking the call, OK against input holding it
        # 40 000 times over, where the old idiom reported BAD.
        #
        # ── AND THE SEARCH MUST BE SCOPED TO THE macOS CALL SITE ──
        #
        # `prepare_startup` contains TWO calls to `require_install_for_session()`:
        # the pre-existing `service_required`/TUN branch, and the macOS call this
        # section exists to protect. A whole-file search is satisfied by the TUN one,
        # so deleting the macOS call left this guard printing OK — proved by
        # falsifying it: the macOS call removed (definition and prose kept) read as
        # OK because line ~469 still matched. The guard could not fail, which is the
        # defect AGENTS.md rule 3 exists to catch and is why this is written as a
        # region rather than a substring.
        #
        # The region is the `#[cfg(target_os = "macos")]` attribute through the end
        # of its `if` body, sliced by the extractor's line numbers. If the slice
        # cannot be made, that is a GUARD failure (WARN), never a verdict on the
        # code — the same rule as everywhere else in this section.
        # Slice the macOS call site out of the stripped body. The attribute is the
        # only `#[cfg(target_os = "macos")]` that immediately precedes a condition
        # test, but there is a second one elsewhere in the file (the dev chmod path),
        # so the anchor is the attribute line whose NEXT non-blank line is not itself
        # a comment and contains `should_request_install_for_absent_service(`.
        # A temp FILE, not a pipe: the python below arrives on the same stdin a
        # heredoc would occupy, so `python3 -` would read the SCRIPT as its data and
        # report "no call site" for every tree including a correct one - observed.
        lifecycle_copy="$(mktemp)"
        printf '%s' "$lifecycle_code" > "$lifecycle_copy"
        macos_call_site="$(python3 - "$lifecycle_copy" 2>&1 <<'PY'
import sys
lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
ATTR = '#[cfg(target_os = "macos")]'
PRED = 'should_request_install_for_absent_service('
FN = 'async fn prepare_startup(&self)'

def region(start):
    out, depth, started = [], 0, False
    for line in lines[start:start + 200]:
        out.append(line)
        depth += line.count('{') - line.count('}')
        if '{' in line:
            started = True
        if started and depth == 0 and '}' in line:
            break
    return "\n".join(out)

def macos_site():
    """The macOS call site: attribute + the `if` it guards. None when absent."""
    for i, l in enumerate(lines):
        if l.strip() != ATTR:
            continue
        body = region(i)
        if PRED in body:
            return body
    for i, l in enumerate(lines):
        if PRED in l and l.strip().startswith('if'):
            j = i - 1
            while j >= 0 and not lines[j].strip():
                j -= 1
            start = j if j >= 0 and lines[j].strip() == ATTR else i
            return region(start)
    # Deleted entirely. Fall back to prepare_startup's body: it does NOT contain
    # the predicate, which is exactly what the caller must be told.
    for i, l in enumerate(lines):
        if FN in l:
            return region(i)
    return None

site = macos_site()
if site is None:
    print("__NO_MACOS_CALL_SITE__")
    raise SystemExit(0)
print(site)
PY
    )"
        rm -f "$lifecycle_copy"

        case "$macos_call_site" in
            *__NO_MACOS_CALL_SITE__*|"")
                warn "could not find the macOS call site in prepare_startup — these two guards cannot run" ;;
            *)
                case "$macos_call_site" in
                    *'if should_request_install_for_absent_service('*)
                        ok "the startup path calls the absent-Service predicate" ;;
                    *)
                        bad "prepare_startup does not CALL should_request_install_for_absent_service"
                        bad "  the predicate existing is not enough: nothing raises the install request,"
                        bad "  so a macOS DMG install still cannot connect" ;;
                esac

                case "$macos_call_site" in
                    *'require_install_for_session()'*)
                        ok "the startup path still raises the install request" ;;
                    *)
                        bad "prepare_startup no longer calls require_install_for_session()"
                        bad "  SERVICE_MANAGER.require_install_for_session() is how the Service"
                        bad "  install dialog is raised on macOS; without it the app never asks" ;;
                esac ;;
        esac

        # The gate must be macOS. On Windows the NSIS installer registers the Service,
        # so an absent one means removal and the pre-existing handling applies;
        # widening this platform-blind would change Windows behaviour unasked.
        #
        # Checked as the cfg attribute IMMEDIATELY preceding the call, since the file
        # also carries a `#[cfg(target_os = "macos")]` elsewhere (the dev chmod path).
        # Same rule as above: this sub-check reads the file, so a read error is the
        # GUARD failing rather than the code being wrong. The result is captured as a
        # marked string so "could not read" and "read it, and the gate is missing"
        # cannot be confused — a bare exit status is not enough distinction here.
        cfg_gate="$(python3 - "$LIFECYCLE" 2>&1 <<'PY'
import re, sys, time
try:
    # Same stability rule as the call-site extractor: a truncated read is the
    # guard failing, not the code being wrong. Without this, a file caught
    # mid-write produced "NOT_GATED" and the guard reported a missing macOS gate
    # that was present.
    text = None
    for _attempt in range(6):
        first = open(sys.argv[1], encoding="utf-8", errors="replace").read()
        time.sleep(0.05)
        second = open(sys.argv[1], encoding="utf-8", errors="replace").read()
        if first == second and first.rstrip().endswith("}"):
            text = first
            break
    if text is None:
        print("UNREADABLE", "file did not hold still across repeated reads")
        raise SystemExit(0)
except OSError as exc:
    print("UNREADABLE", exc)
    raise SystemExit(0)
code = "\n".join(
    l for l in text.splitlines() if not l.strip().startswith("//")
)
# An `if should_request_install_for_absent_service(` guarded by a macOS cfg on the
# line directly above it.
ok = re.search(
    r'#\[cfg\(target_os = "macos"\)\]\s*\n\s*if should_request_install_for_absent_service\(',
    code,
)
print("GATED" if ok else "NOT_GATED")
PY
    )"
        # `NOT_GATED` CONTAINS `GATED`. A `*GATED*` pattern therefore matched the
        # failure case and reported OK — the guard passed while the macOS gate was
        # deleted, which is the "check that cannot fail" defect this file documents
        # in DEBUGGING-METHOD §3. Match the exact word, and keep the unreadable case
        # first so a guard failure is never read as a code verdict.
        case "$cfg_gate" in
            *UNREADABLE*)
                warn "the cfg-gate check could not read lifecycle.rs — this guard cannot run" ;;
            GATED)
                ok "the absent-Service request is cfg-gated to macOS at the call site" ;;
            *)
                bad "the absent-Service install request is not immediately cfg-gated to macOS"
                bad "  it must not change Windows behaviour, where absence means removal" ;;
        esac
    fi

    # The refusal wording. It used to say "Install the service from Settings", which
    # macOS does not render — a student followed it, found no such control, and had
    # nowhere left to go. A message naming UI that does not exist is worse than no
    # message, because it sends someone looking.
    REFUSAL="$REPO/client/src-tauri/src/cmd/chrysalis.rs"
    if [ ! -f "$REFUSAL" ]; then
        warn "no cmd/chrysalis.rs — this guard cannot run"
    else
        # Scoped to the CONSTANT, not the file. The explanatory comments above it
        # quote the old wording to say why it was wrong, and a bare file-wide grep
        # matches those — so removing the string while keeping its rationale would
        # read as a failure, which is backwards. This extracts the value only.
        refusal_value="$(python3 - "$REFUSAL" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
m = re.search(r'TUN_UNAVAILABLE_MESSAGE\s*:\s*&str\s*=\s*"(.*?)";', text, re.S)
print(m.group(1) if m else "")
PY
    )"

        if [ -z "$refusal_value" ]; then
            bad "TUN_UNAVAILABLE_MESSAGE was not found in cmd/chrysalis.rs"
            bad "  if it was renamed, update this guard so the wording stays checked"
        elif printf '%s' "$refusal_value" | grep -q 'from Settings'; then
            bad "the TUN refusal still tells the student to install the service 'from Settings'"
            bad "  macOS renders no such control; the message must name the permission"
            bad "  prompt the app raises instead"
        else
            ok "the refusal does not point at a Settings control that may not exist"
        fi
    fi
}
