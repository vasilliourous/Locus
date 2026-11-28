#!/usr/bin/env bash
# Section 25 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_25() {
    # ─────────────────────────────────────────────────────────────
    # 25. Front-matter conformance (status is structure, not a banner)
    #
    # WHAT IT DOES NOT ENFORCE: that the declared status is TRUE. A guard cannot read
    # a design record and know it is superseded. This catches the missing field and
    # the invented value; the honest value is still a human's job.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "25. Front-matter conformance (status is structure, not a banner)"
    fm_report=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys

ALLOWED_STATUS = {"live", "reference", "design-record"}
missing, badvalue = [], []

def md_files(root):
    for dirpath, dirs, files in os.walk(root):
        if any(part in dirpath for part in ('node_modules', 'target', '.git', 'dist')):
            continue
        for fn in files:
            if fn.endswith('.md'):
                yield os.path.join(dirpath, fn)

def front_matter(path):
    """The keys in the leading fenced block, or None if there is no block."""
    with open(path, encoding="utf-8", errors="replace") as fh:
        lines = fh.read().splitlines()
    # Skip a leading H1 (the docs put the front-matter block AFTER the title).
    i = 0
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i < len(lines) and lines[i].startswith("# "):
        i += 1
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i >= len(lines) or not lines[i].startswith("```"):
        return None
    i += 1
    keys = {}
    while i < len(lines) and not lines[i].startswith("```"):
        m = re.match(r'^([a-z-]+):\s*(.*)$', lines[i].strip())
        if m:
            keys[m.group(1)] = m.group(2).strip()
        i += 1
    return keys

for root in ("docs", "client/docs"):
    for path in md_files(root):
        rel = path.replace(os.sep, "/")
        # Archived/historical material: the folder carries the status. This is the
        # rule §25 enforces for `docs/archive/` and `docs/history/`, and
        # `docs/business/redesign/archive/` follows it too — added 2026-10-05 when
        # the removed device-binding design moved there.
        if rel.startswith("docs/archive/") or rel.startswith("docs/history/") \
                or rel.startswith("docs/business/redesign/archive/"):
            continue
        keys = front_matter(path)
        if keys is None:
            missing.append(rel)
            continue
        if "audience" not in keys:
            missing.append(rel + "  (no `audience:`)")
        if "status" not in keys:
            missing.append(rel + "  (no `status:`)")
        elif keys["status"] not in ALLOWED_STATUS:
            badvalue.append(f"{rel}  status = {keys['status']!r} (allowed: {sorted(ALLOWED_STATUS)})")

for f in sorted(set(missing)):
    print("MISSING\t" + f)
for f in sorted(set(badvalue)):
    print("BADVALUE\t" + f)
PY
    )

    if [ -z "$fm_report" ]; then
        ok "every live document declares audience: and status: in its front-matter"
    else
        while IFS="$(printf '\t')" read -r kind detail; do
            case "$kind" in
                MISSING)  bad "no front-matter (or a missing key): $detail" ;;
                BADVALUE) bad "invalid status: $detail" ;;
            esac
        done <<EOF
$fm_report
EOF
        bad "Fix: give each file a fenced block after its H1 with \`audience:\` and"
        bad "\`status: live|reference|design-record\` — or move it to archive/history."
    fi
}
