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

    // Whether this device is already bound to a DIFFERENT code.
    //
    // ONE CODE PER DEVICE. Without this, a device could activate code after
    // code and hold every one: `bound_fingerprint` lives on the code, so the
    // code cannot see its siblings, and nothing else looked either. The
    // dashboard then counted a single machine as several activated devices.
    //
    // Reads the binding index, whose `fingerprint` field is UNIQUE, so the
    // constraint is enforced by the schema even if this check is ever wrong.
    // A released row (released_at set) does not count — an operator who
    // released the device meant to free it.
    //
    // Returns the OTHER code's string, or "" when the device is free or is
    // re-activating the very code it already holds.
    function deviceBoundToOtherCode(fp, thisCode) {
        // Normalised identically to `recordBinding` below. The two MUST agree
        // on the stored key or the check would look in one place while the
        // write went to another, and the uniqueness rule would silently not
        // apply. Fingerprints are hex by construction, so this is belt and
        // braces — but the two halves have to match either way.
        var safeFp = String(fp).replace(/[^a-zA-Z0-9]/g, "");
        if (!safeFp) return "";
        var rows = null;
        try {
            rows = $app.dao().findRecordsByExpr("device_bindings",
                $dbx.exp("fingerprint = {:f}", { f: safeFp }));
        } catch (lookupErr) {
            // Fail OPEN, deliberately. A lookup failure must not lock a paying
            // student out of a code they legitimately own; the schema's unique
            // constraint is the backstop that cannot be skipped.
            return "";
        }
        for (var i = 0; i < rows.length; i++) {
            var row = rows[i];
            if (row.getString("released_at")) continue;
            var held = row.getString("code");
            if (held && held !== thisCode) return held;
        }
        return "";
    }

    // Records this device's binding in the index.
    //
    // Best-effort by design: the entitlement itself lives on the code row, and
    // a failure to maintain the index must not fail an activation that has
    // already succeeded. The unique constraint means a duplicate attempt is
    // rejected by the schema rather than creating a second live binding.
    function recordBinding(fp, code, tier) {
        try {
            // Normalised ONCE and used for both the lookup and the insert.
            //
            // These two must agree exactly. An earlier version looked up the
            // stripped value but STORED the raw one, so a fingerprint carrying
            // any strippable character would be written under a key the lookup
            // could never find — the uniqueness check would then miss the row
            // and let the same device bind a second code, which is the exact
            // failure this collection exists to prevent.
            //
            // Fingerprints are hex by construction (a SHA-256 digest, or the
            // client's random-hex fallback), so this should never differ in
            // practice. It is done anyway because "should never differ" is not
            // a guarantee, and the cost of being wrong here is silent.
            //
            // The value is also interpolated into a filter string rather than
            // passed as a parameter: no caller in this tree passes
            // findFirstRecordByFilter a parameter object, and discovering that
            // form's behaviour on this PocketBase build in production is not
            // worth the tidiness. Stripping to [a-zA-Z0-9] is what makes the
            // interpolation safe — it cannot carry a quote.
            var safeFp = String(fp).replace(/[^a-zA-Z0-9]/g, "");
            if (!safeFp) return;
            var fresh = null;
            try {
                fresh = $app.dao().findFirstRecordByFilter("device_bindings",
                    "fingerprint = '" + safeFp + "'");
            } catch (none) { fresh = null; }
            if (fresh) {
                // Re-binding (e.g. after an operator release): revive the row
                // rather than inserting a second one, which the unique index
                // would refuse anyway.
                fresh.set("code", code);
                fresh.set("tier", tier);
                fresh.set("bound_at", new Date().toISOString());
                fresh.set("released_at", null);
                fresh.set("release_reason", "");
                $app.dao().saveRecord(fresh);
                return;
            }
            var coll = $app.dao().findCollectionByNameOrId("device_bindings");
            var rec = new Record(coll);
            rec.set("fingerprint", safeFp);
            rec.set("code", code);
            rec.set("tier", tier);
            rec.set("bound_at", new Date().toISOString());
            $app.dao().saveRecord(rec);
        } catch (bindErr) {
            // Swallowed on purpose — see above.
        }
    }

    try {
        var data = $apis.requestInfo(e).data;
        var code = (data.code || "").trim();
        var fp = (data.fingerprint || "").trim();
        try { var addr = (e.request().remoteAddr || "").split(":"); var ip = addr[0] || ""; } catch(ex) { var ip = ""; }

        if (!code) return e.json(400, {code:400, message:"Missing code"});
        if (!fp) return e.json(400, {code:400, message:"Missing device fingerprint"});

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

        // Check binding
        var boundFp = rec.getString("bound_fingerprint");

        // ── Expiry and suspension, checked BEFORE the binding ──
        //
        // These used to live only on the first-activation path, BELOW the
        // `if (boundFp)` re-activation branch that returns early — so a device
        // that was already bound kept re-activating successfully forever, even
        // after its code expired or was suspended. The check was unreachable for
        // exactly the machines it was most likely to matter for.
        //
        // Order is now: expiry → suspension → binding. A lapsed code gets a
        // clear 410 on both paths (the same answer the client already knows how
        // to display), and a suspended code still returns 403 "Code bound to
        // another device" first when the fingerprint differs, so suspension
        // status is not leaked to a probing device.
        var expMs = parsePBDate(rec.get("expires_at"));
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(410, {code:410, message:"Code expired"});
        }
        var suspended = rec.getBool("suspended");

        if (boundFp) {
            // Compared in NORMALISED form on both sides.
            //
            // The stored value is normalised by the bind path, so comparing it
            // against a raw incoming fingerprint would refuse a device that is
            // re-activating its OWN code — the worst kind of false positive,
            // since it tells a paying student their code belongs to someone
            // else and sends them to a middleman. Normalising both sides makes
            // the comparison mean what it says: "is this the same device?".
            var incomingFp = String(fp).replace(/[^a-zA-Z0-9]/g, "");
            if (boundFp !== incomingFp) return e.json(403, {code:403, message:"Code bound to another device"});
            if (suspended) return e.json(403, {code:403, message:"Code suspended"});
            // Same-device re-activation: return the current tier config too, so
            // clients can refresh stale connection parameters (see FIXES.md).
            var tierVal2 = rec.getString("tier").replace(/[^a-zA-Z0-9_]/g, "_");
            // findFirstRecordByFilter (NOT findRecordsByFilter — see the rate-limit
            // note above; the list variant returns nothing on this PB build).
            var cfgRec2 = null;
            try { cfgRec2 = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal2 + "'"); } catch (e2) { cfgRec2 = null; }
            // Make sure the index knows about this binding. A code bound before
            // the index existed (or whose index row was lost) would otherwise
            // hold an entitlement the index cannot see, and the device could
            // then bind a SECOND code through the check below. Repair on read.
            recordBinding(fp, rec.getString("code"), rec.getString("tier"));
            var resp2 = {code:200, message:"Already activated", tier:rec.getString("tier"), device_fingerprint:boundFp};
            resp2.expires_at = expiryForWire(rec);
            if (cfgRec2) {
                try { resp2.server_config = JSON.parse(cfgRec2.get("config")); } catch(ex) { resp2.server_config = cfgRec2.get("config"); }
                resp2.udp_relay = cfgRec2.get("udp_relay");
            }
            return e.json(200, resp2);
        }
        if (suspended) return e.json(403, {code:403, message:"Code suspended"});

        // ── One code per device ──
        //
        // Refuse when this device already holds a DIFFERENT live code. Checked
        // HERE, after the code's own expiry/suspension checks, so a student with
        // a lapsed code is told that rather than being sent to a middleman about
        // a binding they did not create.
        //
        // 409, NOT 403. 403 is already overloaded on this endpoint for both
        // "suspended" and "bound to another device", and the client tells them
        // apart by looking for the substring "suspended" in the message — a
        // contract pinned by activation_contract_test. Reusing 403 here would
        // make an old client report the wrong account state, and the message
        // must therefore also avoid that word.
        var otherCode = deviceBoundToOtherCode(fp, rec.getString("code"));
        if (otherCode) {
            return e.json(409, {
                code: 409,
                message: "This device is already activated on another Locus code. " +
                         "One code works on one device — contact the person who sold " +
                         "you this code and they can move it for you."
            });
        }

        // Bind device.
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
        // The code's `bound_fingerprint` and the binding index MUST hold the
        // same value, or the two halves of "one code per device" disagree: the
        // code names a device the index cannot see, so the check misses it.
        //
        // `recordBinding` normalises internally, so this writes the normalised
        // form too. Both sides come from the same expression on purpose — see
        // the note in `recordBinding` for the latent bug this prevents.
        var boundFpNormalised = String(fp).replace(/[^a-zA-Z0-9]/g, "");
        rec.set("bound_fingerprint", boundFpNormalised);
        rec.set("activated_at", new Date().toISOString());
        $app.dao().saveRecord(rec);
        recordBinding(boundFpNormalised, rec.getString("code"), rec.getString("tier"));

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
