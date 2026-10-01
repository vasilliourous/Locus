// Locus Heartbeat Hook — PocketBase 0.22 compatible
// Changed from GET to POST to avoid leaking activation codes in server access logs.
// Code and fingerprint are sent in the JSON body, not URL query parameters.
routerAdd("POST", "/api/heartbeat", function(e) {
    // ── PocketBase date parsing ──────────────────────────────────────────────
    // CRITICAL: `new Date("2027-09-19 00:00:00.000Z")` returns NaN in goja.
    // The ECMAScript date-string format requires a "T" separator; PocketBase
    // stores a space. Copied verbatim from activation.pb.js (and defined inside
    // the callback because goja does not hoist function declarations across
    // scopes — a file-level helper would be invisible here).
    //
    // The heartbeat learned this the hard way: activation had the guard, and
    // heartbeat did not, so a device that was already bound kept beating
    // successfully forever after its code expired. Expiry only bit a NEW
    // activation, never a running client — which is precisely the machine it
    // most needed to bite. Fixed 2026-09-28.
    function parsePBDate(value) {
        if (value === null || value === undefined || value === "") return 0;
        if (typeof value === "number") return value;
        var s = String(value).trim();
        if (!s) return 0;
        var direct = new Date(s).getTime();
        if (!isNaN(direct)) return direct;
        var swapped = new Date(s.replace(" ", "T")).getTime();
        if (!isNaN(swapped)) return swapped;
        if (/^[0-9]+$/.test(s)) {
            var n = parseInt(s, 10);
            return s.length <= 10 ? n * 1000 : n;
        }
        return NaN;
    }

    try {
        var data = $apis.requestInfo(e).data;
        var code = (data.code || "").trim();
        var fingerprint = (data.fingerprint || "").trim();
        // A device that was restored by recognition has no code to send — the
        // hub deliberately never re-sends the code, because it is a bearer
        // credential for the ENTITLEMENT. Instead the device presents the
        // session token recognition minted for it.
        var token = (data.token || "").trim();

        if (!code && !token) return e.json(400, {code:400, message:"Missing code"});

        // ── Resolve a token to its code ──
        //
        // Done FIRST, so every check below (expiry, suspension, update signal)
        // runs on exactly the same `code` value regardless of how the device
        // authenticated. That is the whole point of the token: it is an
        // alternative way to NAME the entitlement, never a weaker one.
        //
        // Failure here falls through to the code path rather than refusing — a
        // client that sent both is a client mid-upgrade, and the code is the
        // stronger credential.
        if (!code && token) {
            var ident = null;
            try {
                ident = $app.dao().findFirstRecordByFilter("device_identities",
                    "token_hash = '" + $security.sha256(token) + "'");
            } catch (identErr) {
                ident = null;
            }
            // No row, revoked, or expired: the token is not a valid credential
            // and the answer is the same as any other unknown device. 401 with
            // the existing "unknown" vocabulary, NOT a new status string — the
            // client treats an unrecognised status as a transport failure and
            // would keep retrying a token that will never work.
            if (!ident || ident.getString("revoked_at")) {
                return e.json(401, {code:401, message:"Device token is not valid"});
            }
            var tokenExpMs = parsePBDate(ident.get("token_expires_at"));
            if (isNaN(tokenExpMs) || tokenExpMs <= 0 || tokenExpMs < Date.now()) {
                return e.json(401, {code:401, message:"Device token has expired"});
            }
            // The identity names the code, and every rule below then applies to
            // it unchanged — a suspended device stays suspended, an expired code
            // stays expired, whichever credential the client used.
            code = ident.getString("code");
            if (!code) return e.json(401, {code:401, message:"Device token is not valid"});
        }

        // Normalize the lookup code to the canonical hyphenated form
        // ("RQ-XXXX-XXXX-XXXX-C" — the form codes are seeded in). Clients
        // should send the canonical form, but tolerating any pasted variant
        // keeps heartbeat (suspension checks, update signals) alive for
        // legacy installs that stored an unformatted code.
        var s = code.replace(/-/g,"").toUpperCase();
        var canonical = (s.length === 15)
            ? s.substring(0,2)+"-"+s.substring(2,6)+"-"+s.substring(6,10)+"-"+s.substring(10,14)+"-"+s.substring(14,15)
            : code;

        // Use findFirstRecordByData (same approach as activation hook — confirmed working on PB 0.22.21)
        // NOTE: it THROWS "sql: no rows in result set" for an unknown code rather
        // than returning null, so the `if (!record)` check below is only reached
        // once the throw is caught. Without the try/catch an unknown code
        // surfaced as HTTP 500 instead of a clean 404 (found live 2026-09-19).
        var record = null;
        try {
            record = $app.dao().findFirstRecordByData("codes", "code", canonical);
        } catch (notFound) {
            record = null;
        }
        if (!record) return e.json(404, {code:404, message:"Code not found"});

        // ── Subscription expiry, ENFORCED ────────────────────────────────────
        //
        // This is the check that makes a lapsed code actually stop. It runs
        // BEFORE the suspension check and before anything is returned, on every
        // beat, so a device that was already connected finds out at its next
        // check-in and the client can take the tunnel down.
        //
        // 410, not 403: the client distinguishes a definite refusal (which it
        // must act on) from a transport blip by status code, and 410 is what
        // activation already uses for "Code expired". Using 403 here would tell
        // the client "suspended", which is a different account state.
        //
        // The guard is `!isNaN(expMs) && expMs > 0 && expMs < Date.now()`:
        //   - a code with NO expiry parses to 0 and never expires (a permanent
        //     code is a legitimate product, not an error);
        //   - an UNPARSEABLE value yields NaN and is treated as "no expiry"
        //     rather than "expired" — a hub that cannot read its own date must
        //     not lock every student out. This is the one place the safe default
        //     is "valid", because the alternative default silently ends
        //     subscriptions, and activation already made the same choice.
        var expMs = parsePBDate(record.get("expires_at"));
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(410, {code:410, message:"Code expired"});
        }

        if (record.getBool("suspended")) return e.json(403, {code:403, message:"Account suspended — contact your middleman"});

        var response = {status:"ok", server_time:new Date().toISOString()};

        // ── Subscription expiry ──────────────────────────────────────────────
        //
        // Sent so the client can show the student when their code lapses. The
        // value already existed on the record and was already returned by
        // /api/code-lookup; the heartbeat simply never carried it, which meant a
        // client could learn the expiry at activation and then had no way to
        // refresh it. A subscription renews while the app is installed, so that
        // staleness was the normal case rather than an edge case.
        //
        // Emitted as the raw stored value, NOT reformatted. The client parses it
        // with the same tolerant parser used at activation, and reformatting
        // here would create a second dialect that has to stay in sync. An
        // absent/empty value is emitted as null rather than "" so the client can
        // distinguish "no expiry recorded" from "expiry is the empty string" —
        // and so a code without an expiry does not render as an expired one.
        //
        // ADDITIVE ONLY. Older clients ignore unknown keys, so this cannot break
        // a build already in the field, and no client requires it to connect.
        var expiryRaw = record.get("expires_at");
        var expiryStr = (expiryRaw === null || expiryRaw === undefined)
            ? ""
            : String(expiryRaw).trim();
        response.expires_at = expiryStr ? expiryStr : null;

        // Update signal — read from update_config.
        //
        // There is NO rollout percentage. An active release is advertised to
        // every client that heartbeats; `active` is the single off switch.
        //
        // The percentage was removed deliberately (2026-09): it existed to
        // limit how many devices saw a bad build, because the client has no
        // rollback beyond the installer's own atomicity. The cost was a fleet
        // where a low percentage means the operator's own test device is
        // probably outside the bucket — so a working updater looks broken, and
        // the usual outcome was a misdiagnosis rather than safety. With it gone,
        // "publish" and "offer" are the same act, and `active` stops offering.
        //
        // IMPORTANT: use findFirstRecordByFilter, NOT findRecordsByFilter.
        // In this PocketBase build (0.22.21) findRecordsByFilter silently
        // returns ZERO rows for every collection and filter, even `1=1` on a
        // populated table, with no error raised. That made the update gate a
        // permanent no-op: no client was ever offered an update. Both
        // findFirstRecordByFilter and findRecordsByExpr work reliably (verified
        // live 2026-09-19), including on bool fields.
        try {
            var u = null;
            try {
                u = $app.dao().findFirstRecordByFilter("update_config", "active = true");
            } catch (noRow) {
                u = null; // no active row — nothing to advertise
            }
            if (u && u.get("version")) {
                response.update_available = u.get("version");

                // Generic fallback fields. update_url/update_sha256 are the
                // legacy single-platform pair and describe the LINUX binary
                // only, so they are kept for older clients and must not be
                // treated as describing the device's own platform.
                if (u.get("update_url")) response.update_url = u.get("update_url");
                if (u.get("update_sha256")) response.update_sha256 = u.get("update_sha256");

                // Per-platform URL and checksum. The client verifies the bytes
                // it actually downloads, so a single update_sha256 is not enough
                // — it can only ever match one platform, and the updater refuses
                // to apply an update with an empty hash.
                //
                // The field NAMES are renamed on the way out: the record stores
                // download_<platform>, the response carries update_<platform>.
                // That rename is historical and load-bearing — deployed clients
                // read update_linux, so the hook must keep translating. Do not
                // "simplify" it to emit download_* directly.
                var platKeysH = ["linux", "windows", "macos_intel", "macos_arm"];
                for (var hi = 0; hi < platKeysH.length; hi++) {
                    var hk = platKeysH[hi];
                    if (u.get("download_" + hk)) response["update_" + hk] = u.get("download_" + hk);
                    if (u.get("sha256_" + hk)) response["update_sha256_" + hk] = u.get("sha256_" + hk);
                    // The minisign signature. The Tauri updater verifies one
                    // MANDATORILY, so a release advertised without this cannot be
                    // installed — the client would download and then fail. This
                    // hook is the path that carries it; /api/update serves the
                    // same data to the plugin directly.
                    if (u.get("signature_" + hk)) response["update_signature_" + hk] = u.get("signature_" + hk);
                }
            }
        } catch(ex) { /* update_config missing or unreadable — skip updates */ }

        // Server config for this tier.
        // Same findRecordsByFilter caveat as above — findFirstRecordByFilter is
        // the working call. The previous version relied on a lookup that could
        // return nothing, which would omit server_config from the heartbeat and
        // leave clients unable to refresh their tunnel settings.
        var tier = record.getString("tier");
        var tierVal = tier.replace(/[^a-zA-Z0-9_]/g, "_");
        var cfgRec = null;
        try {
            cfgRec = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal + "'");
        } catch (noCfg) {
            cfgRec = null;
        }
        if (cfgRec) {
            // ─── FROZEN WIRE CONTRACT — DO NOT RENAME ───────────────────────
            // The tier's config JSON is passed through VERBATIM, so the UoT
            // endpoint reaches the client under its stored key, "uot_port".
            //
            // That key is load-bearing and cannot be changed: clients already
            // in the field read "uot_port", and an unknown-to-them key is a
            // SILENT no-op rather than an error — the exact failure that made
            // UDP-over-TCP dead fleet-wide until 2026-09-19 (the client struct
            // declared only "server_port_uot"). The client now tolerates both
            // spellings via internal/uotkey, but the hub must keep emitting
            // "uot_port" because older builds cannot be updated retroactively.
            //
            // See FIXES.md entry 29 and internal/uotkey for the full history.
            // ────────────────────────────────────────────────────────────────
            try { response.server_config = JSON.parse(cfgRec.get("config")); } catch(ex) { response.server_config = cfgRec.get("config"); }
            response.udp_relay = cfgRec.get("udp_relay");
        }
        response.tier = tier;

        // ── Which version of this hook is actually running ───────────────────
        //
        // ADDITIVE, and deliberately so: an old client ignores an unknown key, so
        // this breaks nothing, and no client requires it.
        //
        // It exists because the fix above (the 410 on an expired code) reaches the
        // live hub only when `setup.sh` re-runs, which deploys from `/root/server/`
        // rather than from the repo. That made "is the expiry fix live?" a question
        // nobody could answer without shell access to the box — and a hook that
        // silently did nothing looked exactly like one that was never deployed.
        //
        // With this, the question is answered by any heartbeat: bump the number
        // when the enforcement logic changes, and a response that lacks the key is
        // proof of a stale deployment.
        //
        //   2 = expiry enforced (410) + suspension (403) + expires_at in the response
        response.enforcement_version = 2;

        return e.json(200, response);
    } catch(err) {
        return e.json(500, {code:500, message:err.message || String(err)});
    }
});
