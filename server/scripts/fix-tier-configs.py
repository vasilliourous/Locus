#!/usr/bin/env python3
"""
fix-tier-configs.py — repair tier_configs on a LIVE VPS.

WHY: activation/heartbeat serve connection parameters from PocketBase's
`tier_configs` collection. If that collection is empty, has stale passwords,
or contains duplicates (older seed scripts always POSTed new records), clients
receive wrong/stale passwords and the Shadowsocks server rejects them with
"An existing connection was forcibly closed by the remote host".

WHAT THIS DOES:
  1. Reads the GROUND-TRUTH passwords from /etc/shadowsocks/{eco,strike}.json
  2. Authenticates against PocketBase using /root/.pb_admin_creds (PB_TOKEN)
  3. Replaces every tier_configs record with a fresh one matching the live
     ssserver configs (server = the configured DOMAIN, ports 8443/8445)
  4. Prints the final state + the collection schema (diagnostics)

RUN (on the VPS, as root):
    python3 fix-tier-configs.py

Requires: curl, root, PocketBase running on 127.0.0.1:8090.
"""
import json
import os
import subprocess
import sys

API = "http://127.0.0.1:8090"
DOMAIN = os.environ.get("DOMAIN", "networkingguides.duckdns.org")
# Cipher fallback, matching the single definition in server/modules/00-env.sh.
# This script READS the cipher out of the live ssserver configs; the default
# only applies when a config omits the field.
SS_METHOD = os.environ.get("SS_METHOD", "2022-blake3-aes-256-gcm")
ADMIN_CREDS = "/root/.pb_admin_creds"
SS_CONFIG_DIR = "/etc/shadowsocks"

# udp_relay MUST stay false: it historically enabled the client's sing-box
# "udp_over_tcp v2" option, but that protocol is proprietary to sing-box —
# shadowsocks-rust closes those connections (RST, observed 2026-08-01).
# UDP flows via standard ss UDP (server mode tcp_and_udp).
#
# (tier, port, udp, source_tier)
# `source_tier` is the on-box shadowsocks config this row's credentials come
# from. It exists for `free`, which shares the Eco endpoint (port 8443, same
# password, same 1mbit tc class) and has no /etc/shadowsocks/free.json of its
# own. `free` is what codes are minted against and what the console lists.
#
# There is NO `eco` row any more (2026-10-06). It was a duplicate of `free` on
# the same port, kept only in case a code in the field carried the string; the
# live hub holds none, so it appeared in the console as a second plan sharing
# one port. `ECO_PASS` and the on-box `eco` unit names stay — those are the free
# plan's credentials and infrastructure, not a tier.
TIERS = [
    ("free", 8443, False, "eco"),
    ("strike", 8445, False, "strike"),
]

# TIER ROWS THIS SCRIPT MUST NOT DELETE.
#
# `stealth` was retired when the paid ladder merged into one plan
# (docs/business/04-tiers.md §4.2.4), so it is no longer repaired here — but it
# is also not deleted, because a code in the field may still carry that tier
# string and resolve against this row. Removing it would strand real students.
#
# This matters because the repair below REPLACES every unsanctioned row it finds.
# Dropping `stealth` from TIERS alone would silently delete the row on the next
# repair run; the preset disposition is therefore explicit, so an operator can
# see what happens to it rather than discovering it.
#
# If the live probe (docs/operate/CLAIMS.md §5) shows no code carries the string,
# remove this entry and delete the row from the console.
PRESERVE_ROWS = ["stealth"]


def log(msg):
    print(msg)


def api(method, path, data=None, token=None):
    cmd = ["curl", "-s", "-X", method, f"{API}{path}", "-H", "Content-Type: application/json"]
    if token:
        cmd += ["-H", f"Authorization: {token}"]
    if data is not None:
        tf = f"/tmp/fixcfg_{os.getpid()}.json"
        with open(tf, "w") as f:
            json.dump(data, f)
        cmd += ["-d", f"@{tf}"]
    r = subprocess.run(cmd, capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"error": r.stdout[:300]}


def main():
    log("== Locus tier_configs repair ==")

    # ── 0. Sanity checks ──
    if not os.path.isdir(SS_CONFIG_DIR):
        log(f"ERROR: {SS_CONFIG_DIR} not found — is this the VPN VPS?")
        sys.exit(1)

    # ── 1. Ground-truth passwords from live ssserver configs ──
    live = {}
    for tier, port, _udp, src in TIERS:
        path = os.path.join(SS_CONFIG_DIR, f"{src}.json")
        try:
            with open(path) as f:
                cfg = json.load(f)
            live[tier] = {
                "password": cfg.get("password", ""),
                "port": cfg.get("server_port", port),
                "method": cfg.get("method", SS_METHOD),
            }
            note = "" if src == tier else f" (shared with {src})"
            log(f"  {tier}: ssserver on :{live[tier]['port']} (method {live[tier]['method']}){note}")
        except Exception as ex:
            log(f"ERROR: cannot read {path}: {ex}")
            sys.exit(1)

    # ── 2. Admin auth ──
    token = ""
    if os.path.exists(ADMIN_CREDS):
        with open(ADMIN_CREDS) as f:
            for line in f:
                if "=" in line and not line.startswith("#"):
                    k, v = line.strip().split("=", 1)
                    if k == "PB_TOKEN":
                        token = v
    if not token:
        log(f"ERROR: no PB_TOKEN in {ADMIN_CREDS} — cannot authenticate.")
        sys.exit(1)
    test = api("GET", "/api/collections", token=token)
    if test.get("items") is None:
        log("ERROR: admin token rejected. Regenerate it:")
        log("  # on the VPS, as the pocketbase user or via the admin UI:")
        log("  curl -X POST http://127.0.0.1:8090/api/admins/auth-with-password \\")
        log("    -H 'Content-Type: application/json' \\")
        log("    -d '{\"identity\":\"admin@YOURDOMAIN\",\"password\":\"ADMIN_PASSWORD\"}'")
        sys.exit(1)
    log("  ✓ admin token valid")

    # ── 3. Collection schema (diagnostics — a mismatched schema explains
    #       'Failed to create record' 400s from the public API) ──
    schema = api("GET", "/api/collections/tier_configs", token=token)
    if schema.get("fields"):
        log("  tier_configs schema:")
        for f in schema["fields"]:
            log(f"    - {f.get('name')} ({f.get('type')}) required={f.get('required')}")
    else:
        log("  WARNING: cannot read tier_configs schema — it may not exist!")
        log("  Create it by re-running the seed script, or in the admin UI.")

    # ── 4. Replace all tier_configs records with the live passwords ──
    #
    # The delete-and-recreate is what makes this script trustworthy — it
    # reconciles stale passwords against the live ssserver configs. It is also
    # what makes it dangerous: a row omitted from TIERS is a row this script
    # DELETES. `stealth` is exactly that case (retired from the code, but a live
    # code may still carry the string), which is why PRESERVE_ROWS exists and is
    # honoured here rather than being a comment somebody has to remember.
    existing = api("GET", "/api/collections/tier_configs/records?perPage=200", token=token)
    items = existing.get("items", []) if isinstance(existing, dict) else []
    log(f"  existing records: {len(items)}")

    preserved = []
    for rec in items:
        if rec.get("tier") in PRESERVE_ROWS:
            preserved.append(rec)
            log(f"    preserving {rec.get('tier')} (retired, but a live code may still carry it)")

    for rec in items:
        resp = api("DELETE", f"/api/collections/tier_configs/records/{rec['id']}", token=token)
        log(f"    deleted {rec.get('tier')} ({rec['id'][:12]}) -> {resp.get('code', 'ok')}")

    # Re-post the preserved rows verbatim: their config is NOT reconciled against
    # /etc/shadowsocks, because their service is gone. Their endpoint is whatever
    # it was when they were retired, which is what a stranded code needs.
    for rec in preserved:
        body = {
            "tier": rec.get("tier"),
            "config": rec.get("config"),
            "active": rec.get("active", True),
            "udp_relay": rec.get("udp_relay", False),
        }
        resp = api("POST", "/api/collections/tier_configs/records", body, token=token)
        if resp.get("id"):
            log(f"  ✓ restored preserved row {rec.get('tier')}")
        else:
            log(f"  ✗ FAILED to restore preserved row {rec.get('tier')}: {resp}")

    for tier, port, udp, _src in TIERS:
        body = {
            "tier": tier,
            "config": json.dumps({
                "server": DOMAIN,
                "server_port": live[tier]["port"],
                "password": live[tier]["password"],
                "method": live[tier]["method"],
            }),
            "active": True,
            "udp_relay": udp,
        }
        resp = api("POST", "/api/collections/tier_configs/records", body, token=token)
        if resp.get("id"):
            log(f"  ✓ seeded {tier} -> {DOMAIN}:{live[tier]['port']}")
        else:
            log(f"  ✗ FAILED to seed {tier}: {resp}")

    # ── 5. Verify ──
    final = api("GET", "/api/collections/tier_configs/records?perPage=200", token=token)
    items = final.get("items", []) if isinstance(final, dict) else []
    log(f"== final tier_configs: {len(items)} record(s) ==")
    for rec in items:
        try:
            cfg = json.loads(rec.get("config", "{}"))
        except Exception:
            cfg = rec.get("config", {})
        log(f"  {rec.get('tier')}: {cfg.get('server')}:{cfg.get('server_port')} pass={str(cfg.get('password'))[:6]}... udp={rec.get('udp_relay')}")

    log("Done. Clients will refresh automatically via heartbeat within 5 minutes,")
    log("or immediately on next activation.")


if __name__ == "__main__":
    main()
