#!/usr/bin/env bash
# Section 59 -- the lease is derived from the client's constants, not guessed
#
# WHY THIS EXISTS, AND WHY §58 DID NOT COVER IT
#
# §58 executes the HUB's and the CONSOLE's liveness rules against each other. Both
# of those are downstream of one number, and neither computes it: `LEASE_SECONDS`
# in `session_probe.pb.js` and `LEASE_MS` in `model.ts` are COPIES of a value that
# is really an arithmetic consequence of the CLIENT's heartbeat constants.
#
#   MAX_INTERVAL  (heartbeat.rs)  2 h, the cap on the exponential backoff
#   + 10 % jitter                 +12 min
#   + 15 s timeout
#   + 60 s slack                  = 2 h 13 min 15 s, rounded up to 2 h 16 min
#
# If the client's interval changes and the copies do not, the hub declares a
# healthy but unlucky device ABSENT the moment it backs off to its maximum — and
# it does so silently, because every gate stays green: §58 compares the two
# copies to each other, and they still agree. The failure is only visible against
# the ORIGINAL, which is the client.
#
# That is the same shape as `uot_port`/`server_port_uot` and the other names in
# docs/reference/CONTRACT.md: one rule, several files, nothing holding them. The
# copies cannot be deleted (§58 needs the hook to be runnable on its own), so the
# fix is to make the DERIVATION checkable. `docs/state.toml` records the bound and
# names its source; this section recomputes it from that source and fails on
# disagreement.
#
# WHAT IS CHECKED
#
#   1. state.toml's `[liveness]` lease_s / stale_s are the values the client's
#      constants imply, recomputed here (not grepped from state.toml and echoed).
#   2. Both copies — the hook's LEASE_SECONDS and the console's LEASE_MS — equal
#      state.toml's lease_s. §58 covers hook-vs-console; this covers copy-vs-source.
#   3. The stale bound likewise, against GRACE_PERIOD.
#
# Proving it can fail: change `MAX_INTERVAL` in heartbeat.rs to 1 h and run this;
# it reports the implied lease as 1 h 16 min against state.toml's 2 h 16 min.
section_59() {
    echo "59. The liveness lease is derived from the client's heartbeat constants"

    local report
    report=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys

FAIL = 0
def bad(msg):
    global FAIL
    FAIL = 1
    print("BAD\t" + msg)
def ok(msg):
    print("OK\t" + msg)

def read(path):
    try:
        return open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return None

# ── The source of truth: the client's own constants ──
HEARTBEAT = "client/src-tauri/src/chrysalis/heartbeat.rs"
src = read(HEARTBEAT)
if src is None:
    bad("cannot read %s -- the lease cannot be derived" % HEARTBEAT)
    raise SystemExit(FAIL)

def const(text, name):
    """The integer inside `pub const NAME: Duration = Duration::from_secs(<expr>);`"""
    m = re.search(r'const\s+%s\s*:\s*Duration\s*=\s*Duration::from_secs\(([^)]*)\)'
                  % re.escape(name), text)
    if not m:
        return None
    try:
        return int(eval(m.group(1), {"__builtins__": {}}, {}))
    except Exception:
        return None

max_interval = const(src, "MAX_INTERVAL")
grace = const(src, "GRACE_PERIOD")
jm = re.search(r'const\s+JITTER\s*:\s*f64\s*=\s*([0-9.]+)', src)
jitter = float(jm.group(1)) if jm else None

# The per-attempt timeout, which lives beside the other hub timeouts.
CLIENT = "client/src-tauri/src/chrysalis/hub/client.rs"
csrc = read(CLIENT) or ""
tm = re.search(r'Self::Heartbeat\s*=>\s*([0-9]+)', csrc)
timeout = int(tm.group(1)) if tm else None

if max_interval is None:
    bad("%s no longer declares MAX_INTERVAL as Duration::from_secs(<n>) -- the lease's basis is unreadable" % HEARTBEAT)
if grace is None:
    bad("%s no longer declares GRACE_PERIOD -- the stale bound's basis is unreadable" % HEARTBEAT)
if jitter is None:
    bad("%s no longer declares JITTER -- the lease's basis is unreadable" % HEARTBEAT)
if timeout is None:
    bad("%s no longer gives Heartbeat a timeout -- the lease's basis is unreadable" % CLIENT)
if FAIL:
    raise SystemExit(FAIL)

# ── The derivation, stated as arithmetic rather than as a number ──
#
# SLACK is the one term that is a DECISION, not a measurement: a maintenance-tunnel
# retry plus scheduling. It is here, named, so a reader can see it is the only
# hand-chosen term -- everything else is read out of the client.
SLACK = 60
implied_lease = max_interval * (1 + jitter) + timeout + SLACK
implied_lease = int(-(-implied_lease // 60)) * 60   # round UP to a whole minute
implied_stale = grace

ok("the client's constants imply a lease of %ds (MAX_INTERVAL %ds + %d%% jitter + %ds timeout + %ds slack, rounded up)"
   % (implied_lease, max_interval, int(jitter * 100), timeout, SLACK))

# ── state.toml must record exactly that ──
STATE = "docs/state.toml"
st = read(STATE)
if st is None:
    bad("cannot read %s" % STATE)
    raise SystemExit(FAIL)

def toml_int(text, key):
    m = re.search(r'^%s\s*=\s*([0-9]+)\s*$' % re.escape(key), text, re.M)
    return int(m.group(1)) if m else None

lease_s = toml_int(st, "lease_s")
stale_s = toml_int(st, "stale_s")
if lease_s is None:
    bad("state.toml has no `lease_s` under [liveness] -- the bound has no canonical home")
elif lease_s != implied_lease:
    bad("state.toml lease_s is %d but the client's constants imply %d -- if MAX_INTERVAL changed, "
        "the hub and the console would both start calling a healthy device absent"
        % (lease_s, implied_lease))
else:
    ok("state.toml [liveness] lease_s agrees with the client")

if stale_s is None:
    bad("state.toml has no `stale_s` -- the stale bound has no canonical home")
elif stale_s != implied_stale:
    bad("state.toml stale_s is %d but GRACE_PERIOD is %d -- the two windows would age a device out on different clocks"
        % (stale_s, implied_stale))
else:
    ok("state.toml [liveness] stale_s agrees with GRACE_PERIOD")

# ── Both copies must equal the canonical value ──
#
# §58 already proves hook and console agree with EACH OTHER. This proves they agree
# with the SOURCE, which is the check §58 cannot make: two copies of a stale number
# agree perfectly.
HOOK = "server/pb_hooks/session_probe.pb.js"
hook = read(HOOK)
if hook is None:
    bad("cannot read %s" % HOOK)
else:
    # `var LEASE_SECONDS = 2 * 60 * 60 + 16 * 60;` -- evaluated, not text-matched.
    m = re.search(r'var\s+LEASE_SECONDS\s*=\s*([0-9\s*+]+);', hook)
    if not m:
        bad("%s no longer declares LEASE_SECONDS -- the hub's copy of the lease is gone" % HOOK)
    else:
        hook_lease = eval(m.group(1), {"__builtins__": {}}, {})
        if hook_lease != implied_lease:
            bad("%s LEASE_SECONDS is %ds but the client implies %ds" % (HOOK, hook_lease, implied_lease))
        else:
            ok("the hub's LEASE_SECONDS matches the derived lease")

# ── The FOURTH copy, in the admin console's Overview ──
#
# `attention.list` counts a code as `gone_quiet` past the same lease. It cannot
# import the probe's constant (goja hoists nothing across files), so the number is
# written out a third time in a hook. §58's `liveness-consistency.js` compares all
# three to EACH OTHER; this compares the third to the derivation, which is the
# direction that catches all three drifting together.
ADMIN = "server/pb_hooks/admin_console.pb.js"
admin = read(ADMIN)
if admin is None:
    bad("cannot read %s" % ADMIN)
else:
    m = re.search(r'nowA\s*-\s*seenAMs\)\s*>\s*\(([0-9\s*+]+)\)\s*\*\s*1000', admin)
    if not m:
        bad("%s no longer compares `nowA - seenAMs` against a plain-arithmetic lease "
            "-- the Overview's quiet check is unguarded" % ADMIN)
    else:
        admin_lease = eval(m.group(1), {"__builtins__": {}}, {})
        if admin_lease != implied_lease:
            bad("%s gone_quiet threshold is %ds but the client implies %ds" % (ADMIN, admin_lease, implied_lease))
        else:
            ok("the Overview's gone_quiet threshold matches the derived lease")

MODEL = "server/console/src/views/codes/model.ts"
model = read(MODEL)
if model is None:
    bad("cannot read %s" % MODEL)
else:
    m = re.search(r'const\s+LEASE_MS\s*=\s*\(([^)]*)\)\s*\*\s*1000', model)
    if not m:
        bad("%s no longer declares LEASE_MS as (<expr>) * 1000 -- the console's copy is gone" % MODEL)
    else:
        ms_lease = eval(m.group(1), {"__builtins__": {}}, {})
        if ms_lease != implied_lease:
            bad("%s LEASE_MS is %ds but the client implies %ds" % (MODEL, ms_lease, implied_lease))
        else:
            ok("the console's LEASE_MS matches the derived lease")

    m = re.search(r'const\s+STALE_MS\s*=\s*([0-9]+)\s*\*\s*24\s*\*\s*60\s*\*\s*60\s*\*\s*1000', model)
    if not m:
        bad("%s no longer declares STALE_MS as <days> * 24 * 60 * 60 * 1000" % MODEL)
    else:
        ms_stale = int(m.group(1)) * 86400
        if ms_stale != implied_stale:
            bad("%s STALE_MS is %ds but GRACE_PERIOD is %ds" % (MODEL, ms_stale, implied_stale))
        else:
            ok("the console's STALE_MS matches GRACE_PERIOD")

print("FAIL\t%d" % FAIL)
PY
    )

    if printf '%s' "$report" | grep -q '^FAIL	1'; then
        # The per-assertion lines are printed by the SAME rule as everywhere else:
        # OK lines from a run that also failed are still useful to the reader, and
        # they show which half agreed. Process substitution, not a pipeline — `bad`
        # must reach this shell to set FAIL (see §28's comment).
        while IFS="$(printf '\t')" read -r kind msg; do
            [ -n "$kind" ] || continue
            case "$kind" in
                BAD) bad "$msg" ;;
                OK)  ok "$msg" ;;
            esac
        done < <(printf '%s\n' "$report" | grep -v '^FAIL')
        bad "the liveness lease is not derived from the client -- see the BAD lines above"
    else
        ok "the liveness lease, in state.toml and in all three hooks and the console, is derived from the client's constants"
    fi
}
