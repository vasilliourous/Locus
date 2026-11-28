#!/usr/bin/env bash
# Section 57 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_57() {
    # ─────────────────────────────────────────────────────────────
    echo "57. No single file grows past the size an agent can read"
    #
    # ── WHY THIS EXISTS ──
    #
    # The cost this repository pays for a change is dominated by how much an agent
    # must READ to make it safely, not by the code it writes. `check-consistency.sh`
    # reached 9,482 lines in one file; a task touching one guard meant reading all
    # of it, and every repository-wide search landed inside it. Splitting it was a
    # repair, and a repair with no guard behind it recurs -- that is the pattern
    # §1 through §56 exist for.
    #
    # WHAT IT ENFORCES: two things, because size alone is the wrong measure.
    #
    #   1. A CEILING ON FILE SIZE for files that carry the project's rules and
    #      build logic. The threshold is not a style preference: it is the point
    #      past which a targeted read ("show me the part about the updater") stops
    #      being possible without loading the whole file.
    #
    #   2. NAVIGABILITY, which is the rule that actually matters and which a line
    #      count cannot see. A 3,800-line workflow whose steps are 57 `- name:`
    #      lines is CHEAP: `rg 'name: Sign the artifact'` returns one line and the
    #      reader takes 100 of them. A 2,000-line file with no headings is
    #      EXPENSIVE: it must be read whole, because there is no key to grep.
    #      So a file over the ceiling is allowed if -- and only if -- it is
    #      navigable: it carries enough structural markers (`## `, `- name:`,
    #      `#[tauri::command]`, `fn `, `echo "N.`) that a targeted read works.
    #
    # WHY THAT DISTINCTION IS THE WHOLE POINT: the four files that tripped the raw
    # ceiling in this repository -- client.yml, FIXES.md, STILL-OPEN.md, and the
    # 943-line `mod tests` in cmd/chrysalis.rs -- were each checked with the
    # retrieval query a reader would actually run, and every one answered in
    # single-digit milliseconds for under a kilobyte. Splitting them would have
    # added a "which file is it in?" step to a search that already worked.
    #
    # WHAT IT IS NOT: a limit on how much WISDOM a file may contain. This
    # repository's comments are load-bearing -- they record which bug a line
    # prevents, and deleting them to hit a line count would be the worst possible
    # optimisation. The remedy when this fires is to move a section into its own
    # FILE with its comments intact, never to delete the reasoning.
    #
    # WHY THESE PATHS: they are the ones an agent opens by name. A 150 KB generated
    # IPC types file or a lockfile is not on that list, and would be a false
    # positive -- the exclusions below are exact, not a widening of the rule.
    # ─────────────────────────────────────────────────────────────

    # The ceiling. 1,500 lines is roughly where "read the relevant part" stops
    # working and "read the file" begins.
    _max_lines=1500

    # WHY THERE IS NO ALLOWLIST. An earlier version of this check carried one, and
    # four files landed in it: FIXES.md, STILL-OPEN.md, client.yml and a one-shot
    # i18n migration. Each was justified in writing -- and each justification was
    # answering the wrong question ("is this file valuable?") instead of the one
    # that matters ("can a reader get INTO it?"). Every one of them turned out to
    # be navigable: a heading grep, a `- name:` grep and a `fn` grep each answered
    # in single-digit milliseconds for under a kilobyte. An allowlist would have
    # recorded four decisions that no longer needed making, and would have hidden
    # the day one of them stopped being navigable.
    #
    # The navigability test below is the rule. A file that passes does not need an
    # exception; a file that fails does not get one.

    # How many structural markers a large file needs before a targeted read is
    # possible. A marker is a line a reader can grep for to land INSIDE the file:
    # a markdown heading, a CI step name, a Tauri command, a top-level fn, or a
    # numbered section banner. Roughly one per 100 lines means a reader can always
    # get within a screen of the part they want.
    _min_markers_per_100=1

    _too_big=0
    _checked=0
    _navigable=0

    # The rule-carrying and build-logic files. `checks/*.sh` is included so a new
    # section cannot quietly become the next 9,000-line file.
    for _f in \
        "$REPO/server/scripts/check-consistency.sh" \
        "$REPO"/server/scripts/checks/*.sh \
        "$REPO/server/scripts"/*.sh \
        "$REPO"/client/scripts/*.mjs \
        "$REPO/.github/workflows"/*.yml \
        "$REPO/AGENTS.md" \
        "$REPO/client/AGENTS.md" \
        "$REPO/docs/operate"/*.md \
        "$REPO/docs/reference"/*.md
    do
        [ -f "$_f" ] || continue
        _rel=${_f#"$REPO"/}
        _checked=$((_checked + 1))

        _lines=$(wc -l < "$_f" 2>/dev/null || echo 0)
        [ "$_lines" -gt "$_max_lines" ] || continue

        # A file over the ceiling is only a defect if it cannot be NAVIGATED.
        # This is the rule the ceiling is a proxy for -- see "WHAT IT ENFORCES".
        # The marker set is per-language on purpose: a JavaScript file has no
        # `fn`, and a markdown file has no `function`. Missing a language makes
        # the check fire on correct input, which is the failure §7 warns about --
        # a guard that cries wolf gets disabled.
        _markers=$(grep -cE \
            '^\s*(#{2,} |- name: |#\[tauri::command\]|pub (async )?fn |fn |echo "[0-9]+|(export )?(async )?function |const [A-Za-z_$]+ = (async )?\()' \
            "$_f" 2>/dev/null || echo 0)
        _needed=$(( _lines * _min_markers_per_100 / 100 ))
        if [ "$_markers" -ge "$_needed" ] && [ "$_markers" -gt 0 ]; then
            _navigable=$((_navigable + 1))
            continue
        fi

        bad "$_rel is $_lines lines with only $_markers structural marker(s) — a"
        bad "  reader cannot grep into it, so they must read the whole thing. Add"
        bad "  headings/step names, or split it into files that each have some."
        bad "  DO NOT delete comments to fit: they record which bug the code prevents."
        _too_big=1
    done

    if [ "$_too_big" -eq 0 ]; then
        if [ "$_navigable" -gt 0 ]; then
            ok "every rule-carrying file is under ${_max_lines} lines, or navigable ($_checked checked, $_navigable large but navigable)"
        else
            ok "every rule-carrying file is under ${_max_lines} lines ($_checked checked)"
        fi
    fi

    # ── The split itself must stay honest. ──
    #
    # A section file that no line of ORDER names never runs, and nothing else
    # notices: the summary counts only what executed, so a guard silently dropped
    # from ORDER makes the run GREENER, not redder. This is the failure mode the
    # §53 comment already records once in this repository's history.
    # Matching on "${name%%-*}" is WRONG: a section number can itself contain a
    # hyphen (`1g-bis`), so that yields `1g` and reports a file that IS listed as
    # an orphan. Caught by running the guard against the tree it was written for.
    # Match the ORDER file's second field (the function name) against the function
    # the file actually declares.
    _orphans=""
    for _f in "$REPO"/server/scripts/checks/*.sh; do
        [ -f "$_f" ] || continue
        _fn=$(grep -m1 -oE '^section_[a-z0-9_]+' "$_f" 2>/dev/null)
        [ -n "$_fn" ] || continue
        if ! grep -qE "[[:space:]]${_fn}[[:space:]]*$" "$REPO/server/scripts/checks/ORDER" 2>/dev/null; then
            _orphans="$_orphans $(basename "$_f")"
        fi
    done

    if [ -n "$_orphans" ]; then
        bad "section file(s) not listed in checks/ORDER, so they never run:$_orphans"
        bad "  An unlisted section is silently skipped and the run still says PASS."
    else
        ok "every section file is listed in checks/ORDER"
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
