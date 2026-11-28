#!/usr/bin/env bash
# Section 56 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_56() {
    # ─────────────────────────────────────────────────────────────
    echo "56. The NSIS migration table matches the Rust identity table"
    #
    # ── WHY THIS EXISTS ──
    #
    # A release that updated a device from the old product identity installed a
    # SECOND application and stranded the student's activation code in the older
    # one's data root. The fix is split across two languages, because the erasing
    # happened in an installer this repository does not compile:
    #
    #   * `src/chrysalis/update/identity_table.rs` — what the client looks for, so it
    #     can refuse to hand over while a predecessor is running.
    #   * `packages/windows/migrate-predecessor.nsh` — what the installer moves out
    #     of the way, under the elevation only the installer has.
    #
    # Miss a row in the first and the client hands over to an installer that is about
    # to race a running app it cannot see. Miss it in the second and the installer
    # leaves the predecessor installed — the duplicate-application defect again, with
    # nothing anywhere reporting it, because on Windows the process is gone by then
    # (tauri-plugin-updater 2.11.0 calls `process::exit(0)` right after
    # `ShellExecuteW`).
    #
    # ── WHAT THIS CHECKS ──
    #
    # The Rust side is read by RUNNING `cargo run --bin predecessors`, not by
    # re-grepping the file. The binary `include!`s the same
    # `identity_table.rs` the client compiles, so the guard sees what the compiler
    # sees and cannot be fooled by a comment or a moved definition.
    #
    # Both directions, plus order, plus the NSIS row count — because a table whose
    # COUNT disagrees with its rows truncates the installer's loop silently, and NSIS
    # has no arrays to catch it.
    #
    # It deliberately does not try to compile the macro: `makensis` is only present on
    # the Windows runner, and a check that skips where it cannot run is the "SKIP !=
    # PASS" failure `docs/reference/DEBUGGING-METHOD.md` §3 names. What it CAN do is
    # make the two lists unable to drift.
    # ─────────────────────────────────────────────────────────────
    PRED_TABLE="$REPO/client/src-tauri/src/chrysalis/update/identity_table.rs"
    PRED_HOOK="$REPO/client/src-tauri/packages/windows/migrate-predecessor.nsh"

    if [ ! -f "$PRED_TABLE" ]; then
        bad "identity_table.rs is missing - the predecessor list has no source of truth"
    elif [ ! -f "$PRED_HOOK" ]; then
        bad "packages/windows/migrate-predecessor.nsh is missing - a cross-grade update"
        bad "  would install a second application and strand the activation code"
    else
        # The Rust table, as the compiler sees it. `cargo run` is quiet so a warning
        # from another crate cannot be mistaken for a row.
        #
        # stderr is KEPT, not discarded. It used to go to /dev/null, and that is why
        # this check reported "the predecessors binary produced no rows" on every CI
        # run when the truth was "no Rust toolchain in the job that ran it" — the one
        # message a reader could act on was thrown away and the one that remained
        # named the wrong cause. The tail is printed ONLY on failure, so a clean run
        # stays quiet. See `.github/workflows/client.yml` (the step moved to
        # `build (linux)` on 2026-11-26) and F-21, the same mistake in the same job.
        rust_err=$(mktemp)
        rust_rows=$(cd "$REPO/client" && cargo run --quiet --bin predecessors 2>"$rust_err")
        if [ -z "$rust_rows" ]; then
            bad "the predecessors binary produced no rows - it did not run, or the table is empty"
            bad "  (run: cd client && cargo run --bin predecessors)"
            if [ -s "$rust_err" ]; then
                bad "  cargo said:"
                tail -20 "$rust_err" | sed 's/^/         /'
            else
                bad "  cargo said nothing at all - if this is CI, suspect a job with no"
                bad "  Rust toolchain: it compiles nothing and reports that as empty output."
            fi
        fi
        rm -f "$rust_err"
        if [ -n "$rust_rows" ]; then
            nsis_rows=$(grep -cE '^!define MIGRATE_[0-9]+_APPID ' "$PRED_HOOK" || true)
            rust_count=$(printf '%s\n' "$rust_rows" | grep -c . || true)
            # Per-row drift must not be summarised away by a count that happens to
            # agree. Demonstrated failing: with a single wrong value in the hook this
            # guard printed a green OK underneath two BAD lines, which is the
            # "green tick next to a failure" shape this repository has already paid
            # for twice. The OK is emitted only when the count matches AND no row
            # drifted.
            rows_clean=1

            # The count define, which turns a mismatch into a silent truncation.
            nsis_count=$(sed -n 's/^!define MIGRATE_COUNT \([0-9][0-9]*\)$/\1/p' "$PRED_HOOK" | head -1)
            if [ -z "$nsis_count" ]; then
                bad "the NSIS hook has no \`!define MIGRATE_COUNT\` - its loop bound is undefined"
            elif [ "$nsis_count" != "$nsis_rows" ]; then
                bad "MIGRATE_COUNT is $nsis_count but the hook defines $nsis_rows APPID rows"
                bad "  NSIS has no arrays: a wrong count truncates the loop with no error."
            fi

            if [ "$rust_count" != "$nsis_rows" ]; then
                bad "the Rust table has $rust_count identities and the NSIS hook has $nsis_rows"
                bad "  A row in one and not the other means a fleet either races a running"
                bad "  predecessor (Rust-only) or keeps the duplicate app installed (NSIS-only)."
            fi

            # Per-row, BY CONTENT AND BY ORDER. Order matters because both sides say
            # most-recent-first, and the installer walks the list in that order.
            i=1
            while IFS="$(printf '\t')" read -r app_id product executable; do
                [ -n "$app_id" ] || continue
                for field in APPID PRODUCT EXE; do
                    case "$field" in
                        APPID) want="$app_id" ;;
                        PRODUCT) want="$product" ;;
                        EXE) want="$executable" ;;
                    esac
                    got=$(sed -n "s/^!define MIGRATE_${i}_${field} \"\\(.*\\)\"$/\\1/p" "$PRED_HOOK" | head -1)
                    if [ -z "$got" ]; then
                        bad "MIGRATE_${i}_${field} is missing from the NSIS hook (Rust row $i: $app_id)"
                    elif [ "$got" != "$want" ]; then
                        bad "row $i ${field}: Rust says '$want', the NSIS hook says '$got'"
                        bad "  Row ORDER is significant on both sides - they are compared positionally."
                        rows_clean=0
                    fi
                done
                i=$((i + 1))
            done <<EOF
$rust_rows
EOF

            if [ "$rust_count" = "$nsis_rows" ] && [ "$rows_clean" -eq 1 ]; then
                ok "the NSIS migration table matches the Rust identity table ($rust_count rows)"
            fi
        fi
    fi

    echo
}
