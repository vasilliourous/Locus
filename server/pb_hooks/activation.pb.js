// Chrysalis Activation Hook — PocketBase 0.22 compatible
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

    // ── Whether the holder of this code blew their own sign-out race ──
    //
    // `released_at` is written by device_release.pb.js when a device signs out.
    // If it is LATER than `activated_at`, the device that is now presenting the
    // code had already told the hub it was done with it — which is the one case
    // where signing out must actually free the code.
    //
    // ── Why this exists at all, since an earlier version of this file refused ──
    //
    // This file used to allow any presenter of a redeemed code to re-activate it,
    // on the reasoning that "the student re-entering their own code" is the
    // reinstall path and there is no device identity to tell them from a stranger.
    // The cost of that reasoning is the defect it was reported as: a code someone
    // was still signed in with signed in fine on a second machine, for the whole
    // remaining term, with nothing on the hub recording a second device at all.
    // One code was one purchase and silently served any number of machines.
    //
    // The cost of the opposite rule — refuse every used code — is the reinstall
    // path that the 2026-10-03 `already_used` incident was about. A student who
    // reinstalls, or replaces a laptop that died, has no way to sign out first and
    // would be locked out of what they paid for.
    //
    // A sign-out is the one act that distinguishes the two cases without a device
    // identity, so it is the one that grants the release. The agent's sign-out
    // already reaches this hub (device_release.pb.js); before this, nothing here
    // ever READ it, which is why the report was "signing out does not update the
    // status in regards to the code".
    //
    // ── Why the comparison, and not just "release_count > 0" ──
    //
    // A counter that only ever grows would misread a code that was released once
    // and is now signed in on a second machine: the honest student who moves from
    // laptop to desktop signs out the laptop, activates the desktop, and then
    // installs on a third machine — and a `> 0` test would free the code for
    // anyone holding a photo of the card, for as long as the desktop stayed
    // signed in. The stamps are ordered by the same clock and are written by the
    // SAME save, so `released_at > activated_at` means "the sign-out happened
    // after the redemption that is now in force", and the claim is spent by the
    // redemption that follows it (which rewrites `activated_at` to a later
    // instant). One sign-out, one activation.
    //
    // `parsePBDate` is the shared declaration at the top of this callback.
    function releasedSinceActivation(codeRec) {
        var atMs = parsePBDate(codeRec.get("activated_at"));
        var relMs = parsePBDate(codeRec.get("released_at"));
        if (!atMs || isNaN(atMs)) return false;
        if (!relMs || isNaN(relMs)) return false;
        return relMs > atMs;
    }

    // ── Writing a column that the live hub may not have yet ──
    //
    // WHAT THIS IS NOT FOR, stated first because an earlier draft of this comment
    // got it backwards and the correction is the point.
    //
    // A missing column does NOT fail the save. Verified against PocketBase
    // 0.22.21 source, not inferred: `Record.Set` stores an undefined key in
    // `m.data` without complaint (models/record.go), and `Dao.update` writes
    // `ColumnValueMap()`, which iterates the COLLECTION SCHEMA ONLY
    // (daos/base.go → models/record.go). An unknown field is therefore never in
    // the UPDATE. It is silently dropped.
    //
    // So `rec.set("first_seen_at", …)` on the live hub — whose `codes` table has
    // no such column — was harmless to the save, and the "activation answers 500"
    // story this comment used to tell was wrong. What actually happened is quieter
    // and worse for an operator: the write was ACCEPTED AND DISCARDED, so the
    // console's "issued before" marker simply never appeared and nothing anywhere
    // said why.
    //
    // WHAT THIS HELPER IS ACTUALLY FOR, then:
    //   1. It makes the discard VISIBLE. `setIfPresent` returns false, so a hook
    //      can tell the difference between "written" and "silently dropped"
    //      instead of both looking like success.
    //   2. It keeps the intent in one place, so bringing the schema up to date
    //      (scripts/seed-pb.py adds missing columns) immediately restores the
    //      behaviour without touching any hook.
    //
    // The fields that DECIDE anything (`activated_at`, `expires_at`, `suspended`,
    // `tier`, `released_at`) deliberately do NOT go through it. They are columns
    // every deployed hub has had since the first seed; if one were ever absent,
    // writing it and having it dropped should be a loud problem, not a quiet skip.
    function setIfPresent(record, name, value) {
        try {
            // `getByName` returns null when the field is absent — it does not
            // throw (verified: models/schema/schema.go). The try/catch is for a
            // record whose collection could not be resolved at all, which is a
            // different failure and must not be mistaken for "column missing".
            if (record.collection().fields.getByName(name)) {
                record.set(name, value);
                return true;
            }
            return false;
        } catch (noCollection) {
            return false;
        }
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

        // ── Who is asking, when loopback cannot say ──
        //
        // On the activation TUNNEL path (server/modules/09-activation-tunnel.sh)
        // this hook is reached at 127.0.0.1:8090, so `ip` above is ALWAYS
        // loopback and EVERY tunnelled student shares one `activate_` bucket.
        // This request is the one that mints entitlements, so it does not
        // require a fingerprint (`fp` is optional — see the note above; deployed
        // 3.2.x clients still send one and a missing one must not break them).
        // Measured consequence before this fix: the first five students in a
        // class activate and the rest are told "too many attempts", which the
        // client reports as a failure to reach the hub.
        //
        // The path filter sets `X-Chrysalis-Bootstrap` to a per-client digest it
        // computes itself (peer address keyed with the bootstrap key). It
        // OVERWRITES any copy the client sent, so this value is the filter's
        // statement about the client, not the client's about itself. Caddy's
        // path sets no such header, so nothing changes there.
        //
        // Absent, empty or absurdly long is treated as absent: an unbounded
        // string here would become an unbounded rate-key column.
        var bootstrapId = "";
        try {
            var allHeaders = $apis.requestInfo(e).headers || {};
            // TWO SPELLINGS, CURRENT FIRST. The header is a MATCHED PAIR with the
            // path-filter module (server/modules/09-activation-tunnel.sh, IDENTITY_HEADER)
            // and with the client that sends it. It was renamed on 2026-11-23 and the
            // hub runs its OWN copy from /root/server/, so a deployed hub and a
            // deployed client can be on opposite sides of the rename for the length
            // of a release cycle.
            //
            // If the two sides disagree and nobody accepts both, the header is simply
            // ABSENT — no error, and every tunnelled client collapses into ONE shared
            // rate-limit bucket, which is a silent loss of the protection rather than
            // a visible failure. Accepting both spellings costs one `||` and removes
            // the entire window. DEPRECATED ALIAS: drop the `x-locus-bootstrap` arm
            // once no deployed client sends it (NAME-MIGRATION.md §3.4).
            var rawId = allHeaders["x-chrysalis-bootstrap"] || allHeaders["X-Chrysalis-Bootstrap"]
                || allHeaders["x-locus-bootstrap"] || allHeaders["X-Locus-Bootstrap"] || "";
            rawId = String(rawId).trim();
            if (rawId.length >= 16 && rawId.length <= 64) bootstrapId = rawId;
        } catch(ex) { bootstrapId = ""; }

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
        var rateKey = "activate_" + (fp || bootstrapId || ip).replace(/[^a-zA-Z0-9]/g,"_");
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
        // `ip` is loopback on the tunnel path, and writing it would make every
        // tunnelled student's row claim to come from 127.0.0.1 — which is worse
        // than an empty column, because it looks like data. The rate key already
        // carries the per-client digest, so the address is left empty rather
        // than filled with something untrue.
        att.set("ip", bootstrapId ? "" : ip); att.set("rate_key",rateKey);
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
        // ── Suspension, and why it is ALSO re-checked below ──
        //
        // This check answers a new activation. The re-issue arm further down
        // answers a student re-entering their own redeemed code, and it is a
        // SECOND route to the same tier config — so a suspension that only
        // guarded here would be bypassed by re-activating. It is checked again
        // immediately before that arm returns, deliberately, because the two
        // arms are edited independently and the second one is the one a
        // suspended student would actually reach.
        if (rec.getBool("suspended")) return e.json(403, {code:403, message:"Code suspended"});

        // ── One code, one machine ──
        //
        // `activated_at` is the whole of the single-use rule (see
        // `alreadyRedeemed` above), and this is the arm that ENFORCES it. It used
        // to hand a full entitlement to anyone presenting a redeemed code, on the
        // reasoning that a re-presented code is a reinstall. The measured cost was
        // that a code still signed in on one machine signed in on a second one for
        // the whole remaining term, with the hub recording nothing — see
        // `releasedSinceActivation` above for the full argument.
        //
        // So there are exactly two ways past this point with a redeemed code:
        //
        //   1. The device that holds it signed out (device_release.pb.js) after the
        //      redemption that is now in force. The next activation consumes that
        //      sign-out and becomes the redemption in force.
        //   2. Nobody: a second device gets 409 and the student is told to sign
        //      the first one out first.
        //
        // Re-activating a code this device legitimately holds is still an "Already
        // activated" success, because the alternative is a locked-out student who
        // did nothing wrong: the code IS in force and the clock is running, and
        // re-issuing the same entitlement changes nothing on the hub.
        if (alreadyRedeemed(rec)) {
            // Re-asserted here, not just at the top: this arm RETURNS a tier
            // config, which is the whole entitlement. One check on the way in
            // is not enough when the arm that hands out the goods is the one a
            // suspended student uses to get back in — and it is 3 lines away
            // from an `edit` that would not obviously need to keep it.
            if (rec.getBool("suspended")) return e.json(403, {code:403, message:"Code suspended"});

            if (!releasedSinceActivation(rec)) {
                // 409, not 403: the client keeps a refusal from a real answer, and
                // a suspension must stay distinguishable from "this is somebody
                // else's key" — the kick-out screen keys on the word "suspended".
                //
                // The sentence is the operator's only lever here: there is no
                // device to name and no remote release, so the exact fix (sign out
                // on the machine that is already using it, then enter the code on
                // this one) has to be IN the message. So does the sentence that
                // stops someone re-selling a key they cannot use. Pinned by
                // `a_redeemed_code_is_refused_on_a_second_device` in
                // client/src-tauri/tests/activation_contract.rs.
                return e.json(409, {
                    code: 409,
                    message: "This code is already in use on another device. Sign out on that device first, then enter the code here",
                    already_in_use: true
                });
            }

            // A sign-out is waiting, so this is the first redemption on this
            // device: consume it. `activated_at` is moved to now, which both
            // records the redemption that is now in force and spends the claim —
            // a second machine presenting the same code immediately afterwards
            // finds `released_at` no longer later than it and is refused.
            //
            // `first_seen_at` is deliberately NOT touched: the code has been
            // issued before, and the console's "issued before" marker would be a
            // lie if this path rewrote it.
            rec.set("activated_at", new Date().toISOString());
            $app.dao().saveRecord(rec);
            var tierVal2 = rec.getString("tier").replace(/[^a-zA-Z0-9_]/g, "_");
            // findFirstRecordByFilter (NOT findRecordsByFilter — see the
            // rate-limit note above; the list variant returns nothing here).
            var cfgRec2 = null;
            try { cfgRec2 = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal2 + "'"); } catch (e2) { cfgRec2 = null; }
            var resp2 = {code:200, message:"Activation successful", tier:rec.getString("tier")};
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
        // ── A successful activation CONSUMES the override ──
        //
        // A released code arrives HERE, at the first-redemption arm, because
        // `activated_at` is null. Its `evict_requested_at` was set to cut off the
        // PREVIOUS holder. If this save left that predicate in place, the new
        // holder's very next heartbeat would be refused — the resale would appear
        // to work for one beat and then cut off the student who just paid. That is
        // a worse bug than the one the release was fixing.
        //
        // Written in the SAME save as `activated_at`, for the same reason
        // `first_seen_at` is: two writes would be two chances to record only one.
        //
        // `force_state` is cleared too — the code has a holder again, so there is
        // nothing left for it to pin. `first_seen_at` is deliberately NOT touched
        // (see below): the code has been issued before, and that marker must
        // survive a re-issue.
        //
        // Through `setIfPresent`: these columns are absent on a hub that has not
        // been re-seeded, and PocketBase silently discards a write to an undefined
        // field rather than failing the save.
        setIfPresent(rec, "force_state", "");
        setIfPresent(rec, "evict_requested_at", null);
        setIfPresent(rec, "evict_reason", "");
        // The FIRST-redemption marker, written once and never overwritten.
        //
        // `activated_at` cannot answer "is this code already someone's?" after
        // an operator RELEASE clears it: the console then shows a code that has
        // already been issued as plain "available", which is how a code gets
        // handed to a second student while the first still holds a card. This
        // field survives a release, so the console can say "issued before" and
        // an operator can find the code that was double-sold.
        //
        // Written in the SAME save as `activated_at` on purpose: two writes to
        // one record would be two chances to record only one of them.
        // `alreadyRedeemed` is still the reader of `activated_at` alone — this
        // is an operator signal and changes no behaviour for the student.
        // Through `setIfPresent`: these columns are ABSENT on the live hub
        // (measured 2026-11-27), and PocketBase silently discards a write to an
        // undefined field rather than failing the save — so the marker was simply
        // never recorded and the console showed a re-issued code as never issued.
        // The helper makes that visible instead of silent. See it for the source
        // verification, and note the entitlement write below is unaffected either
        // way: `activated_at` is a real column and always reaches the UPDATE.
        if (!rec.get("first_seen_at")) {
            setIfPresent(rec, "first_seen_at", new Date().toISOString());
            // Truncated, like every other fingerprint this hub stores. It is
            // corroboration for an operator, not a device index: nothing reads
            // it back to make a decision.
            setIfPresent(rec, "first_seen_fingerprint", fp.substring(0, 16) + "****");
        }
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
        // ── Informational only: was this code released by an override? ──
        //
        // ADDITIVE, and it REFUSES NOBODY. A released code must stay activatable
        // by the next holder — that is what releasing is for — so this key exists
        // only so that a support conversation about "my code changed hands" has an
        // answer. Old clients ignore it.
        //
        // Deliberately NOT a read of `evict_requested_at` in a way that could
        // grow a refusal: the eviction is the heartbeat's alone
        // (heartbeat.pb.js), because only the device that already holds the code
        // should be cut off, and activation cannot tell the holder from the buyer.
        if (String(rec.get("force_state") || "") === "released"
                || String(rec.get("evict_requested_at") || "")) {
            resp.released_by_override = true;
        }
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
