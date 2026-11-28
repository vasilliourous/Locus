#!/usr/bin/env bash
# Section 6 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_6() {
    # ─────────────────────────────────────────────────────────────
    # 6. PocketBase hook traps.
    #
    # This project has lost more time to goja/PocketBase specifics than to logic
    # errors, and each one failed SILENTLY — a rate limit that never ran, an audit
    # column that recorded nothing, a helper that 500'd a whole endpoint for its
    # entire life. Each is mechanical enough to check:
    #
    #   a. File-scope helper functions are invisible inside routerAdd callbacks
    #      ("helperFn is not defined"). Every hook here declares helpers INSIDE the
    #      callback for this reason; a file-level `function` is a latent 500.
    #   b. `findRecordsByFilter` returns zero rows with no error on PB 0.22.21. Any
    #      use of it is a silent no-op — the working calls are findFirstRecordByFilter
    #      and findRecordsByExpr.
    #   c. Admin token comparison must not use `!==`/`==` on the secret, which exits
    #      at the first differing byte.
    #   d. A hook that claims to read `X-Admin-Token` must read it from the REQUEST
    #      headers. `$apis.requestInfo(e).data` carries the body only, and
    #      `e.request().header.get(...)` reads the key-value store, which never holds
    #      headers — so such a fallback is dead code that looks like a feature.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "6. PocketBase hook traps"

    HOOKS_DIR="$REPO/server/pb_hooks"

    # (a) A file-scope `function name()` — i.e. one at column 0 — is not visible to
    # routerAdd callbacks. Inline helpers are indented inside the callback.
    file_scope_fn=$(grep -rn '^function ' "$HOOKS_DIR"/*.js 2>/dev/null || true)
    if [ -z "$file_scope_fn" ]; then
        ok "no file-scope function declarations in any hook"
    else
        bad "file-scope function declaration(s) — invisible inside routerAdd callbacks:"
        printf '%s\n' "$file_scope_fn" | sed 's/^/         /'
    fi

    # (b) findRecordsByFilter is broken in this PB build.
    #
    # Match only an ACTUAL call, not a comment describing the trap. Every hook here
    # carries a warning comment naming this function, so a naive grep reports the
    # documentation as the defect. Stripping to code before the comment marker, and
    # requiring the call syntax `findRecordsByFilter(`, is what makes this usable.
    broken_filter=$(
        for f in "$HOOKS_DIR"/*.js; do
            [ -f "$f" ] || continue
            # `sed 's://.*::'` drops `//` line comments, so prose cannot match.
            code=$(sed 's://.*::' "$f")
            hit=$(printf '%s\n' "$code" | grep -n 'findRecordsByFilter(' || true)
            [ -n "$hit" ] && printf '%s: %s\n' "$f" "$hit"
        done
    )
    if [ -z "$broken_filter" ]; then
        ok "no use of findRecordsByFilter (silently returns zero rows on this build)"
    else
        bad "findRecordsByFilter called — returns zero rows with no error:"
        printf '%s\n' "$broken_filter" | sed 's/^/         /'
    fi

    # (c) Token comparisons must be constant-time, not `!==`.
    plain_compare=$(grep -rnE '(adminToken|supplied|admin_token)[[:space:]]*!==[[:space:]]*validToken|validToken[[:space:]]*!==[[:space:]]*(adminToken|supplied)' "$HOOKS_DIR"/*.js 2>/dev/null || true)
    if [ -z "$plain_compare" ]; then
        ok "admin token compared without an early-exit (constantTimeEquals)"
    else
        bad "admin token compared with !== — leaks match length by timing:"
        printf '%s\n' "$plain_compare" | sed 's/^/         /'
    fi

    # (d) A hook must not claim a header fallback it cannot honour. Look for the
    # dead pattern specifically: reading a hyphenated header name out of the
    # requestInfo key-value store.
    dead_header=$(grep -rn 'requestInfo(e)\.data\|\.header\.get("X-\|\.header\.get(.X-' "$HOOKS_DIR"/*.js 2>/dev/null | grep -E 'header\.get\("X-' || true)
    if [ -z "$dead_header" ]; then
        ok "no header read through the requestInfo key-value store"
    else
        bad "header read through e.request().header.get() — that store holds the body, not headers:"
        printf '%s\n' "$dead_header" | sed 's/^/         /'
    fi

    # (e) RETIRED: the device-binding normalisation check.
    #
    # This used to assert that any hook writing a fingerprint into `device_bindings`
    # or `codes.bound_fingerprint` normalised it first, because a raw/stripped
    # mismatch made the "one code per device" uniqueness check silently stop
    # matching. Both the binding and the index are gone (codes are single-use and
    # not device-bound — see activation.pb.js), so the pattern it grepped for no
    # longer exists anywhere in the tree.
    #
    # It is removed rather than left in place on purpose: a guard whose pattern can
    # never match reads exactly like a guard that passed, which is the failure mode
    # this project has paid for more than once (§(a) of DEBUGGING-METHOD.md). The
    # replacement below checks the invariant that exists now.

    # (e2) A code is single-use, and `activated_at` is the whole of that rule.
    #
    # WHY THIS IS A CHECK. The rule is easy to break by accident in a hook that
    # "helpfully" clears the stamp — an operator release legitimately does, but an
    # expiry or a suspension must NOT, because clearing `activated_at` would make a
    # code the student is using look unused and available for a second sale.
    #
    # So: only the release paths may clear `activated_at`. Every other hook is
    # refused. `admin_unbind.pb.js` and the console's codes.unbind are the two
    # places an operator deliberately releases a code.
    stray_clear=$(grep -rnE 'set\("activated_at",\s*(null|"")\s*\)' "$HOOKS_DIR"/*.js 2>/dev/null \
        | grep -v 'admin_unbind\.pb\.js' \
        | grep -v 'admin_console\.pb\.js' || true)
    if [ -z "$stray_clear" ]; then
        ok "only the release paths clear the single-use stamp"
    else
        bad "a hook outside the release paths clears codes.activated_at — that makes a"
        bad "code a student is using look unused, and it can then be sold twice:"
        printf '%s\n' "$stray_clear" | sed 's/^/         /'
    fi

    # (e3) No hook may read or write the retired device-binding fields on a code.
    #
    # `bound_fingerprint` is a frozen column on an existing hub (kept so an operator
    # can still inspect history) but nothing may go on using it, or the retired
    # device model would quietly creep back into the activation path.
    used_binding=$(grep -rn 'bound_fingerprint' "$HOOKS_DIR"/*.js 2>/dev/null \
        | grep -v '^\s*//' \
        | grep -vE ':\s*(//|\*)' || true)
    if [ -z "$used_binding" ]; then
        ok "no hook uses the retired bound_fingerprint field"
    else
        bad "a hook still reads or writes codes.bound_fingerprint:"
        printf '%s\n' "$used_binding" | sed 's/^/         /'
    fi

    # (f) A hook must not call a Web/Node builtin that goja does not define.
    #
    # WHY THIS IS A CHECK AND NOT A COMMENT. `admin_console.pb.js` refused EVERY
    # release for weeks with "signature not base64 of a minisign .sig file", naming
    # the fetch as the culprit. The real cause was `atob(...)`: PocketBase's goja
    # runtime defines no `atob`, `btoa`, `Buffer` or `TextDecoder`, so the call threw
    # `ReferenceError`, the enclosing `try/catch` turned the crash into a `false`
    # verdict, and every value — correct ones included — was judged invalid.
    #
    # Nothing could have caught it before runtime. It parses as JavaScript, `node
    # --check` is happy, and every static check here reads shapes, not semantics.
    # What makes it a guard rather than a warning is that the class of bug is
    # knowable in advance: these four names simply do not exist in this runtime.
    #
    # The comment-only exclusion is not cosmetic — this file explains the trap in
    # prose at the call site, so a naive grep reports the documentation as the defect.
    goja_builtins=""
    for f in "$HOOKS_DIR"/*.js; do
        [ -f "$f" ] || continue
        hit=$(sed 's://.*::' "$f" | grep -nE '(^|[^.A-Za-z0-9_$])(atob|btoa|TextDecoder|TextEncoder)[[:space:]]*\(|\bBuffer\.(from|alloc|concat)\(' || true)
        [ -n "$hit" ] && goja_builtins="${goja_builtins}${f}: ${hit}
    "
    done
    if [ -z "$goja_builtins" ]; then
        ok "no hook calls a builtin goja does not define (atob/btoa/Buffer/TextDecoder)"
    else
        bad "a hook calls a builtin this runtime does not define — it throws at runtime,"
        bad "and a surrounding try/catch turns that crash into a wrong verdict:"
        printf '%s' "$goja_builtins" | sed 's/^/         /'
        bad "  use the hand-rolled base64ToBinary() in admin_console.pb.js instead"
    fi
}
