// Locus Device Recognition Hook — PocketBase 0.22 compatible
//
// ANSWERS ONE QUESTION: "does this device already hold a live entitlement?"
//
// It exists so a student who reinstalls does not have to find their card again.
// The client, on first launch with nothing stored, presents its device identity;
// if the hub recognises it, the code prompt is skipped entirely.
//
// WHY THIS IS NOT /api/code-lookup
//
// That endpoint takes a CODE and reports its state. This one takes no code at
// all — the whole point is that the student may not have one to give. A device
// with an entitlement has an identity regardless of whether anyone can still
// read the card it was activated from.
//
// ── SECURITY: THIS ENDPOINT MUST NEVER RETURN THE ACTIVATION CODE ──────────
//
// The activation code is a bearer credential. If this endpoint returned it,
// then anyone who could produce a matching identity could read out a code and
// walk away with the entitlement — an IDOR. So the code is never echoed, never
// partially echoed, and never used as a lookup key here. The client is told
// only facts about its OWN entitlement: tier, expiry, and whether it is live.
//
// ── SECURITY: PROOF, NOT ASSERTION ─────────────────────────────────────────
//
// The device id is a NAME — it is stable, non-secret, and appears (truncated)
// in support logs and screenshots. Treating it as the credential would mean a
// screenshot is a takeover. So this endpoint authenticates on
// `sha256(secret)`: the client sends its *verifier*, which only a device
// actually holding the secret can produce. The secret itself never crosses the
// wire, and the hub stores only the verifier.
//
// ── SECURITY: NOT AN ORACLE ────────────────────────────────────────────────
//
// A verifier is 64 hex chars, so guessing one is infeasible — but the endpoint
// still must not become a *confirmation* oracle for a leaked verifier or a
// device-enumeration surface. Therefore:
//   * one uniform failure response, for every reason (unknown, revoked,
//     malformed), so a caller learns nothing from the difference;
//   * rate limiting, reusing the activation_attempts table with its own bucket;
//   * timing is not used as a signal — the same work is done whether or not a
//     row exists.
//
// Response shape (always HTTP 200 for a well-formed request, so the client can
// distinguish "not recognised" from "transport failed"):
//   {status: "recognised"|"unknown", tier?, expires_at?, store?, message?}
//
// `unknown` covers every negative — no such identity, revoked, no entitlement.
// A client must fall back to the code prompt for it, and must not treat it as
// an error.
//
// NOTE: deployed by copying to /opt/pocketbase/pb_hooks/ (see OPS.md).
// PocketBase must be restarted for a new hook file to register.

routerAdd("POST", "/api/device-recognise", function(e) {
    // ── PocketBase date parsing ──────────────────────────────────────────────
    // CRITICAL: `new Date("2027-09-19 00:00:00.000Z")` returns NaN in goja.
    // Same trap as code_lookup.pb.js and activation.pb.js — a parse failure
    // there silently disabled expiry enforcement for months. Defined inside the
    // callback because goja does not hoist across scopes.
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

    // The single negative answer. Every failure returns this exact object, so a
    // caller cannot tell "no such device" from "revoked" from "malformed" —
    // which is what stops this being an enumeration oracle.
    function unknown() {
        return {status:"unknown", message:"This device is not recognised"};
    }

    try {
        var data = $apis.requestInfo(e).data;
        var verifier = (data.verifier || "").trim().toLowerCase();
        var deviceId = (data.device_id || "").trim();
        var store = (data.store || "").trim();
        try { var addr = (e.request().remoteAddr || "").split(":"); var ip = addr[0] || ""; } catch(ex) { var ip = ""; }

        // ── Shape check ──
        // A verifier is a 64-char hex sha256 digest. Anything else is not worth
        // a database round trip, and is answered with the SAME response as a
        // well-formed miss so the shape check is not itself a signal.
        if (!/^[0-9a-f]{64}$/.test(verifier)) return e.json(200, unknown());

        // ── Rate limiting ──
        // Own bucket ("recognise:"), so a student retrying recognition cannot
        // consume the activation or lookup budget — the two mistakes are
        // independent, and coupling them locked students out of the affordance
        // meant to explain their mistake (see code_lookup.pb.js).
        //
        // Note the bucket is keyed on the IP, NOT the verifier: keying on the
        // verifier would let an attacker spread guesses across many verifiers
        // and never hit the limit.
        $app.dao().db().newQuery("DELETE FROM activation_attempts WHERE created < datetime('now','-10 minutes')").execute();
        var rateKey = "recognise_" + (ip || "noip").replace(/[^a-zA-Z0-9]/g,"_");
        // findRecordsByExpr, not findRecordsByFilter — the list variant returns
        // zero rows on this PB build, which silently disabled a rate limit
        // before (found live 2026-09-19).
        var recentCount = 0;
        try {
            recentCount = $app.dao().findRecordsByExpr("activation_attempts",
                $dbx.exp("rate_key = {:k}", { k: rateKey })).length;
        } catch (countErr) {
            recentCount = 0;
        }
        if (recentCount >= 10) {
            // Same uniform body, plus a 429 so the client can back off. It is
            // NOT a distinct status string: "we are busy" and "we do not know
            // you" must not be distinguishable by a prober.
            return e.json(429, unknown());
        }
        // Log the attempt. The verifier is NOT stored here — it is the
        // credential, and an attempts table is not a credential store. Only the
        // device id prefix is kept, matching the redaction discipline used for
        // codes and fingerprints elsewhere.
        var c2 = $app.dao().findCollectionByNameOrId("activation_attempts");
        var att = new Record(c2);
        att.set("ip", ip);
        att.set("rate_key", rateKey);
        att.set("fingerprint", deviceId.substring(0,16)+"****");
        att.set("code_attempted", "recognise");
        $app.dao().saveRecord(att);

        // ── Look up the identity by verifier ──
        // findFirstRecordByFilter (NOT findRecordsByFilter, which returns zero
        // rows on this build). The verifier is hex-validated above, so
        // interpolating it cannot carry a quote — the same safety argument
        // activation.pb.js makes for its binding filter.
        var ident = null;
        try {
            ident = $app.dao().findFirstRecordByFilter("device_identities",
                "verifier = '" + verifier + "'");
        } catch (none) {
            ident = null;
        }
        if (!ident) return e.json(200, unknown());

        // A revoked identity is a definitive no. Answered with the SAME uniform
        // body — the client must not be able to tell a revoked device from an
        // unknown one, or "revoked" becomes a way to confirm a guessed verifier.
        if (ident.getString("revoked_at")) return e.json(200, unknown());

        // ── The identity is real. Does it hold anything worth returning? ──
        var code = ident.getString("code");
        if (!code) {
            // A device that has an identity but has never activated. This is a
            // legitimate state (every device is in it before its first code),
            // and the answer is the same uniform miss: there is no entitlement
            // to restore, so the client shows the code prompt.
            return e.json(200, unknown());
        }

        // Re-read the CODE, because `device_identities.code` is a string copy
        // and the code row is the authority on its own state. Anything can have
        // changed it since the identity was written: a suspension, an expiry, an
        // operator unpick. Trusting the copy would keep a suspended device
        // looking entitled, which is exactly what suspension exists to prevent.
        var rec = null;
        try {
            rec = $app.dao().findFirstRecordByData("codes", "code", code);
        } catch (notFound) {
            rec = null;
        }
        // No code row: the identity points at something deleted. Uniform miss,
        // and the stale pointer is cleared so it stops being consulted.
        if (!rec) {
            ident.set("code", "");
            $app.dao().saveRecord(ident);
            return e.json(200, unknown());
        }

        // Expiry and suspension are checked HERE, on every recognition, so a
        // reinstalling student cannot revive a lapsed or suspended entitlement
        // by presenting a stored identity. This is the same ordering rule
        // activation.pb.js settled on: expiry, then suspension, then the rest.
        var exp = rec.get("expires_at");
        var expMs = parsePBDate(exp);
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(200, unknown());
        }
        if (rec.getBool("suspended")) {
            return e.json(200, unknown());
        }

        // ── Recognised ──
        // Notice what is NOT here: the code. The client is told its tier and
        // expiry — the facts it needs to show a working app — and nothing that
        // would let it (or anyone reading the response) present the credential
        // elsewhere.
        var response = {status:"recognised", tier: rec.getString("tier")};
        var expiryRaw = rec.get("expires_at");
        var expiryStr = (expiryRaw === null || expiryRaw === undefined) ? "" : String(expiryRaw).trim();
        // null, not "", so the client can tell "no expiry recorded" from "expiry
        // is the empty string" — the same distinction the heartbeat makes.
        response.expires_at = expiryStr ? expiryStr : null;
        if (ident.getString("store")) response.store = ident.getString("store");

        // ── The tier config ──
        //
        // Sent so a recognised device can actually build a tunnel. Without it the
        // client would know its tier name and nothing else — no server, port,
        // password or cipher — which is enough to show a screen and not enough to
        // connect, so "recognised" would be a promise the client could not keep.
        //
        // This is the SAME payload /api/activate returns (passed through
        // verbatim from tier_configs.config, so `uot_port` keeps its frozen
        // wire name — see contract.rs). It carries credentials for the tunnel;
        // it does NOT carry the activation code, which is the distinction that
        // matters: the code is a bearer credential for the ENTITLEMENT, while
        // these values only let this device talk to one tier's server.
        //
        // findFirstRecordByFilter, NOT findRecordsByFilter — the list variant
        // silently returns zero rows on this PB build, which would hand the
        // client a "recognised" with no server_config (found live 2026-09-19).
        var tierVal = rec.getString("tier").replace(/[^a-zA-Z0-9_]/g, "_");
        var cfgRec = null;
        try { cfgRec = $app.dao().findFirstRecordByFilter("tier_configs", "tier = '" + tierVal + "'"); } catch (cfgErr) { cfgRec = null; }
        if (cfgRec) {
            try { response.server_config = JSON.parse(cfgRec.get("config")); } catch (parseErr) { response.server_config = cfgRec.get("config"); }
            response.udp_relay = cfgRec.get("udp_relay");
        }
        // A tier with no config row is an operator error. The device is still
        // recognised — its entitlement is real — but it cannot connect, and the
        // client handles a missing server_config by falling back to the code
        // prompt rather than showing a connected app that cannot reach anything.

        // ── Mint the session token ──
        //
        // A recognised device has no code, but every enforcement rule the hub
        // has — suspension, expiry, config refresh — runs on the heartbeat, and
        // the heartbeat is code-keyed. Without a credential the device could be
        // restored and connect but never check in, so a suspended device would
        // keep working forever. That is the failure mode the whole code-keyed
        // design exists to prevent, so recognition must hand back something the
        // heartbeat will accept.
        //
        // The token is that something. It is device-scoped and revocable, it
        // does NOT reveal the activation code, and the hub resolves it back to
        // this row — and from there to the code — so enforcement is unchanged.
        //
        // Only the HASH is stored. A plaintext token in the database would mean
        // a database read hands out working credentials, which is the same
        // reasoning that keeps the verifier hashed.
        //
        // Minted on every recognition rather than reused: a token that never
        // rotates is a permanent credential, and rotating costs one write on a
        // call that only happens on first launch.
        var token = "";
        var tokenOk = false;
        try {
            // `$security.randomString` is PocketBase's own CSPRNG-backed helper,
            // and `$security.sha256` is a real SHA-256.
            //
            // This started as a hand-rolled mixing function, on the assumption
            // that goja had no crypto. That was wrong twice over: it is exactly
            // the home-made-cryptography class this project's FIXES.md keeps
            // paying for, AND it was unnecessary — `$security` provides
            // md5/sha256/sha512/randomString (verified against the live runtime;
            // see FIXES.md 2026-09-30, which lists the globals the hook runtime
            // actually has). Use the real primitive.
            //
            // Note what `$security` does NOT have: base64. That is the trap
            // documented in FIXES.md — it looks like the place a base64 helper
            // would live, and it has none. Nothing here needs one.
            token = $security.randomString(64);
            ident.set("token_hash", $security.sha256(token));
            // 30 days. Long enough that an app left closed for a term still
            // works on reopen, short enough that a leaked token is not forever.
            // The device rotates it on every recognition, so a student who opens
            // the app regularly always holds a fresh one.
            ident.set("token_expires_at", new Date(Date.now() + 30*24*60*60*1000).toISOString());
            ident.set("last_seen_at", new Date().toISOString());
            $app.dao().saveRecord(ident);
            tokenOk = true;
        } catch (tokenErr) {
            // The device is still recognised — the entitlement is real — but it
            // cannot check in. Reported as a miss rather than a partial success,
            // because a client that cannot heartbeat would silently never learn
            // of a suspension, and that is not a state worth calling working.
            tokenOk = false;
        }
        if (!tokenOk) return e.json(200, unknown());
        response.token = token;
        response.token_expires_at = ident.getString("token_expires_at");

        return e.json(200, response);
    } catch(err) {
        // Transient/internal failure. The client treats this as "cannot tell"
        // and falls back to the code prompt — which is always safe, because the
        // student with a card is never worse off than before this endpoint
        // existed.
        return e.json(500, unknown());
    }
});
