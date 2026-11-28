#!/usr/bin/env bash
# Section 23 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_23() {
    echo
    echo "23. The free tier's endpoint, cap and allowance agree across the tree"
    #
    # WHY THIS EXISTS
    #
    # The free tier reuses the legacy Eco slot: port 8443, the Eco password, the
    # `tc-eco-cap.service` unit — renamed customerside only. That means FOUR files
    # describe one endpoint, in three different vocabularies:
    #
    #   server/modules/04-tc.sh      the tc cap ("1mbit") and the port (8443)
    #   server/scripts/seed-pb.py    the tier_configs rows ("free" and "eco" -> 8443)
    #   server/scripts/fix-tier-configs.py / seed-live.py   the repair/seed tools
    #   server/pb_hooks/heartbeat.pb.js   the allowance (free_allowance_mb)
    #
    # Nothing tied them together, so the free tier could be half-deployed in a way
    # that looks complete: the cap dropped here, the row forgotten there, and the
    # student gets an activation that names a tier with no server_config — a
    # "connected" app that cannot reach the internet (the exact failure class in
    # FIXES.md). This asserts the AGREEMENT, not each file separately: the point is
    # that they match, and a check on one file alone cannot tell you that.
    #
    # Every assertion below was observed FAILING against the pre-fix tree before it
    # was kept: the 5mbit cap, the missing `free` seed row, and a heartbeat with no
    # allowance field each tripped it.

    TC_MOD="$REPO/server/modules/04-tc.sh"
    SEED="$REPO/server/scripts/seed-pb.py"
    HB="$HOOKS/heartbeat.pb.js"

    if [ ! -f "$TC_MOD" ] || [ ! -f "$SEED" ] || [ ! -f "$HB" ]; then
        warn "free-tier guard: a file it needs is missing — skipping"
    else
        # (a) The 8443 tc class is capped at 1mbit, not the legacy 5mbit.
        #     Match the apply line specifically: `apply_tc_now 8443 "1:10" "1mbit"`.
        if grep -Eq 'apply_tc_now[[:space:]]+8443[[:space:]]+"1:10"[[:space:]]+"1mbit"' "$TC_MOD"; then
            ok "04-tc.sh caps the free/Eco port (8443) at 1mbit"
        else
            bad "04-tc.sh does NOT cap port 8443 at 1mbit (expected: apply_tc_now 8443 \"1:10\" \"1mbit\")"
        fi

        # (b) The reboot-persistence unit uses the same rate, or a reboot silently
        #     restores the old cap while the running class says otherwise.
        if grep -Eq 'create_tc_service[[:space:]]+"eco"[[:space:]]+"1:10"[[:space:]]+"1mbit"[[:space:]]+8443' "$TC_MOD"; then
            ok "04-tc.sh's tc-eco-cap.service uses the same 1mbit rate"
        else
            bad "04-tc.sh's tc-eco-cap.service rate disagrees with the applied cap"
        fi

        # (c) The seed carries BOTH tier names against 8443. `free` is minted
        #     against; `eco` must survive so codes already in the field resolve.
        if grep -Eq '\("free",[[:space:]]*"ECO_PASS",[[:space:]]*8443\)' "$SEED"; then
            ok "seed-pb.py seeds the \`free\` tier against port 8443"
        else
            bad "seed-pb.py has no \`free\` tier row on port 8443 — the console cannot mint a working free code"
        fi
        # INVERTED 2026-10-06. This assertion used to REQUIRE the legacy `eco` row,
        # on the reasoning that a code minted in the Eco era carries that string and
        # must keep resolving. That reasoning was sound and is now spent: the live
        # hub holds NO code with `tier = "eco"` (verified 2026-10-06), so the row was
        # a duplicate of `free` on the same port — which the console renders as a
        # second plan sharing one port, i.e. a bug every operator sees daily, buying
        # protection for a code that does not exist.
        #
        # Kept as a tombstone rather than deleted, so a reader who remembers the old
        # rule finds out why it is gone instead of re-adding the row.
        if grep -Eq '\("eco",[[:space:]]*"ECO_PASS",[[:space:]]*8443\)' "$SEED"; then
            bad "seed-pb.py still seeds the retired \`eco\` row — it duplicates \`free\` on 8443 and shows as a second plan in the console"
        else
            ok "seed-pb.py seeds one row per plan (no retired \`eco\` duplicate)"
        fi

        # (d) The heartbeat carries the allowance, and only for the free tiers.
        #     Assert the KEY, because that is the wire contract the client reads.
        # Anchor on the ASSIGNMENT — see the note on the throttle check below.
        if grep -Eq 'response\.free_allowance_mb[[:space:]]*=' "$HB"; then
            ok "heartbeat.pb.js sends free_allowance_mb"
        else
            bad "heartbeat.pb.js does not send the free-tier allowance — the client has nothing to count against"
        fi
        # Anchor on the ASSIGNMENT (`response.free_throttle_mbps =`), not a bare
        # mention: the key also appears in the file's own comments and in the
        # enforcement-version note, so a plain grep stays green with the assignment
        # deleted — a check that cannot fail, which is the defect this section
        # exists to catch. (It did exactly that on the first draft.)
        if grep -Eq 'response\.free_throttle_mbps[[:space:]]*=' "$HB"; then
            ok "heartbeat.pb.js sends free_throttle_mbps"
        else
            bad "heartbeat.pb.js does not send the free-tier throttle speed — the client has nothing to slow to"
        fi
        if grep -Eq 'tierVal === "free"' "$HB" && ! grep -Eq 'tierVal === "eco"' "$HB"; then
            ok "the allowance is gated to the free plan only"
        elif ! grep -Eq 'tierVal === "free"' "$HB"; then
            bad "the free allowance is not gated to the free plan at all — a paying tier would be metered"
        else
            bad "the allowance still accepts the retired \`eco\` tier — the row is gone, so the branch is dead weight"
        fi

        # (e) TWO-SIDED: the client must read the same key the hub writes. A hub key
        #     with no client reader is a silent no-op, which is precisely how the
        #     UoT endpoint stayed dead fleet-wide (FIXES.md 29). The heartbeat
        #     response struct is where the wire keys are declared.
        CLIENT_HB="$REPO/client/src-tauri/src/chrysalis/heartbeat.rs"
        if [ -f "$CLIENT_HB" ]; then
            # Anchor on the FIELD DECLARATION, not a bare mention: the doc comments
            # and tests also name the key, so a plain grep passes even when the
            # field itself has been renamed — a check that cannot fail is the exact
            # defect this whole section exists to avoid.
            if grep -Eq '^[[:space:]]*pub free_allowance_mb:' "$CLIENT_HB"; then
                ok "the client declares free_allowance_mb (hub key has a reader)"
            else
                bad "the hub sends free_allowance_mb but the client does not declare it — the allowance would be a silent no-op"
            fi
            if grep -Eq '^[[:space:]]*pub free_throttle_mbps:' "$CLIENT_HB"; then
                ok "the client declares free_throttle_mbps (hub key has a reader)"
            else
                bad "the hub sends free_throttle_mbps but the client does not declare it — the throttle speed would be a silent no-op"
            fi
        else
            warn "no client heartbeat.rs found — cannot check the client side of the allowance"
        fi

        # (f) The cached allowance/throttle must be READ back, not only written.
        #     A `store_*` with no reader is the same class of silent no-op as a wire
        #     key with no reader: the value arrives, is persisted, and nothing ever
        #     uses it. `throttle_mbps` is the live example of this being worth
        #     checking — it is deliberately stored-but-not-yet-applied, and the guard
        #     makes that an explicit, greppable fact rather than a missing call.
        CLIENT_STORE="$REPO/client/src-tauri/src/chrysalis/store.rs"
        if [ -f "$CLIENT_STORE" ]; then
            if grep -Eq 'pub async fn throttle_mbps\(' "$CLIENT_STORE"; then
                ok "the client can read back the stored throttle speed"
            else
                bad "the client stores the throttle speed with no reader — the value would be persisted and never used"
            fi
        fi

        # (h) THE METER MUST HAVE A WRITER, AND THE VERDICT MUST HAVE A CALLER.
        #
        # WHY THIS EXISTS, and why it is a guard rather than a test.
        #
        # The free tier's allowance shipped "BUILT": `chrysalis::usage` was written,
        # tested with a fake clock and documented as working — and the counter in the
        # field never moved. `Usage::record` and `store::store_usage` had **no
        # caller**. Every test called `record` directly, so every test passed while
        # nothing in the app ever produced a number, and the usage bar read a
        # permanent zero. A student reported exactly that: "the counter for 15GB does
        # not count up or update in any way".
        #
        # This is the failure mode unit tests are structurally blind to. Testing a
        # function proves the function works; only a guard on the WIRING can tell you
        # it is connected. It is the same class as the UoT silent no-op (FIXES.md 29)
        # and the `throttle_mbps` case directly above: a value that arrives, is
        # stored, and is never consumed.
        #
        # WHAT IT ASSERTS: that the meter exists, that its producer is called from
        # outside the meter's own tests, that the flush is awaited somewhere, and that
        # the allowance it bills against is mirrored from the heartbeat. Each of these
        # was the missing link at some point in the fix; each is greppable.
        #
        # VERIFIED TO FAIL: commenting out the `set_allowance_mb` call in
        # `runtime.rs`, or the `meter::observe` call in `traffic_probe.rs`, each
        # produces a BAD line below.
        CLIENT_METER="$REPO/client/src-tauri/src/chrysalis/meter.rs"
        CLIENT_PROBE="$REPO/client/src-tauri/src/core/manager/traffic_probe.rs"
        CLIENT_RUNTIME="$REPO/client/src-tauri/src/chrysalis/runtime.rs"
        if [ ! -f "$CLIENT_METER" ]; then
            bad "the free-tier meter (chrysalis/meter.rs) is missing — the usage counter has no producer"
        else
            if grep -Eq 'pub fn observe\(' "$CLIENT_METER"; then
                ok "the free-tier meter exposes a producer for the usage counter"
            else
                bad "the free-tier meter has no producer — the counter would never advance"
            fi
            # The producer must be CALLED from the traffic sample path. Anchor on the
            # call, not a mention: the name also appears in meter.rs's own docs.
            if [ -f "$CLIENT_PROBE" ] && grep -Eq 'meter::observe\(' "$CLIENT_PROBE"; then
                ok "the Core's traffic samples are fed into the free-tier meter"
            else
                bad "nothing feeds the Core's traffic into the meter — the allowance would read a permanent zero (the reported defect)"
            fi
            # …and the flush must be awaited, or the count lives only in memory and a
            # restart loses the whole month.
            if grep -Eq 'meter::flush\(' "$CLIENT_PROBE" || grep -Eq 'meter::flush\(' "$CLIENT_METER"; then
                ok "the meter's accumulated bytes are written to the config"
            else
                bad "the meter accumulates but never flushes — the stored window would never advance"
            fi
            # The accumulator must be DEBITED, not zeroed. Two reviewed defects lived
            # here: a `swap(0)` before the store lost the bytes on a failed write, and
            # a `store(0)` after it erased a sample that landed mid-flush. Both are
            # silent under-counting, which is the same class as the reported bug and
            # invisible from the UI.
            #
            # Anchored on the CALL EXPRESSION (`pending.fetch_sub(`), not a bare
            # `fetch_sub(` mention: the function's own doc explains the fix using that
            # word, and one of the tests names it in prose — so a looser grep stayed
            # green with the implementation replaced by a store of zero. Observed
            # failing that way before the anchor was tightened.
            if grep -Eq 'fn debit_written\(' "$CLIENT_METER" && grep -Eq 'pending\.fetch_sub\(' "$CLIENT_METER"; then
                ok "the meter debits only what it wrote, so a failed or in-flight sample is not lost"
            else
                bad "the meter does not debit its accumulator correctly — a failed flush or a mid-flush sample would silently lose bytes"
            fi
            # …and a zeroing clear must never come back in the flush path.
            if grep -Eq 'pending\.store\(0' "$CLIENT_METER"; then
                bad "the meter clears its accumulator with a store of zero — a sample landing mid-flush would be erased"
            else
                ok "the meter never zeroes its accumulator outright"
            fi
            # …and the allowance it bills against must be mirrored from the hub, or
            # `has_allowance` stays false and every sample is rebaselined unbilled.
            if [ -f "$CLIENT_RUNTIME" ] && grep -Eq 'meter::set_allowance_mb\(' "$CLIENT_RUNTIME"; then
                ok "the hub's allowance is mirrored into the meter"
            else
                bad "the hub's allowance never reaches the meter — nothing would be billed, and the counter would stay at zero"
            fi
            # The spent verdict must gate something, or "throttled" is a colour.
            if grep -Eq 'pub fn allows_new_connection\(' "$CLIENT_METER" "$REPO/client/src-tauri/src/chrysalis/usage.rs"; then
                if grep -Eq 'allows_new_connection\(' "$REPO/client/src-tauri/src/cmd/chrysalis.rs"; then
                    ok "a spent allowance is enforced on the connect path"
                else
                    bad "a spent allowance is computed but never enforced — 'throttled' would only recolour the bar"
                fi
            else
                warn "no allows_new_connection helper found — cannot check the throttle gate"
            fi
        fi

        # (g) THE PROSE ABOUT THE CAPS, not just the caps.
        #
        # WHY THIS EXISTS. The (a)–(f) checks all passed while two DOCUMENTS
        # restated the caps wrongly: the root `README.md` said "Eco 5Mbit ... tc caps
        # (5/100/200 Mbps)" — the pre-rename value the cap was CHANGED FROM — and
        # `04-tc.sh`'s own header comment said "Eco (port 8443): 5 Mbps" above code
        # that applies 1mbit. The guard checked the code and the docs drifted in
        # silence, which is the §7 lesson one level out: verifying the value is not
        # verifying what the documents SAY about it.
        #
        # THAT GUARD ENFORCED THE DUPLICATION, AND WAS ITSELF THE BUG. It required
        # `README.md` to contain the literal `tc caps (1/100 Mbps)` — so the only way
        # to pass was to keep a second hand-maintained copy of the caps in prose. The
        # copy was correct on the day the guard was written and had to be re-edited
        # by hand every time a cap moved. Its failure mode was not "the docs are
        # wrong", it was "nobody restated the caps here", which is the state
        # `docs/STATE.md` rule 4 asks for: one home per fact.
        #
        # WHAT IT ASSERTS NOW. The cap has exactly one prose home
        # (`docs/business/04-tiers.md`), backed by `state.toml` `[tiers.*].cap`, which
        # §27 recomputes from `04-tc.sh`. `README.md` is a front door and must POINT,
        # not restate. So the check is the negative: no cap string in `README.md` at
        # all. That fails on a reintroduced copy (of either the current or a retired
        # number) instead of failing on a removal.
        #
        # VERIFIED TO FAIL: re-adding "tc caps (1/100 Mbps)" to README.md, or the
        # retired "Eco 5Mbit", each produces a BAD line. See the §23(g) note.
        README_MD="$REPO/README.md"
        if [ -f "$README_MD" ]; then
            # Any "N/M Mbps" pairing of the two tiers is a restatement. Match the
            # shapes a copy actually takes, current and retired.
            if grep -Eq 'tc caps|tc caps \(|1/100 Mbps|5/100/200 Mbps|Eco 5Mbit|5M tc' "$README_MD"; then
                bad "README.md restates the tier caps — the caps have one home (04-tiers.md + state.toml [tiers.*].cap); link to it instead"
            else
                ok "README.md does not restate the tier caps (one home: 04-tiers.md + state.toml)"
            fi
            # The retired 5Mbit free-tier cap must never reappear, in any wording.
            if grep -Eq 'Eco 5Mbit' "$README_MD"; then
                bad "README.md still calls the free/Eco tier '5Mbit' — the cap was lowered to 1mbit"
            else
                ok "README.md does not restate the retired 5Mbit free-tier cap"
            fi
        fi
        # The module header comment must not contradict the code below it. Match the
        # retired "Eco (port 8443): 5 Mbps" line specifically; the applied caps are
        # checked by (a)/(b) above and by §27(a)/(b).
        if grep -Eq 'Eco \(port 8443\): 5 Mbps' "$TC_MOD"; then
            bad "04-tc.sh's header comment says 'Eco (port 8443): 5 Mbps' but the code below applies 1mbit"
        else
            ok "04-tc.sh's header comment does not contradict its own applied cap"
        fi
    fi
}
