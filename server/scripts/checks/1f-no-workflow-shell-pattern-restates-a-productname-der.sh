#!/usr/bin/env bash
# Section 1f -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_1f() {
    echo
    echo "1f. No workflow shell pattern restates a productName-derived filename"
    # ─────────────────────────────────────────────────────────────
    # 1f. NO WORKFLOW SHELL PATTERN RESTATES A `productName`-DERIVED FILENAME.
    #
    # WHY THIS EXISTS: the SAME defect shipped twice, a week apart, and both times it
    # was a shell glob thousands of lines away from the config string it depended on.
    #
    #   v3.5.0  the macOS zip assertion globbed `installer-Locus_*` while
    #           `productName` said `Chrysalis` -> "expected two macOS download zips,
    #           found 0", after all four platforms had built and signed.
    #   2026-11   the Windows-sign step globbed `installer/Locus_*_x64-setup.exe`
    #           while its OWN COMMENT above said the file is `Chrysalis_*` with no
    #           prefix -> matched nothing, exit 1 with a message naming a third
    #           spelling.
    #
    # Both were invisible to every other guard because each side was internally
    # consistent. `productName` derives FOUR filenames (the installer, the `.app`,
    # the macOS zip, and the `.dmg`), so a literal for any of them in the workflow is
    # a stale copy waiting to happen.
    #
    # THE CHECK: in the workflow, a quoted shell token that names a product-derived
    # filename must not contain a product word that disagrees with `productName`.
    # Derived-from-config uses (`${PRODUCT}_`, `${APP_NAME}`, `${mac_zip_prefix}`)
    # contain no product word at all and pass automatically — which is the point:
    # the fix is to read the value, and this guard only fires on the restating.
    _wn_product="$(python3 - "$REPO/client/src-tauri/tauri.conf.json" <<'PY2' 2>/dev/null || true
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("productName") or "")
except Exception:
    print("")
PY2
    )"
    if [ -z "$_wn_product" ]; then
        bad "1f: could not read productName, so the workflow filename patterns cannot be checked"
    else
        # Every product word this project has used for the derived filenames. A new
        # one is added here when the product is renamed; the check is "no OTHER word".
        _wn_known="Locus Chrysalis"
        _wn_bad=""
        for _wn_word in $_wn_known; do
            [ "$_wn_word" = "$_wn_product" ] && continue
            # A product word attached to a derived filename: `Locus_<x>_x64-setup`,
            # `Locus.app`, `installer-Locus_`, `Locus_<x>_<arch>.zip`.
            if grep -qE "(^|[^A-Za-z])${_wn_word}(_|\\.app)" "$WORKFLOW" 2>/dev/null; then
                _wn_line=$(grep -nE "(^|[^A-Za-z])$_wn_word(_|\\.app)" "$WORKFLOW" | grep -vE '^[0-9]+:[[:space:]]*#' | head -1)
                if [ -n "$_wn_line" ]; then
                    _wn_bad="$_wn_word"
                fi
            fi
        done
        if [ -n "$_wn_bad" ]; then
            bad "the workflow names a '${_wn_bad}' filename while productName is '${_wn_product}'"
            printf '%s\n' "$_wn_line" | sed 's/^/         /'
            bad "  productName derives the installer, .app, zip and dmg names; restating one"
            bad "  in a shell pattern is the v3.5.0 defect. Read it from the config instead:"
            # Deliberately NOT a copy-pasteable command in a double-quoted string:
            # the nested quotes need escaping, and the escaped form stopped bash from
            # parsing this whole 8,000-line script. Name the shape instead.
            bad "  pattern: read productName from client/src-tauri/tauri.conf.json via python3 -c"
        else
            ok "no workflow shell pattern restates a '${_wn_bad:-other}' product filename (productName is '${_wn_product}')"
        fi
    fi
}
