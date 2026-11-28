#!/usr/bin/env bash
# Section 4 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_4() {
    # ─────────────────────────────────────────────────────────────
    # 4. The cipher is defined exactly once.
    #
    # The SS2022 migration collapsed ten copies into one. This keeps it collapsed:
    # a bare quoted SS2022 cipher string anywhere except modules/00-env.sh and the
    # templates (which are declarative references, not config writers) is drift.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "4. Cipher single-definition"

    # ── THIS GUARD NAMES THE STRING IT BANS, SO IT MUST NOT READ ITSELF ──
    #
    # The pattern lines below contain the banned literal and a quoted one at that.
    # They were excluded by `grep -v 'check-consistency.sh'` until the sections
    # were split into this directory; after the split, the exemption named a file
    # that no longer held the patterns, and this guard reported its own two grep
    # lines as drift on 2026-11-28. `lib/guard_self.py` is the one rule every
    # such guard shares: a comment in `server/scripts/checks/` may name a banned
    # string, a code line may not — here or anywhere. See its docstring.
    cipher_hits=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys
sys.path.insert(0, os.path.join("server", "scripts", "lib"))
from guard_self import is_explanation

# The banned shape: a QUOTED SS2022 literal, which is what a config writer
# needs. `SS_METHOD` itself and the declarative templates are references.
#
# BUILT, NOT SPELLED. A regex literal here would contain the exact string this
# guard reports, and its own source line would be the first finding — which is
# what happened on 2026-11-28 when the old pattern line moved into this file.
# `guard_self.is_explanation` licenses a COMMENT to name a banned string; it
# deliberately does not license a code line, because a code line is what a real
# violation looks like. So the one place this guard must say it, says it in two
# pieces. Do not "simplify" this back into a literal.
CIPHER = "2022-" + "blake3"
LITERAL = re.compile(r'"' + CIPHER + r'|= *"' + CIPHER)
SKIP_LINE = re.compile(r'SS_METHOD\b|04-tc')

for root, dirs, files in os.walk("server"):
    dirs[:] = [d for d in dirs if d not in ("node_modules", "target")]
    for fn in files:
        if not fn.endswith((".sh", ".py")):
            continue
        p = os.path.join(root, fn).replace(os.sep, "/")
        try:
            body = open(p, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        for n, line in enumerate(body.splitlines(), 1):
            if not LITERAL.search(line):
                continue
            if SKIP_LINE.search(line):
                continue
            if is_explanation(p, line):
                continue
            print("%s:%d:%s" % (p, n, line.strip()[:100]))
PY
    )

    if [ -z "$cipher_hits" ]; then
        ok "no hardcoded SS2022 cipher outside the SS_METHOD definition"
    else
        bad "$(printf '%s\n' "$cipher_hits" | wc -l | tr -d ' ') hardcoded SS2022 cipher literal(s) outside SS_METHOD:"
        printf '%s\n' "$cipher_hits" | sed 's/^/         /'
    fi

    if grep -q 'SS_METHOD=' "$REPO/server/modules/00-env.sh" 2>/dev/null; then
        ok "SS_METHOD is defined in modules/00-env.sh"
    else
        bad "SS_METHOD is not defined in modules/00-env.sh"
    fi
}
