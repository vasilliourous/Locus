#!/usr/bin/env bash
# Section 42 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_42() {
    echo
    echo "42. The update offer names a host the CLIENT can actually reach"

    # ─────────────────────────────────────────────────────────────
    # 42. The update offer names a host the CLIENT can actually reach
    #
    # Field-measured 2026-11-06, on a school network that blocks the hub's canonical
    # name by reputation:
    #
    #   * `/api/update` and `/api/release` both returned
    #     `https://…duckdns.org/updates/3.3.5/…` — the LEGACY name — while the client
    #     dialled the canonical one, so the client's own origin check would have
    #     refused the artifact even on an uncensored network.
    #   * and on the filtered network, NEITHER name is reachable, so ANY absolute URL
    #     built from the hub's own hostname is unfetchable there. The escape hatch is
    #     the alias the operator points devices at, which means the hook must answer
    #     with the host the request actually arrived on.
    #
    # This guard pins the three values that have to agree for that to work, because
    # each is a separate file and two of them were wrong at once.
    # ─────────────────────────────────────────────────────────────
    UPDATE_HOOK="$REPO/server/pb_hooks/update.pb.js"
    if [ ! -f "$UPDATE_HOOK" ]; then
        bad "server/pb_hooks/update.pb.js is missing — /api/update cannot answer"
    else
        # (a) The hook must rewrite the offer's origin to the request's host.
        #
        #     EXECUTED, not grepped. The first version of this check searched for
        #     `offerHost` and `url.substring(cutoff)` — tokens that both survive a
        #     `if (false) {` in front of the block, so it reported OK against a hook
        #     that rewrote nothing. A check that cannot fail reads exactly like one
        #     that passed (AGENTS.md rule 3). So the block is lifted out of the file
        #     and run against a stand-in `e` and `$apis`, and the ANSWER is asserted.
        if ! command -v node >/dev/null 2>&1; then
            warn "node is unavailable — cannot evaluate the /api/update origin rewrite"
        else
            rewrite_out=$(node - "$UPDATE_HOOK" <<'NODE' 2>/dev/null
const fs = require("fs");
const src = fs.readFileSync(process.argv[2], "utf8");
const start = src.indexOf('        var offerHost = "";');
const endMark = "url = scheme + offerHost + url.substring(cutoff);\n        }";
const endIdx = src.indexOf(endMark);
if (start === -1 || endIdx === -1) { console.log("MISSING"); process.exit(0); }
const block = src.slice(start, endIdx + endMark.length);
try {
  const run = (url, headers) => {
    const e = {};
    const $apis = { requestInfo: () => ({ headers }) };
    eval(block);
    return url;
  };
  const moved = run("https://networkingguides.duckdns.org/updates/3.3.5/x", { host: "vpn.example.org" });
  const kept  = run("https://cdn.example/other/path", { host: "vpn.example.org" });
  const none  = run("https://networkingguides.jadedns.uk/updates/3.3.5/x", {});
  console.log(moved === "https://vpn.example.org/updates/3.3.5/x" &&
              kept === "https://cdn.example/other/path" &&
              none === "https://networkingguides.jadedns.uk/updates/3.3.5/x" ? "OK" : "WRONG");
} catch (err) { console.log("ERROR"); }
NODE
    )
            case "$rewrite_out" in
                OK)
                    ok "/api/update rewrites the artifact origin to the host the request arrived on (executed)" ;;
                WRONG|MISSING|ERROR|"")
                    bad "/api/update does NOT rewrite the offer's origin to the request host"
                    bad "  (evaluating the block gave '${rewrite_out:-no output}'). On a network"
                    bad "  that blocks the hub's canonical name the device is offered a URL it"
                    bad "  cannot fetch, and the update path is the only route out of that state"
                    ;;
            esac
        fi

        # (b) It must read the Host header through the accessor that actually works.
        #     `e.request().host` is the Go field and is invisible through the goja
        #     bridge; `$apis.requestInfo(e).headers` keys have `-` replaced by `_`.
        #     Asserted here because the wrong spelling fails SILENTLY: the rewrite
        #     simply never fires, and the response still looks well formed.
        if grep -q 'requestInfo(e).headers' "$UPDATE_HOOK" && grep -q 'reqHeaders\["host"\]' "$UPDATE_HOOK"; then
            ok "/api/update reads the Host header through requestInfo(e).headers"
        else
            bad "/api/update reads the request host through something other than"
            bad "  \$apis.requestInfo(e).headers — e.request().host is the Go struct field"
            bad "  and does not reach goja, so the origin rewrite would never fire"
        fi

        # (c) The client's allowlist must accept BOTH the dialled hub and the legacy
        #     name, because the published record points at the legacy one. A single
        #     string here refuses the artifact the hub is offering.
        # Matched on the ALLOWLIST ENTRY, not the constant's name: the first version
        # of this check grepped for `LEGACY_HUB_URL` anywhere in the file and was
        # satisfied by a doc comment that mentions it, so deleting the array entry
        # still reported OK. The pattern below requires the `crate::…::LEGACY_HUB_URL`
        # form that only an expression can produce.
        if grep -qE '^[^/]*crate::chrysalis::contract::LEGACY_HUB_URL,' "$REPO/client/src-tauri/src/chrysalis/update/signal.rs"; then
            ok "the client's artifact allowlist accepts the legacy hub name"
        else
            bad "client update/signal.rs no longer LISTS crate::chrysalis::contract::LEGACY_HUB_URL"
            bad "  in the artifact allowlist — the published download URLs point at that"
            bad "  name, so every update download would be refused"
        fi
    fi

    # ─────────────────────────────────────────────────────────────
    # 43. setup.sh lets an explicit DOMAIN override the secrets bundle
    #
    # Measured 2026-11-07, during the school-network deploy: `DOMAIN=x setup.sh`
    # with a bundle containing `DOMAIN=y` deployed `y`, silently. `set -a` + source
    # turns every bundle line into an assignment, and an assignment overwrites the
    # caller. The committed bundle still names the LEGACY duckdns host while
    # contract::HUB_URL — what every current client dials — is the jadedns one, so
    # the deploy would have built a correct hub at a name nobody dials, with no
    # error anywhere and a green tree.
    #
    # ASSERTED BEHAVIOURALLY, not by grepping for a variable name. A grep would be
    # satisfied by the comment explaining the fix (the §26 lesson). This runs the
    # precedence logic against a synthetic bundle and checks the ANSWER.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "════════════════════════════════════════════════════════════"
}
