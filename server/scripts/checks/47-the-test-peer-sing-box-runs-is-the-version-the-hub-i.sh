#!/usr/bin/env bash
# Section 47 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_47() {
    echo "47. The test peer sing-box runs is the version the hub installs"
    # ─────────────────────────────────────────────────────────────
    # WHY THIS EXISTS
    #
    # `tunnel_end_to_end.rs` runs a REAL sing-box as the student's tunnel peer, and
    # that is only meaningful if it is the binary the hub runs. `02-shadowsocks.sh`
    # pins `SING_BOX_VERSION="${SING_BOX_VERSION:-v1.12.1}"`; the workflow's install
    # step said `version=1.14.4` and nothing in the tree compared them. The
    # shadowsocks-rust pin one step below already carries this exact argument in its
    # comment ("a test peer on a different version can accept a config the deployed
    # one rejects") — it just had no check behind it either.
    #
    # WHAT IT ASSERTS
    #
    #   (a) The workflow reads its version FROM the hub's module rather than naming
    #       one of its own. A named literal is the thing that drifted.
    #   (b) The workflow's own `sed` expression, run against the module, yields the
    #       same value this check read. A silent empty extraction would build a URL
    #       that 404s and fail with a message about the network.
    #   (c) The step RUNS the tool and asserts the version it reports. `sing-box
    #       version` on its own prints and continues.
    #
    # (a) is a POSITIVE test (`version=` must be assigned from that file) because the
    # obvious negative one — `grep -q 'server/modules/02-shadowsocks.sh'` — passed on
    # a pre-fix workflow whose only mention of the path was in a COMMENT. All four
    # failure modes were then observed failing: a hardcoded version, the module's
    # line shape drifting, the extraction removed, and the run-and-check dropped.
    #
    # WHAT IT CANNOT DO: `install_singbox` skips when a binary already exists at
    # `/usr/local/bin/sing-box`, so a box that once had another version keeps it.
    # The deployed peer is therefore not guaranteed to be the pin — that is
    # `STILL-OPEN.md`'s business, not this check's.
    # ─────────────────────────────────────────────────────────────
    SINGBOX_MODULE="$REPO/server/modules/02-shadowsocks.sh"
    if [ ! -f "$SINGBOX_MODULE" ]; then
        bad "02-shadowsocks.sh not found; cannot check the sing-box peer pin"
    elif [ ! -f "$WORKFLOW" ]; then
        bad "workflow not found at ${WORKFLOW}; cannot check the sing-box peer pin"
    else
        # Same extraction the workflow runs. `sed -n` rather than a regex over the
        # whole file so a commented-out line cannot supply the value.
        hub_singbox="$(sed -n 's/^SING_BOX_VERSION="\${SING_BOX_VERSION:-\(v[0-9.]*\)}".*/\1/p' "$SINGBOX_MODULE")"
        if [ -z "$hub_singbox" ]; then
            bad "could not read SING_BOX_VERSION from 02-shadowsocks.sh"
            bad "  The workflow now extracts it from that exact line; if the line's"
            bad "  shape changed, the CI install step would fetch nothing."
        else
            ok "the hub's sing-box pin is readable (${hub_singbox})"

            # The step's body, from its name line to the next `- name:`.
            SINGBOX_STEP="$(awk '
                /^      - name: Install sing-box/ { grab = 1 }
                grab && /^      - name: / && !/Install sing-box/ { exit }
                grab { print }
            ' "$WORKFLOW")"
            if [ -z "$SINGBOX_STEP" ]; then
                bad "could not find the 'Install sing-box' step in the workflow"
                bad "  An empty step would make every assertion below vacuous."
            else
                # (a) The value must be READ from the module, not named here.
                #
                # A bare `grep -q 'server/modules/02-shadowsocks.sh'` passed on the
                # FIRST draft of this check, on a pre-fix workflow whose only mention
                # of the module's path was in a COMMENT — exactly the false pass
                # §46 hit with `index($0, "runner.os }}")`. The test is therefore a
                # positive one: `version=` must be assigned FROM that file.
                if printf '%s' "$SINGBOX_STEP" \
                    | grep -qE '^ *version="?\$\(sed -n .*server/modules/02-shadowsocks\.sh'; then
                    ok "the CI sing-box version is read from the hub's own module"
                else
                    bad "the CI sing-box install names its own version instead of the hub's"
                    bad "  It used to say 1.14.4 while the hub installs ${hub_singbox}."
                    bad "  Read SING_BOX_VERSION from server/modules/02-shadowsocks.sh."
                fi

                # (b) The step must run from the REPO ROOT.
                #
                # Every other step in the `build` job inherits
                # `defaults.run.working-directory: client`, and this one reads a path
                # under `server/`. Without the override sed ran from `client/` and
                # reported `can't read server/modules/02-shadowsocks.sh: No such file
                # or directory` — which reads exactly like a deleted file, and was
                # first diagnosed as one. Assertions (a) and (c) both passed on that
                # workflow, because they check the step's TEXT and neither of them
                # cares which directory the text runs in.
                #
                # The test is a POSITIVE one: the step must carry
                # `working-directory: .`. A bare grep for the string would also match
                # the comment explaining it, which is the false pass §47(a) hit with
                # the module's path, so the key line is matched as a key.
                step_dir="$(printf '%s\n' "$SINGBOX_STEP" \
                    | sed -n 's/^ *working-directory: *\(.*\)$/\1/p' | head -1)"
                if [ "$step_dir" = "." ]; then
                    ok "the CI sing-box step runs from the repository root, where server/ is"
                else
                    bad "the CI sing-box step does not set 'working-directory: .'"
                    bad "  This job defaults to client/, so 'server/modules/…' resolves"
                    bad "  to client/server/modules/… and sed fails before any download."
                    bad "  That is the linux build failure of run 42656242564."
                fi

                # (c) The extraction the workflow performs must yield a real version.
                # The workflow's `sed` EXPRESSION is lifted verbatim and executed
                # against the same module, so the two cannot agree with each other
                # while disagreeing with the pin. The module is resolved from the
                # step's OWN working directory — the one (b) just pinned — so a step
                # that runs from the wrong place fails here as well, at the point
                # where the value actually stops resolving.
                #
                # The first draft re-expressed the pattern here instead of lifting it,
                # and the re-expression matched nothing — the check compared itself
                # against itself and both looked right. The backslashes are part of
                # the lifted text and are NOT stripped: `\(` and `\1` are what the
                # expression means, and removing them silently yields ''. Verified by
                # changing only the workflow's pattern (the module untouched): this
                # line then reports '' and fails.
                ci_singbox_expr="$(printf '%s\n' "$SINGBOX_STEP" \
                    | sed -n "s/^ *version=\"\$(sed -n '\(.*\)' server\/modules\/02-shadowsocks\.sh)\".*/\1/p" \
                    | head -1)"
                if [ -z "$ci_singbox_expr" ]; then
                    ci_singbox="<no extraction found>"
                else
                    # Run from the step's directory, exactly as CI would, rather than
                    # from $REPO. That is the difference between this check testing
                    # the expression and testing the expression AS PLACED.
                    #
                    # The fallback is the job's own default (`client`), NOT `.`:
                    # falling back to the root would make this assertion pass on
                    # exactly the workflow that broke — it would read the module from
                    # a directory CI never runs in. With `client` the extraction
                    # yields nothing, which is the truth.
                    ci_singbox="$(cd "$REPO/${step_dir:-client}" 2>/dev/null \
                        && sed -n -e "$ci_singbox_expr" server/modules/02-shadowsocks.sh 2>/dev/null \
                        | head -1)"
                    ci_singbox="v${ci_singbox#v}"
                    [ "$ci_singbox" = "v" ] && ci_singbox=""
                fi
                if [ "$ci_singbox" = "$hub_singbox" ]; then
                    ok "the extraction the workflow performs agrees with the module (${ci_singbox})"
                else
                    bad "the workflow's extraction yields '${ci_singbox}', the module says '${hub_singbox}'"
                    bad "  Either the sed pattern here drifted, or the module's line shape did."
                fi

                if printf '%s' "$SINGBOX_STEP" | grep -q 'sing-box version | grep -qF'; then
                    ok "the installed sing-box is executed and its version asserted"
                else
                    bad "the step never checks the version it installed"
                    bad "  'sing-box version' alone prints and continues; a runner image"
                    bad "  or a cached binary on another version would pass silently."
                fi
            fi
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
