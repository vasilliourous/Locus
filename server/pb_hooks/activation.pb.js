// Locus Activation Hook — PocketBase 0.22 compatible
// Uses findFirstRecordByData for lookups (simplest API, works across versions)
// Uses newQuery().execute() for SQL operations
// All code inside routerAdd callback (functions not hoisted in goja scope)

routerAdd("POST", "/api/activate", function(e) {
    // ── PocketBase date parsing ──────────────────────────────────────────────
    // CRITICAL: `new Date("2027-09-19 00:00:00.000Z")` returns NaN in goja.
    // The ECMAScript date-string format requires a "T" separator; PocketBase
    // stores a space. Measured against the live hub on 2026-09-19.
    //
    // Every expiry check used to read:
    //     var ed = new Date(exp).getTime(); if (!isNaN(ed) && ed < Date.now()) ...
    // and because the parse ALWAYS returned NaN, the isNaN guard was always
    // taken and the comparison NEVER RAN — so no code ever expired. The guard
    // converted a parse failure into "still valid", the most dangerous default
    // for an expiry check.
    //
    // Defined inside the callback on purpose: goja does not hoist function
    // declarations across scopes, so a file-level helper is invisible here.
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

    // The code's expiry, as the client should store it, or null when unset.
    //
    // ADDITIVE ONLY, and the same shape /api/heartbeat already sends (see the
    // matching helper there). Without it the client only ever learned the expiry
    // on its FIRST heartbeat — so a freshly activated device showed no expiry at
    // all until a beat landed, which reads to a student as "the date never
    // appears". Returning it here makes the date correct from the activation
    // response onward, and older clients simply ignore the extra key.
    //
    // Never an empty string: an unset expiry must be `null`, because the client
    // distinguishes "no date recorded" from "the date is blank" to avoid showing
    // a paying student "expired" on a code that simply has no expiry set.
    function expiryForWire(record) {
        var raw = record.get("expires_at");
        var str = (raw === null || raw === undefined) ? "" : String(raw).trim();
        return str ? str : null;
    }

    // The expiry a term produces, as an ISO instant, or null when the code has
    // no term.
    //
    // THE TERM MODEL, IN ONE PLACE. A code carries `term_days` — how long ONE
    // purchase lasts, measured from activation. This turns that into the
    // instant we store in `expires_at`.
    //
    // Why the instant is materialised rather than derived on read: `expires_at`
    // is read by four hooks, the console and the client, all treating it as an
    // instant. Deriving it at every read would mean teaching all of them the
    // arithmetic, and any one of them could then disagree with the others. The
    // term is only how the value is COMPUTED; once written it is the single
    // source of truth, exactly as before.
    //
    // Returns null for a term of 0 or absent — "never expires". That is a
    // legitimate product (a permanent code), and it is also the state every
    // un-migrated row is in, so an old code keeps its current behaviour: an
    // absent term must never silently become a dated one.
    function expiryFromTerm(termDays, fromMs) {
        var days = Number(termDays);
        if (!isFinite(days) || days <= 0) return null;
        return new Date(fromMs + (days * 24 * 60 * 60 * 1000)).toISOString();
    }

    // ── Whether this code has already been redeemed ──
    //
    // ONE CODE, ONE USE. A code stops being available the moment it is first
    // activated. What it must NOT do is tie itself to a device: a student who
    // reinstalls, replaces a laptop, or resets their machine still owns the code
    // they paid for, and there is no device identity left in this system to
    // recognise them by.
    //
    // So the record of "this code is in use" is a timestamp on the code row
    // itself — `activated_at`, which the bind path already writes. Everything
    // that used to hang on `bound_fingerprint` (the device binding, the
    // `device_bindings` index, the identity migration) is gone; what remains is
    // the simplest possible statement of the rule and it is enforced by one
    // field.
    //
    // RE-ACTIVATION IS ALLOWED, and that is deliberate: `activated_at` set means
    // "already in use", not "refuse". The student presenting their own code again
    // is the recovery path — reinstalling, moving to a new machine, repairing a
    // broken install — and refusing it would recreate exactly the support call
    // this whole change exists to remove. The value is only used to tell the
    // FIRST activation ("Activation successful") from a later one ("Already
    // activated"), so the client can show the right message.
    //
    // Returns true when the code has been redeemed before now.
    function alreadyRedeemed(codeRec) {
        var at = codeRec.get("activated_at");
        if (at === null || at === undefined) return false;
        return String(at).trim() !== "";
    }


    try {
        var data = $apis.requestInfo(e).data;
        var code = (data.code || "").trim();
        // `fingerprint` is still ACCEPTED and still logged, but it no longer
        // decides anything: it is a coarse rate-limit key and a support
        // correlator. A deployed 3.2.x client keeps sending it, and refusing a
        // request for a missing one would break every install in the field, so
        // it is optional here.
        var fp = (data.fingerprint || "").trim();
        try { var addr = (e.request().remoteAddr || "").split(":"); var ip = addr[0] || ""; } catch(ex) { var ip = ""; }

        if (!code) return e.json(400, {code:400, message:"Missing code"});

        // Luhn-mod-N check (32-char charset matching client)
        var s = code.replace(/-/g,"").toUpperCase();
        var c = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789", n = c.length, ok = false, sum = 0, alt = false;
        for (var i = s.length - 2; i >= 0; i--) {
            var idx = c.indexOf(s[i]);
            if (idx === -1) { ok = false; break; }
            var v = idx;
            if (alt) { v *= 2; if (v >= n) v = v - n + 1; }
            sum += v; alt = !alt; ok = true;
        }
        if (ok) ok = ((n - (sum % n)) % n) === c.indexOf(s[s.length - 1]);
        if (!ok) return e.json(400, {code:400, message:"Invalid code format"});

        // Rate limiting — clean old + count recent
        $app.dao().db().newQuery("DELETE FROM activation_attempts WHERE created < datetime('now','-10 minutes')").execute();
        // rateKey is SHA256 hex or IP — strip anything non-alphanumeric for query safety.
        //
        // The "activate_" prefix namespaces this bucket. /api/code-lookup writes
        // "lookup_" keys into the same table, and the two must NOT share a
        // budget: a typo here would otherwise consume the read-only lookup that
        // exists to tell the student WHY the code was rejected. See the matching
        // note in code_lookup.pb.js.
        var rateKey = "activate_" + (fp || ip).replace(/[^a-zA-Z0-9]/g,"_");
        // NOTE: findRecordsByFilter is broken in this PB build (0.22.21) — it
        // returns zero rows for every filter, which silently DISABLED this rate
        // limit entirely (5 attempts / 10 min was never enforced). We count with
        // findRecordsByExpr, which works. Verified live 2026-09-19.
        var recentCount = 0;
        try {
            recentCount = $app.dao().findRecordsByExpr("activation_attempts",
                $dbx.exp("rate_key = {:k}", { k: rateKey })).length;
        } catch (countErr) {
            recentCount = 0; // fail open on a counting error rather than blocking users
        }
        if (recentCount >= 5) return e.json(429,{code:429, message:"Too many attempts"});

        // Log attempt
        var c2 = $app.dao().findCollectionByNameOrId("activation_attempts");
        var att = new Record(c2);
        att.set("ip",ip); att.set("rate_key",rateKey);
        att.set("fingerprint",fp.substring(0,16)+"****");
        att.set("code_attempted",code.substring(0,4)+"****");
        $app.dao().saveRecord(att);

        // Find code — use findFirstRecordByData which is simpler.
        // Lookup uses the canonical hyphenated form (the form codes are seeded
        // in); tolerate any pasted variant just like the heartbeat hook.
        var canonical = (s.length === 15)
            ? s.substring(0,2)+"-"+s.substring(2,6)+"-"+s.substring(6,10)+"-"+s.substring(10,14)+"-"+s.substring(14,15)
            : code;
        // NOTE: findFirstRecordByData THROWS "sql: no rows in result set" when
        // the code is absent — it does not return null. Catching it here turns
        // a perfectly ordinary "no such code" into a clean 404 instead of a 500
        // (the previous `if (!rec)` check was unreachable). See code_lookup.pb.js
        // for the same fix, found live on 2026-09-19.
        var rec = null;
        try {
            rec = $app.dao().findFirstRecordByData("codes", "code", canonical);
        } catch (notFound) {
            rec = null;
        }
        if (!rec) return e.json(404, {code:404, message:"Code not found"});

        // ── Expiry and suspension ──
        //
        // Checked FIRST, before anything else is decided, so a lapsed or
        // suspended code gets its real answer rather than a message about
        // redeemability. A lapsed code returns 410 (which the client already
        // knows how to display), and a suspended code returns 403 "Code
        // suspended" — a phrase the client matches on, pinned by the contract
        // test, so it must keep that exact substring.
        var expMs = parsePBDate(rec.get("expires_at"));
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(410, {code:410, message:"Code expired"});
        }
        if (rec.getBool("suspended")) return e.json(403, {code:403, message:"Code suspended"});

        // ── Redeemed already? ──
        //
        // `activated_at` is the whole of the single-use rule (see
        // `alreadyRedeemed` above). A code that has been redeemed before is NOT
        // refused — the student presenting their own code again is the recovery
        // path, and there is no device identity left to check it against. It
        // simply re-issues the entitlement and reports "Already activated" so
        // the client can say so.
        //
        // The tier config is returned on this path too, so a reinstalling student
        // refreshes stale connection parameters rather than keeping whatever was
        // cached before (see FIXES.md).
        if (alreadyRedeemed(rec)) {
            var tierVal2 = rec.getString("tier").replace(/[^a-zA-Z0-9_]/g, "_");
            // findFirstRecordByFilter (NOT findRecordsByFilter — see the
            // rate-limit note above; the list variant returns nothing here).
            var cfgRec2 = null;
            try { cfgRec2 = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal2 + "'"); } catch (e2) { cfgRec2 = null; }
            var resp2 = {code:200, message:"Already activated", tier:rec.getString("tier")};
            resp2.expires_at = expiryForWire(rec);
            if (cfgRec2) {
                try { resp2.server_config = JSON.parse(cfgRec2.get("config")); } catch(ex) { resp2.server_config = cfgRec2.get("config"); }
                resp2.udp_relay = cfgRec2.get("udp_relay");
            }
            return e.json(200, resp2);
        }

        // ── First redemption ──
        //
        // The expiry is recomputed from the TERM, now, because this is the
        // moment the student's clock starts. Previously `expires_at` was fixed
        // when the card was MINTED, so a card printed in February and sold in
        // May gave the buyer whatever was left of February's window — and
        // unsold stock decayed on the shelf.
        //
        // A code with no term (0/absent) keeps whatever expiry it already has,
        // which is the pre-term behaviour and is what every un-migrated row
        // relies on. Never clears a date to null unless the term says so, so
        // this cannot shorten a term that is already running.
        var nowMs = Date.now();
        var termDays = rec.get("term_days");
        var termExpiry = expiryFromTerm(termDays, nowMs);
        if (termExpiry) rec.set("expires_at", termExpiry);
        // The whole of the single-use rule: stamp the code as redeemed. This is
        // one field, written once, and `alreadyRedeemed` above is the only reader
        // — there is no device, no fingerprint and no index to keep in step.
        rec.set("activated_at", new Date().toISOString());
        $app.dao().saveRecord(rec);

        // Clean rate limiting
        $app.dao().db().newQuery("DELETE FROM activation_attempts WHERE rate_key={:key}").bind({key:rateKey}).execute();

        // Get tier config.
        // findFirstRecordByFilter, not findRecordsByFilter: the list variant
        // silently returns zero rows on this PB build, which would hand the
        // client a successful activation with NO server_config — a "connected"
        // app that cannot reach the internet.
        var tierVal = rec.getString("tier").replace(/[^a-zA-Z0-9_]/g, "_");
        var cfgRec = null;
        try { cfgRec = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal + "'"); } catch (e3) { cfgRec = null; }
        // `device_fingerprint` is echoed back because it is a FROZEN wire field:
        // deployed 3.2.x clients read it and would break if it vanished. It no
        // longer means "the device this code is bound to" — nothing does — and
        // the client has stopped using it, but the key stays for as long as a
        // released build might send the request.
        var resp = {code:200, message:"Activation successful", tier:rec.getString("tier"), device_fingerprint:fp};
        resp.expires_at = expiryForWire(rec);
        if (cfgRec) {
            // The tier config is passed through VERBATIM, so the UoT endpoint
            // reaches the client under its stored key "uot_port". That key is a
            // FROZEN wire contract — do not rename it. See the full note in
            // heartbeat.pb.js and FIXES.md entry 29.
            try { resp.server_config = JSON.parse(cfgRec.get("config")); } catch(ex) { resp.server_config = cfgRec.get("config"); }
            resp.udp_relay = cfgRec.get("udp_relay");
        }
        return e.json(200, resp);
    } catch(err) {
        return e.json(500, {code:500, message:err.message || String(err)});
    }
});
