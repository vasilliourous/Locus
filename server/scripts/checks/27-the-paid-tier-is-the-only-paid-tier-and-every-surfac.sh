#!/usr/bin/env bash
# Section 27 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_27() {
    echo
    echo "27. The paid tier is the only paid tier, and every surface agrees"
    #
    # WHY THIS EXISTS
    #
    # §23 does this for the FREE tier. The paid tier had no such treatment, and that
    # is how Stealth and Strike drifted into being the same plan at two rates for
    # months without anything noticing: four files described the paid endpoint
    # (`04-tc.sh` cap, `02-shadowsocks.sh` service, `seed-pb.py` row + udp_relay,
    # `Codes.vue` dropdown) in three vocabularies, and no check asserted they AGREED.
    #
    # The merge (docs/business/04-tiers.md §4.2.4) collapses those to one tier, and
    # this holds the collapse in place. It is written as a set of AGREEMENTS plus a
    # NEGATIVE, because the failure modes here are:
    #
    #   1. Someone re-adds a 200mbit class or a `stealth` service because a stale
    #      comment said the ladder had three tiers.
    #   2. The cap in the applied class and the cap in the reboot unit disagree, so
    #      a reboot silently restores the old rate.
    #   3. The console offers a tier the hub does not seed, so the operator can mint
    #      a code that can never resolve.
    #
    # Every assertion below was observed FAILING against the pre-merge tree before it
    # was kept, and against each half-merged state (the 200mbit cap, the surviving
    # `stealth` seed row, the console's stale dropdown literal).
    PAID_PORT=8445
    PAID_CAP=100mbit
    PAID_ROW=strike

    if [ ! -f "$TC_MOD" ] || [ ! -f "$SEED" ]; then
        warn "paid-tier guard: a file it needs is missing — skipping"
    else
        # (a) The applied cap. Anchored on the APPLY LINE, not a bare mention: the
        #     number also appears in comments, so a plain grep stays green with the
        #     assignment deleted — a check that cannot fail.
        if grep -Eq "apply_tc_now[[:space:]]+${PAID_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${PAID_CAP}\"" "$TC_MOD"; then
            ok "04-tc.sh caps the paid port (${PAID_PORT}) at ${PAID_CAP}"
        else
            bad "04-tc.sh does NOT cap port ${PAID_PORT} at ${PAID_CAP} (expected: apply_tc_now ${PAID_PORT} \"1:30\" \"${PAID_CAP}\")"
        fi

        # (b) The reboot unit uses the same rate, or a reboot silently restores the
        #     old cap while the running class says otherwise.
        if grep -Eq "create_tc_service[[:space:]]+\"${PAID_ROW}\"[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${PAID_CAP}\"[[:space:]]+${PAID_PORT}" "$TC_MOD"; then
            ok "04-tc.sh's tc-${PAID_ROW}-cap.service uses the same ${PAID_CAP} rate"
        else
            bad "04-tc.sh's tc-${PAID_ROW}-cap.service rate disagrees with the applied cap"
        fi

        # (c) THE NEGATIVE — the retired rate must not come back. This is the
        #     property that actually protects the fleet: `200mbit` anywhere in the
        #     module means someone has restored the tier the merge deleted.
        if grep -Eq '200mbit' "$TC_MOD"; then
            bad "04-tc.sh still names a 200mbit cap — the retired second paid tier has come back"
        else
            ok "04-tc.sh names no retired 200mbit cap"
        fi

        # (d) The seed carries the paid row, with UDP advertised. Assert the ROW and
        #     the udp flag together: a row without udp_relay sells a gaming plan that
        #     does not do gaming, which is the FIXES.md-29 class of silent no-op.
        if grep -Eq "\(\"${PAID_ROW}\",[[:space:]]*\"STRIKE_PASS\",[[:space:]]*${PAID_PORT}\)" "$SEED"; then
            ok "seed-pb.py seeds the \`${PAID_ROW}\` row on port ${PAID_PORT}"
        else
            bad "seed-pb.py has no \`${PAID_ROW}\` row on port ${PAID_PORT} — a paid code could not resolve"
        fi
        # INVERTED 2026-10. This assertion used to REQUIRE the paid-only gate
        # (`udp_enabled and t == "strike"`), because UDP was the paid plan's
        # differentiator. UDP is now a given on both plans and the differentiator is
        # RATE, so the gate it demanded is precisely the defect. The stronger
        # two-sided form now lives at (h): every row advertises UDP, and each row
        # names its own listener. Kept here as a TOMBSTONE rather than deleted, so a
        # reader who remembers the old gate finds out why it is gone instead of
        # re-adding it.
        if grep -Eq 'uot_enabled and t == "strike"' "$SEED"; then
            bad "seed-pb.py gates UDP to the paid row — UDP is a given on BOTH plans now (see §27(h)); the differentiator is rate"
        else
            ok "seed-pb.py does not gate UDP to the paid row (both plans carry it)"
        fi

        # (e) The retired tier must not be SEEDED. Its row may survive in a live DB
        #     (field codes may carry the string), but a fresh seed must not recreate
        #     it — that is how a retired tier silently becomes sellable again.
        # `"stealth"` — the quote is NOT escaped. The old `\"stealth\"` made GNU grep
        # emit `warning: stray \ before "` on stderr on every run: harmless to the
        # result, and harmful to the habit of reading this output, which is the thing
        # §27's own note says this script exists to protect.
        if grep -Eq '("stealth",[[:space:]]*"STEALTH_PASS"' "$SEED"; then
            bad "seed-pb.py still seeds a \`stealth\` row — the retired tier is sellable again on a fresh deploy"
        else
            ok "seed-pb.py seeds no retired \`stealth\` row"
        fi

        # (f) THE CONSOLE OFFERS NO TIER OF ITS OWN.
        #
        #     INVERTED WITH THE CONSOLE REDESIGN (2026-11). Until then the mint form
        #     carried a hardcoded `ref<string[]>(['free','budget','strike'])` — a
        #     PRE-LOAD placeholder, because the list is hydrated from `tiers.list` —
        #     and this check asserted that literal named the paid row. The redesign
        #     deleted the literal: the form's list now starts EMPTY and is filled
        #     only by the hub, so it can no longer offer a tier the hub does not
        #     hold, on first paint or any other. That makes the old assertion
        #     unsatisfiable AND unnecessary, and re-adding a literal to satisfy it
        #     would restore the defect the check existed to catch.
        #
        #     So the assertion is now the STRONGER, one-sided form: no tier name may
        #     appear as a literal list in the console at all. Verified to FAIL against
        #     the pre-redesign tree (`ref<string[]>(['free','budget','strike'])`
        #     matches it), and against this tree with that line put back.
        if [ -d "$CONSOLE_SRC" ]; then
            tier_literal=$(grep -rnE "ref<string\[\]>\(\[[^]]*'(free|budget|strike|stealth|eco)'" "$CONSOLE_SRC" 2>/dev/null || true)
            if [ -z "$tier_literal" ]; then
                ok "the console names no tier literal — mint options come only from the hub's tiers.list"
            else
                bad "the console hardcodes a tier list again; a literal can offer a tier the hub cannot serve"
                bad "  $tier_literal"
            fi
        else
            warn "no console src/ found — cannot check the tier list"
        fi

        # (h) UDP IS A GIVEN — BOTH PLANS ADVERTISE IT, ON SEPARATE PORTS.
        #
        # WHY THIS EXISTS. UDP stopped being the paid plan's differentiator: the two
        # plans are separated by RATE (100 Mbps vs 1 Mbps), not by whether a UDP path
        # exists. That makes three things load-bearing at once, and each has a silent
        # failure mode:
        #
        #   1. `seed-pb.py` must set udp_relay for EVERY row, not just `strike`. The
        #      old gate (`t == "strike"`) is a one-word edit away from coming back,
        #      and the symptom would be "free users can't play Roblox" — noticed by
        #      students, not by CI.
        #   2. Each row must name its OWN uot_port. A sing-box listener is bound to
        #      one password, so pointing the free row at the paid port sends ECO_PASS
        #      to a listener expecting STRIKE_PASS: the handshake fails and UDP dies
        #      for exactly one plan.
        #   3. The free UDP port must be SHAPED. Without a tc class on it, the free
        #      plan's UDP is bounded only by the client's own cap — the hub trusting
        #      the thing it is supposed to be shaping, and a modified client gets
        #      uncapped UDP. That is the security half of this change, and the reason
        #      the user asked for a server-side cap.
        #
        # Every assertion below was observed FAILING against the pre-change tree.
        UOT_FREE_PORT=8447
        FW="$REPO/server/modules/08-firewall.sh"

        if grep -Eq 'udp = uot_enabled$' "$SEED"; then
            ok "seed-pb.py advertises UDP for every plan (no paid-only gate)"
        else
            bad "seed-pb.py does NOT advertise UDP for every plan — the paid-only gate is back, and free users lose UDP silently"
        fi

        # WAS A PIN ON ONE EXPRESSION, now a pin on the PROPERTY.
        #
        # The old check grepped the literal `uot_port if t == "strike" else
        # uot_port_free`, which is the two-way conditional the free plan's UDP fix
        # introduced. That expression is correct only while there are exactly two
        # plans: a third would fall through to the `else` and be handed the FREE
        # listener, so Budget clients would handshake against a listener holding
        # ECO_PASS — TCP fine, every game silently dead. It was, as written, a check
        # that made the hazard un-fixable: any restructure to support N plans had to
        # break it first.
        #
        # The property that actually matters is negative as well as positive: the
        # two-way conditional must be GONE, and each plan's port must come from its
        # own variable. Both arms were observed failing — the second by reverting
        # `seed-pb.py` to the two-way form, the first by leaving it in place after
        # the mapping was added.
        if grep -Eq 'uot_port if t == "strike"' "$SEED"; then
            bad "seed-pb.py still selects uot_port with a two-way conditional — a third plan would be handed the free plan's listener"
        else
            ok "seed-pb.py no longer selects uot_port with a two-way conditional"
        fi

        _uot_bad=""
        for _pair in "free:uot_port_free" "strike:uot_port" "budget:uot_port_budget"; do
            _plan="${_pair%%:*}"; _var="${_pair#*:}"
            # The mapping literal must name the row and its variable together.
            grep -Eq "\"${_plan}\":\s*${_var}" "$SEED" || _uot_bad="${_uot_bad} ${_plan}"
        done
        if [ -n "$_uot_bad" ]; then
            bad "seed-pb.py's uot_port mapping does not give each plan its own listener:${_uot_bad}"
            bad "  expected a dict entry per plan: \"free\": uot_port_free, \"strike\": uot_port, \"budget\": uot_port_budget"
        else
            ok "seed-pb.py gives each plan its own uot_port (free, strike, budget)"
        fi

        if grep -Eq "apply_tc_now[[:space:]]+${UOT_FREE_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"1mbit\"" "$TC_MOD"; then
            ok "04-tc.sh caps the free UDP port (${UOT_FREE_PORT}) at 1mbit, server-side"
        else
            bad "04-tc.sh does NOT cap the free UDP port (${UOT_FREE_PORT}) — free UDP would be bounded only by the client, which a modified client controls"
        fi

        if grep -Eq "create_tc_service[[:space:]]+\"eco-udp\"[[:space:]]+\"[0-9:]+\"[[:space:]]+\"1mbit\"[[:space:]]+${UOT_FREE_PORT}" "$TC_MOD"; then
            ok "04-tc.sh's tc-eco-udp-cap.service uses the same 1mbit rate"
        else
            bad "04-tc.sh's free-UDP reboot unit disagrees with the applied cap"
        fi

        # The paid UDP port must NOT be shaped: capping a latency-sensitive flow to
        # protect bandwidth is the wrong trade for a plan sold on 100 Mbps.
        if grep -Eq "apply_tc_now[[:space:]]+8446" "$TC_MOD"; then
            bad "04-tc.sh shapes the PAID UDP port (8446) — that can add latency to exactly the traffic the paid plan exists for"
        else
            ok "04-tc.sh does not shape the paid UDP port"
        fi

        # Two-sided: the firewall must admit both listeners, or clients sit on an
        # advertised port that nothing answers.
        # ANCHORED ON THE COMMAND, not on a bare mention of the port variable.
        #
        # The first draft of this check grepped for `${UOT_PORT_FREE:-8447}` and
        # stayed GREEN when both `ufw allow` lines were deleted — because the port
        # also appears in the surrounding `log` lines. That is the same
        # "a check on a value is not a check on what the code does with it" defect
        # §23(f) and the macOS section both record, so it is anchored on `ufw allow`
        # now and was re-observed failing against the deleted-rule tree.
        if [ -f "$FW" ]; then
            if grep -Eq '^[[:space:]]*ufw allow[[:space:]]+"\$\{UOT_PORT_FREE:-8447\}"/(tcp|udp)' "$FW"; then
                ok "08-firewall.sh actually opens the free UDP port (8447) with ufw"
            else
                bad "08-firewall.sh does not open the free UDP port with ufw allow — the hub would advertise it and refuse it"
            fi
        fi

        # (i) THE UNIT NAMES `setup.sh` STARTS MUST BE CREATED BY A MODULE.
        #
        # WHY THIS EXISTS. `setup.sh` keeps its own SERVICES list, and after the
        # per-plan UoT split that list still named the retired `sing-box-uot`. The
        # modules were correct, the listeners worked, and the only symptom was
        # "sing-box-uot failed to start" in the deploy log and a smoke test failing
        # on a service that does not exist. That is noise which teaches an operator
        # to skim deploy output — how a real failure gets missed.
        #
        # HOW IT IS CHECKED. The unit names are TEMPLATED (`tc-${name}-cap.service`,
        # `sing-box-uot-${name}.service`, `shadowsocks-${tier}.service`), so grepping
        # for the final filename cannot work — the first draft of this check was a
        # regex that matched everything and therefore could never fail, which is the
        # defect it exists to catch. Instead each name in SERVICES is decomposed into
        # the TEMPLATE it must come from plus the ARGUMENT that must have been passed,
        # and the argument is looked up in the module that owns that template.
        SETUP="$REPO/server/setup.sh"
        if [ -f "$SETUP" ]; then
            # BOTH assignment forms, because the first draft read only
            # `SERVICES="..."` and silently missed `SERVICES="${SERVICES} ..."` — the
            # line the UoT units are added on. That is why the check passed while the
            # retired `sing-box-uot` was still in the list: it was never extracted, so
            # there was nothing to fail on. A verifier that cannot see the value it
            # checks is the §23(f) defect again.
            _svc_list="$(
                sed -n 's/^[[:space:]]*SERVICES=\(.*\)$/\1/p' "$SETUP" \
                    | sed 's/^"//; s/"$//; s/\${SERVICES}//; s/[{}]//g' \
                    | tr ' ' '\n' | grep -v '^$'
            )"
            _unbacked=""
            for _svc in $_svc_list; do
                case "$_svc" in
                    tc-*-cap)
                        _arg="${_svc#tc-}"; _arg="${_arg%-cap}"
                        grep -qE "create_tc_service[[:space:]]+\"${_arg}\"" "$TC_MOD" \
                            || _unbacked="$_unbacked $_svc" ;;
                    sing-box-uot-*)
                        _arg="${_svc#sing-box-uot-}"
                        grep -qE "write_uot_service[[:space:]]+\"${_arg}\"" \
                            "$REPO/server/modules/02-shadowsocks.sh" \
                            || _unbacked="$_unbacked $_svc" ;;
                    shadowsocks-*)
                        _arg="${_svc#shadowsocks-}"
                        grep -qE "write_service[[:space:]]+\"${_arg}\"" \
                            "$REPO/server/modules/02-shadowsocks.sh" \
                            || _unbacked="$_unbacked $_svc" ;;
                    # ANYTHING ELSE IS A FAILURE, not a skip.
                    #
                    # The first draft used `*) : ;;` here — so a name that did not
                    # match one of the three templates (an old `sing-box-uot`, a
                    # typo, a unit some future module invents) fell through and was
                    # never checked. That is allow-list blindness: the check
                    # confirmed only the names it already understood, which is
                    # precisely the set that cannot be wrong. Observed passing on a
                    # deliberately stale `sing-box-uot`.
                    *)
                        _unbacked="$_unbacked $_svc" ;;
                esac
            done
            if [ -n "$_unbacked" ]; then
                bad "setup.sh starts service(s) no module creates:${_unbacked}"
                bad "  each name must match a write_service / write_uot_service / create_tc_service argument"
            else
                ok "every service setup.sh starts is created by a module"
            fi
        fi

        # (g) THE PROSE ABOUT THE PAID CAP, not just the cap (§23(g)'s lesson one
        #     tier over). This used to REQUIRE the literal `tc caps (1/100 Mbps)` in
        #     `README.md`, which made a hand-maintained second copy of the caps the
        #     condition for a green build — see the longer note at §23(g). It now
        #     asserts the absence instead: README points at the owner, it does not
        #     restate it. VERIFIED TO FAIL by re-adding either the current or a
        #     retired cap string to README.md.
        if [ -f "$REPO/README.md" ]; then
            if grep -Eq 'tc caps|1/100 Mbps|5/100/200 Mbps|Stealth [0-9]+ ?Mbps|5M tc' "$REPO/README.md"; then
                bad "README.md restates a tier cap — the caps have one home (04-tiers.md + state.toml); link to it instead"
            else
                ok "README.md does not restate any tier cap (one home: 04-tiers.md + state.toml)"
            fi
            if grep -Eq 'Stealth [0-9]+ ?Mbps|5M tc' "$REPO/README.md"; then
                bad "README.md still restates a retired tier's cap"
            else
                ok "README.md does not restate a retired tier's cap"
            fi
        fi

        # ── (j) BUDGET: THE THIRD TIER, HELD TO THE SAME STANDARD ──────────────
        #
        # WHY THIS EXISTS. §27(a)-(h) hold the ONE paid tier in agreement because
        # Stealth and Strike drifted into being the same plan at two rates and
        # nothing noticed. The 2026-11-12 change adds a second paid rung
        # (docs/business/04-tiers.md §4.2.5), so the class of defect §27 exists for
        # returns with it — and the specific failure here is quieter than the old
        # one: a Budget that shares Full's endpoint works perfectly for anyone whose
        # traffic never touches it, and is only wrong where it costs money.
        #
        # WHAT "AGREES" MEANS FOR A TIER. Five surfaces describe this endpoint, in
        # four vocabularies, and each pair can disagree silently:
        #
        #   04-tc.sh (cap + class, twice: applied and reboot)
        #     ↕ 02-shadowsocks.sh (the listener's port and credential)
        #     ↕ seed-pb.py       (the row name and the port it advertises)
        #     ↕ 08-firewall.sh   (whether the advertised port is admitted)
        #     ↕ Codes.vue / _theme.tsx / tier-badge.tsx (whether it is sellable and
        #       how it is labelled)
        #
        # EVERY ASSERTION BELOW WAS OBSERVED FAILING FIRST, by mutating the tree to
        # each defect and re-running: the cap, the reboot unit, the seed row, the
        # firewall rule, the console list, the colour and the label.
        BUDGET_PORT=8448
        BUDGET_CAP=25mbit
        BUDGET_ROW=budget
        BUDGET_UOT_PORT=8449
        THEME_TSX="$REPO/client/src/pages/_theme.tsx"
        BADGE_TSX="$REPO/client/src/components/connection/tier-badge.tsx"

        # (j1) The applied cap, anchored on the APPLY LINE so a comment cannot
        #      satisfy it — §27(a)'s lesson, one tier over.
        if grep -Eq "apply_tc_now[[:space:]]+${BUDGET_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${BUDGET_CAP}\"" "$TC_MOD"; then
            ok "04-tc.sh caps Budget's TCP port (${BUDGET_PORT}) at ${BUDGET_CAP}"
        else
            bad "04-tc.sh does NOT cap port ${BUDGET_PORT} at ${BUDGET_CAP} (expected: apply_tc_now ${BUDGET_PORT} \"1:40\" \"${BUDGET_CAP}\")"
        fi

        # (j2) SHAPED UDP. The opposite assertion from the paid port at §27(h), and
        #      the asymmetry is the point: Full is sold on latency so its 8446 is
        #      deliberately unshaped; Budget is sold on a rate, so an unshaped
        #      listener is a route around the whole product. A guard that "tidied"
        #      the two into agreement would remove the plan's only server-side bound.
        if grep -Eq "apply_tc_now[[:space:]]+${BUDGET_UOT_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${BUDGET_CAP}\"" "$TC_MOD"; then
            ok "04-tc.sh caps Budget's UDP port (${BUDGET_UOT_PORT}) at ${BUDGET_CAP}, server-side"
        else
            bad "04-tc.sh does NOT cap Budget's UDP port (${BUDGET_UOT_PORT}) — its UDP would be bounded only by the client, which a modified client controls"
        fi

        # (j3) BOTH reboot units. A class that exists now and disappears at boot is
        #      the §23(b) defect: the running system says one thing, the reboot
        #      another, and nobody finds out until a reboot.
        for _pair in "${BUDGET_ROW}:${BUDGET_PORT}" "${BUDGET_ROW}-udp:${BUDGET_UOT_PORT}"; do
            _unit="${_pair%%:*}"; _port="${_pair#*:}"
            if grep -Eq "create_tc_service[[:space:]]+\"${_unit}\"[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${BUDGET_CAP}\"[[:space:]]+${_port}" "$TC_MOD"; then
                ok "04-tc.sh's tc-${_unit}-cap.service uses the same ${BUDGET_CAP} rate"
            else
                bad "04-tc.sh's tc-${_unit}-cap.service rate disagrees with the applied cap (or is missing)"
            fi
        done

        # (j4) The seed row, with the ROW, its PASSWORD and its PORT asserted
        #      together — a row on the wrong port serves a listener it has no key
        #      for, which is §27(h)'s "one plan told to use the other's listener".
        if grep -Eq "\(\"${BUDGET_ROW}\",[[:space:]]*\"BUDGET_PASS\",[[:space:]]*${BUDGET_PORT}\)" "$SEED"; then
            ok "seed-pb.py seeds the \`${BUDGET_ROW}\` row on port ${BUDGET_PORT} with its own key"
        else
            bad "seed-pb.py has no \`${BUDGET_ROW}\` row on port ${BUDGET_PORT} — a Budget code could not resolve"
        fi

        # (j5) The listener the hub will advertise. Asserted at BOTH ends: the
        #      config is written, and its port is the one the seed will hand out.
        # The `\$` here is a LITERAL dollar (the file contains a shell variable
        # reference, not its expansion), written in single quotes so the shell does
        # not eat it before grep sees it. The first draft double-escaped it and
        # matched nothing — a guard that reported BAD on a correct tree, which is
        # the failure mode that teaches an operator to skim this output.
        if grep -Eq 'write_uot_config[[:space:]]+"budget"[[:space:]]+"\$UOT_PORT_BUDGET"' "$REPO/server/modules/02-shadowsocks.sh" \
           && grep -Eq "UOT_PORT_BUDGET:-${BUDGET_UOT_PORT}" "$REPO/server/modules/02-shadowsocks.sh"; then
            ok "02-shadowsocks.sh creates Budget's UoT listener on :${BUDGET_UOT_PORT}"
        else
            bad "02-shadowsocks.sh does not create a \`budget\` UoT listener on :${BUDGET_UOT_PORT} — the hub would advertise a port nothing answers"
        fi

        # (j6) The firewall admits it. Anchored on `ufw allow`, because a bare mention
        #      of the port in a log line made §27(h)'s first draft pass with the rules
        #      deleted — a check that cannot fail.
        if [ -f "$FW" ]; then
            if grep -Eq '^[[:space:]]*ufw allow[[:space:]]+"\$\{BUDGET_PORT:-8448\}"/tcp' "$FW"; then
                ok "08-firewall.sh opens Budget's TCP port (${BUDGET_PORT})"
            else
                bad "08-firewall.sh does not open Budget's TCP port — the hub would serve a tier nothing can reach"
            fi
            if grep -Eq '^[[:space:]]*ufw allow[[:space:]]+"\$\{UOT_PORT_BUDGET:-8449\}"/udp' "$FW"; then
                ok "08-firewall.sh opens Budget's UDP port (${BUDGET_UOT_PORT})"
            else
                bad "08-firewall.sh does not open Budget's UDP port with ufw allow — the hub would advertise it and refuse it"
            fi
        fi

        # (j7) The THREE CLIENT LISTS, which is the check that would have caught the
        #      original Stealth/Strike divergence years earlier. A tier can be live
        #      on the hub and still be unsellable (no console entry) or unbranded
        #      (no colour). Neither is an error anywhere; both are invisible.
        # The console half. §27(f) asserts the NEGATIVE (no tier literal anywhere);
        # this asserts the POSITIVE that makes the negative safe: the mint form's
        # option list is assigned from the hub's `tiers.list`, so Budget is offered
        # by construction rather than by a name somebody remembered to type here.
        #     The fetch moved in the URL redesign (2026-11): it used to live on
        #     Codes.vue and was passed down as a prop. It now lives in ONE shared
        #     store, views/codes/meta.ts, because the same two lists feed the Codes
        #     filter bar, the inspector's pickers and the mint/renew drawers — and
        #     the old code refetched both on every renewal, two round-trips per
        #     action. The assertion follows the code rather than pinning a filename:
        #     what must stay true is that the list is READ FROM THE HUB (here) and
        #     BOUND to the drawer's options (below), not which file does the read.
        #
        #     `CONSOLE_SRC`/`CONSOLE_CREATE` come from §1c, which runs earlier.
        #     `CONSOLE_TIERS` is derived here, and MUST default safely: the
        #     orchestrator runs under `set -u`, so an unset variable would abort the
        #     ENTIRE run at this line rather than fail one assertion.
        CONSOLE_TIERS="${CONSOLE_SRC:-$REPO/server/console/src}/views/codes/meta.ts"
        CONSOLE_CREATE_F="${CONSOLE_CREATE:-${CONSOLE_SRC:-$REPO/server/console/src}/views/codes/CreateDrawer.vue}"
        if [ -f "$CONSOLE_CREATE_F" ] && [ -f "$CONSOLE_TIERS" ]; then
            if grep -q "call<{ tiers: TierRecord\[\] }>('tiers.list')" "$CONSOLE_TIERS"; then
                ok "the console reads the tier list from the hub's tiers.list"
            else
                bad "the console does not read tiers from tiers.list — the plan would be live on the hub and un-mintable"
            fi
            if grep -qE '<option v-for="t in tiers"' "$CONSOLE_CREATE_F"; then
                ok "the mint form's options are bound to that list"
            else
                bad "the mint form's tier options are not bound to the hub's list — a literal would offer tiers that cannot be served"
            fi
        else
            bad "a console file §27 needs is missing (expected: views/codes/meta.ts and views/codes/CreateDrawer.vue)"
        fi
        if [ -f "$THEME_TSX" ]; then
            if grep -Eq "^[[:space:]]*${BUDGET_ROW}:[[:space:]]*\{[[:space:]]*color:" "$THEME_TSX"; then
                ok "TIER_COLORS has an entry for the Budget wire name"
            else
                bad "TIER_COLORS has no '${BUDGET_ROW}' entry — every Budget student would see the generic fallback badge"
            fi
        fi
        if [ -f "$BADGE_TSX" ]; then
            # The label mapping, not merely the word: `budget` must appear in an arm
            # that produces a label for a student, so a bare mention in the doc
            # comment above cannot satisfy it.
            if grep -Eq "key === '${BUDGET_ROW}'" "$BADGE_TSX"; then
                ok "tier-badge.tsx maps the Budget wire name to a student-visible label"
            else
                bad "tier-badge.tsx has no label arm for '${BUDGET_ROW}' — the badge would show the raw wire name"
            fi
        fi

        # (j8) THE NEGATIVE, and the one that protects the fleet: Budget must not be
        #      on the retired Stealth port, and its credential must not be reused
        #      anywhere. Both are one-line edits away and neither is visible.
        if grep -Eq "\(\"${BUDGET_ROW}\",[[:space:]]*\"BUDGET_PASS\",[[:space:]]*8444\)" "$SEED" \
           || grep -Eq "write_config[[:space:]]+\"${BUDGET_ROW}\"[[:space:]]+8444" "$REPO/server/modules/02-shadowsocks.sh"; then
            bad "Budget is on port 8444 — the retired Stealth slot. Its unit and config survive a redeploy (skip-if-exists), so a Budget listener there would collide with them or inherit their old password"
        else
            ok "Budget is not on the retired Stealth port (8444 stays dark)"
        fi
    fi
}
