#!/usr/bin/env bash
# Section 3 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_3() {
    # ─────────────────────────────────────────────────────────────
    # 3. Fields the hooks WRITE must exist in the collection schema.
    #
    # This is the `unbound_at` class: PocketBase silently discards an unknown
    # field, so a hook can write faithfully for months into nothing. `seed-pb.py`
    # is the schema authority; this checks the two agree.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "3. Hook-written fields vs the schema"

    schema_fields=$(grep -oE '"name"[[:space:]]*:[[:space:]]*"[a-z0-9_]+"' "$SCRIPTS/seed-pb.py" 2>/dev/null \
        | sed 's/.*"\([a-z0-9_]*\)"$/\1/' | sort -u)

    if [ -z "$schema_fields" ]; then
        bad "could not extract any field names from seed-pb.py — has its shape changed?"
    else
        # Fields written via `.set("name", ...)` in the hooks. Deliberately excludes
        # PocketBase's own auto fields (created/updated/id) and the code_events
        # collection, which is written with the same helper.
        # Fields written via `.set("name", ...)` in the hooks. Two shapes occur:
        # a literal name, and a concatenated prefix like `"download_" + platform`
        # which expands to the schema's `download_linux`, `download_windows`, ...
        # The second is expanded here rather than skipped — it is exactly the kind
        # of dynamic write that hides a missing column.
        hook_fields=$(
            {
                # Literal field names. A trailing '_' means the name is a
                # concatenation prefix, which the block below expands instead.
                grep -rhoE '\.set\("[a-z_]+"' "$HOOKS"/*.js 2>/dev/null \
                    | sed 's/\.set("//;s/"//' \
                    | grep -v '_$'
                # Concatenated prefixes: emit one field per known platform suffix.
                for prefix in $(grep -rhoE '\.set\("[a-z0-9_]+_"' "$HOOKS"/*.js 2>/dev/null \
                        | sed 's/\.set("//;s/"//' | sort -u); do
                    for plat in linux windows macos_intel macos_arm; do
                        printf '%s%s\n' "$prefix" "$plat"
                    done
                done
            } | sort -u
        )

        missing=""
        for f in $hook_fields; do
            case "$f" in
                id|created|updated|collectionId|collectionName|expand) continue ;;
            esac
            printf '%s\n' "$schema_fields" | grep -qx "$f" || missing="$missing $f"
        done

        if [ -n "$missing" ]; then
            bad "hook(s) write field(s) that are NOT in the schema — PocketBase will"
            bad "silently discard them, so the write looks like it worked:"
            echo "        $missing"
            echo "        Add them to seed-pb.py, or stop writing them."
        else
            n=$(printf '%s\n' "$hook_fields" | wc -l)
            ok "all ${n} hook-written field(s) exist in the schema"
        fi
    fi
}
