#!/usr/bin/env bash
# Section 33 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_33() {
    echo "33. The status wire struct and the TypeScript interface agree"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `ChrysalisStatus` is serialised by Rust with `rename_all = "camelCase"` and was
    # consumed by a hand-written TypeScript interface. Nothing connected them. A
    # field added on one side only is invisible: Rust serialises it and the UI
    # ignores it (a feature that does nothing), or the UI reads a key that is never
    # sent and gets `undefined` (a value rendered as blank or NaN).
    #
    # This is the `uot_port` / `server_port_uot` class one layer out — a name
    # duplicated across languages with nothing enforcing agreement — and it was found
    # for real: the new `udp` field was added to the Rust struct and not to the
    # interface.
    #
    # ── WHAT THIS CHECKS NOW, AND WHY IT CHANGED ──
    #
    # The interface is no longer hand-written. `client/src/services/chrysalis.ts`
    # re-exports the types from `chrysalis.generated.ts`, which
    # `gen-ts-contract.py` derives from the Rust. Section 38 asserts that the
    # generated file matches the Rust, for ALL NINE IPC types rather than this one.
    #
    # So this section no longer compares two handwritten copies — that comparison is
    # now vacuous by construction. It asserts the two things generation does NOT
    # guarantee, which is where a hand-edit or a half-finished migration would show
    # up:
    #
    #   1. `services/chrysalis.ts` imports/re-exports the generated types rather than
    #      redeclaring them. A redeclaration is the old failure mode returning, and
    #      it would shadow the generated type while §38 stayed green.
    #   2. `ChrysalisStatus` is actually among them — the type this section has always
    #      been about must still be a generated export, not quietly dropped from the
    #      re-export list.
    # ─────────────────────────────────────────────────────────────
    WIRE_RUST="$REPO/client/src-tauri/src/cmd/chrysalis.rs"
    WIRE_TS="$REPO/client/src/services/chrysalis.ts"
    WIRE_GEN="$REPO/client/src/services/chrysalis.generated.ts"
    if [ ! -f "$WIRE_RUST" ] || [ ! -f "$WIRE_TS" ] || [ ! -f "$WIRE_GEN" ]; then
        bad "cannot check the status wire shape: one of $WIRE_RUST, $WIRE_TS, $WIRE_GEN is missing"
    else
        wire_report=$(python3 - "$WIRE_RUST" "$WIRE_TS" "$WIRE_GEN" <<'PY'
import re, sys

rust, ts, gen = (open(p, encoding="utf-8").read() for p in sys.argv[1:4])

if not re.search(r'pub struct ChrysalisStatus \{', rust):
    print("BAD\tcould not find `pub struct ChrysalisStatus` in cmd/chrysalis.rs")

# (1) A redeclaration in the hand-written file shadows the generated type and
# reintroduces exactly the drift this section exists to prevent.
for name in ("ChrysalisStatus", "UdpStatus", "AllowanceStatus", "SubscriptionStatus",
             "ValidateCodeResult", "ActivationResult", "ConnectionResult",
             "UpdateStatus", "UpdateProgress"):
    if re.search(r'^export (?:interface|type) %s\b' % name, ts, re.M):
        print("BAD\tservices/chrysalis.ts redeclares %s instead of re-exporting the "
              "generated type — a hand-written mirror shadows the generated one" % name)

# (2) The type this section is about must still be generated and re-exported.
if not re.search(r'^export interface ChrysalisStatus \{', gen, re.M):
    print("BAD\tChrysalisStatus is not in chrysalis.generated.ts — the generator dropped it")
reexport = re.search(r'export type \{(.*?)\} from', ts, re.S)
if not reexport:
    print("BAD\tservices/chrysalis.ts no longer re-exports the generated types")
elif "ChrysalisStatus" not in reexport.group(1):
    print("BAD\tservices/chrysalis.ts does not re-export ChrysalisStatus")
PY
    )
        if [ -z "$wire_report" ]; then
            ok "ChrysalisStatus is generated, not hand-mirrored, and still re-exported"
        else
            while IFS="$(printf '\t')" read -r _kind message; do
                bad "STATUS WIRE: $message"
            done <<EOF
$wire_report
EOF
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
