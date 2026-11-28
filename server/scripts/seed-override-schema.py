#!/usr/bin/env python3
"""Additive, schema-only migration for the code-override feature.

WHY THIS EXISTS INSTEAD OF `seed-pb.py`.

`seed-pb.py` is the schema authority and it *does* reconcile these columns. But it
also PATCHes `update_config` to `{version: "1.0.0", active: true, rollout: 0}`.
On a live hub that holds `{version: 3.5.2, active: 0}` that is not a no-op — it
withdraws the current release and offers a *downgrade* to every client that beats.
The repository's own deploy note says a code deploy must never "withdraw a release
or reset a rollout", so `seed-pb.py` is the wrong tool for a routine code push.

This script does ONE thing: it adds the columns and the collection the override
feature needs, and writes no records. It is idempotent — running it twice changes
nothing the second time.

It is deliberately narrow: it does not seed tiers, update_config, codes, or any
other collection. If those need reconciling, run `seed-pb.py` from `setup.sh`,
which is where the release/rollout reset is already a known and accepted effect of
a full rebuild.

Usage (ON THE HUB, where PocketBase listens on 127.0.0.1):

    DOMAIN=<hub domain> python3 seed-override-schema.py          # apply
    DOMAIN=<hub domain> python3 seed-override-schema.py --check  # report only
"""
import json
import os
import sys

API = "http://127.0.0.1:8090"
DOMAIN = os.environ.get("DOMAIN", "")
ADMIN_CREDS = "/root/.pb_admin_creds"

if not DOMAIN:
    sys.exit("DOMAIN environment variable required (must be the hub's own domain)")

CHECK_ONLY = "--check" in sys.argv[1:]

TOKEN = ""


def api(method, path, data=None):
    import subprocess

    cmd = ["curl", "-s", "-X", method, f"{API}{path}",
           "-H", "Content-Type: application/json"]
    if TOKEN:
        cmd += ["-H", f"Authorization: {TOKEN}"]
    if data is not None:
        tf = f"/tmp/ovmig_{os.getpid()}.json"
        with open(tf, "w") as f:
            json.dump(data, f)
        cmd += ["-d", f"@{tf}"]
    r = subprocess.run(cmd, capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"error": r.stdout[:300]}


def login():
    """Authenticate as the PocketBase admin, trying both 0.22 and 0.23+ paths."""
    global TOKEN
    # The admin's email is NOT necessarily `admin@<DOMAIN>`: on the live hub the
    # account is on a different domain from the canonical hub name, and assuming
    # otherwise is what made the first attempt fail to authenticate. Prefer the
    # recorded email, fall back to the DOMAIN-derived one only if absent.
    email = os.environ.get("PB_ADMIN_EMAIL", "")
    password = os.environ.get("PB_ADMIN_PASS", "")
    try:
        with open(ADMIN_CREDS) as f:
            for line in f:
                if line.startswith("PB_ADMIN_EMAIL=") and not email:
                    email = line.split("=", 1)[1].strip()
                if line.startswith("PB_ADMIN_PASS=") and not password:
                    password = line.split("=", 1)[1].strip()
    except OSError:
        pass
    if not email:
        email = f"admin@{DOMAIN}"
    if not password:
        sys.exit(f"no admin password: set PB_ADMIN_PASS or provide {ADMIN_CREDS}")
    for path in ("/api/admins/auth-with-password", "/api/collections/_superusers/auth-with-password"):
        resp = api("POST", path, {"identity": email, "password": password})
        tok = resp.get("token") or resp.get("adminToken") or ""
        if tok:
            TOKEN = tok
            return True
    return False


# The exact additions this feature needs. Kept beside the schema in seed-pb.py;
# check-consistency.sh §3 fails if a hook writes a field that is not here AND not
# there, so the two cannot silently disagree.
CODES_COLUMNS = [
    # ── The override feature (2026-11-28) ──
    {"name": "force_state", "type": "text"},
    {"name": "evict_requested_at", "type": "date"},
    {"name": "evict_reason", "type": "text"},
    {"name": "override_id", "type": "text"},
    # ── Still-missing columns the hooks have written since before that ──
    #
    # These four are the "THIRD REPORT OF THE SAME DEFECT" set from
    # docs/reference/FIXES.md. The hooks have written them for months; the live
    # hub has never had them, so every write was silently discarded. Adding them
    # here is the same one-line act as adding the override columns, and it fixes
    # the drift that made the whole per-code liveness axis read `unknown`.
    #
    # `last_seen_at` is the load-bearing one: without it nothing knows whether a
    # device is live, so `observed_state` is always `unknown` and an eviction can
    # never be justified. The other three are operator-facing (suspended reason,
    # "issued before" marker, probe's truncated fingerprint).
    #
    # NULLABLE, no backfill: null means "no observation yet", which the console
    # renders as "—" and never as a definite state.
    {"name": "last_seen_at", "type": "date"},
    {"name": "first_seen_at", "type": "date"},
    {"name": "first_seen_fingerprint", "type": "text"},
    {"name": "suspended_reason", "type": "text"},
]

CODE_OVERRIDES = [
    {"name": "code", "type": "text", "required": True},
    {"name": "override_id", "type": "text", "required": True},
    {"name": "aspect", "type": "text", "required": True},
    {"name": "from_value", "type": "text"},
    {"name": "to_value", "type": "text"},
    {"name": "reason", "type": "text", "required": True},
    {"name": "actor", "type": "text", "required": True},
    {"name": "observed_state", "type": "text"},
    {"name": "observed_at_override", "type": "date"},
    {"name": "outcome", "type": "text", "required": True},
]

# Append-only: an audit log must not be editable or deletable, and a null rule
# denies everyone including the admin. This is the guarantee, not a convention.
OVERRIDE_RULES = {
    "listRule": "@request.auth.admin = true",
    "viewRule": "@request.auth.admin = true",
    "createRule": "@request.auth.admin = true",
    "updateRule": None,
    "deleteRule": None,
}


def main():
    if not login():
        sys.exit("could not authenticate as the PocketBase admin")

    failures = 0

    # ── 1. The four columns on `codes` ──
    # Read the schema IMMEDIATELY before patching. An earlier version of this
    # script computed `missing` from one read and PATCHed a merged list later;
    # when the columns had already been added by someone else the PATCH carried a
    # duplicate and PocketBase answered only "Failed to update the collection."
    # The real reason (`validation_duplicated_field_name`) is in the per-field
    # `data` map, which the generic message hides — so failures here print it.
    # PocketBase can serve a slightly stale collection metadata immediately after
    # a schema PATCH, so retry the read before concluding anything is missing.
    # This is not defensive noise: it is what made a previous run of this script
    # append four columns that were ALREADY there and fail with
    # `validation_duplicated_field_name` — a self-inflicted error that looked like
    # a PocketBase fault.
    import time as _time

    cur = {}
    schema = []
    for attempt in range(4):
        cur = api("GET", "/api/collections/codes")
        if not cur.get("id"):
            sys.exit(f"the `codes` collection does not exist or is unreadable: {cur}")
        schema = cur.get("schema") or []
        have_now = {f.get("name") for f in schema}
        if all(c["name"] in have_now for c in CODES_COLUMNS) or attempt == 3:
            break
        _time.sleep(0.4)
    have = {f.get("name") for f in schema}
    missing = [f for f in CODES_COLUMNS if f["name"] not in have]
    if not missing:
        print("  codes: every column this script manages is already present")
    elif CHECK_ONLY:
        print(f"  codes: WOULD ADD {', '.join(f['name'] for f in missing)}")
    else:
        merged = schema + missing
        resp = api("PATCH", "/api/collections/codes", {"schema": merged})
        if resp.get("id"):
            print(f"  codes: + added {', '.join(f['name'] for f in missing)}")
        else:
            detail = resp.get("data") or {}
            hint = ""
            for k, v in detail.items():
                if isinstance(v, dict):
                    for fld, info in v.items():
                        if isinstance(info, dict) and info.get("message"):
                            hint = f"{fld}: {info['message']}"
            print(f"  codes: FAILED to add {', '.join(f['name'] for f in missing)}: "
                  f"{resp.get('message', resp)}{(' — ' + hint) if hint else ''}")
            failures += 1

    # ── 2. The `code_overrides` collection ──
    allc = api("GET", "/api/collections?perPage=200")
    exists = any(c.get("name") == "code_overrides" for c in allc.get("items", []))
    if not exists:
        if CHECK_ONLY:
            print("  code_overrides: WOULD CREATE")
        else:
            body = {"name": "code_overrides", "type": "base", "schema": CODE_OVERRIDES}
            body.update(OVERRIDE_RULES)
            resp = api("POST", "/api/collections", body)
            if resp.get("id"):
                print("  code_overrides: created (append-only)")
            else:
                print(f"  code_overrides: FAILED to create: {resp.get('message', resp)}")
                failures += 1
    else:
        # Reconcile columns AND rules. An earlier build could have created it with
        # permissive rules, in which case "append-only" would be a comment rather
        # than a rule.
        cur_ov = api("GET", "/api/collections/code_overrides")
        cv = cur_ov.get("schema") or []
        chave = {f.get("name") for f in cv}
        cmiss = [f for f in CODE_OVERRIDES if f["name"] not in chave]
        drift = {k: v for k, v in OVERRIDE_RULES.items() if cur_ov.get(k) != v}
        if not cmiss and not drift:
            print("  code_overrides: already present and append-only")
        elif CHECK_ONLY:
            if cmiss:
                print(f"  code_overrides: WOULD ADD {', '.join(f['name'] for f in cmiss)}")
            if drift:
                print(f"  code_overrides: WOULD RECONCILE rules {', '.join(drift)}")
        else:
            patch = {}
            if cmiss:
                patch["schema"] = cv + cmiss
            patch.update(drift)
            resp = api("PATCH", "/api/collections/code_overrides", patch)
            if resp.get("id"):
                bits = []
                if cmiss:
                    bits.append(f"added {', '.join(f['name'] for f in cmiss)}")
                if drift:
                    bits.append(f"reconciled {', '.join(drift)}")
                print(f"  code_overrides: {'; '.join(bits)}")
            else:
                print(f"  code_overrides: FAILED to reconcile: {resp.get('message', resp)}")
                failures += 1

    print()
    if failures:
        print(f"SCHEMA MIGRATION FAILED — {failures} problem(s). Do NOT deploy hooks.")
        sys.exit(1)
    print("OK — override schema is in place" + (" (check only, nothing written)" if CHECK_ONLY else ""))


if __name__ == "__main__":
    main()
