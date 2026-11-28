// Chrysalis Code Lookup Hook — PocketBase 0.22 compatible
//
// READ-ONLY pre-check for activation codes. The client calls this from the
// activation screen so a student learns whether their code is actually
// recognised BEFORE committing to the full /api/activate round-trip (which
// binds the device and can take up to 30s).
//
// GUARANTEES:
//   * NEVER binds a device, NEVER writes to the codes collection.
//   * Shares the same Luhn-mod-N precheck and canonical-code lookup as
//     /api/activate, so the two endpoints cannot disagree about what a code is.
//   * Reuses the activation_attempts table for rate limiting, so it cannot be
//     used as an unbounded code-enumeration oracle.
//
// Response shape (always HTTP 200 for a well-formed request so the client can
// distinguish "code is bad" from "transport failed"):
//   {status: "ok"|"unbound"|"bound_this_device"|"bound_other"|
//            "suspended"|"expired"|"not_found", tier?, expires_at?, message?}
//
// NOTE: deployed by copying this file to /opt/pocketbase/pb_hooks/ (see OPS.md).
// PocketBase must be restarted for a new hook file to register.

routerAdd("POST", "/api/code-lookup", function(e) {
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

    // Is a sign-out waiting to be claimed? Same comparison as
    // activation.pb.js → `releasedSinceActivation`, kept here because goja does
    // not hoist across files and because the two must agree: this endpoint tells
    // the student whether the code will activate, and activation is the authority
    // that decides. A divergence would put "ready" on screen and a 409 after it.
    function releasedSince(activatedAt, releasedAt) {
        var atMs = parsePBDate(activatedAt);
        var relMs = parsePBDate(releasedAt);
        if (!atMs || isNaN(atMs)) return false;
        if (!relMs || isNaN(relMs)) return false;
        return relMs > atMs;
    }

    try {
        var data = $apis.requestInfo(e).data;
        var code = (data.code || "").trim();
        var fp = (data.fingerprint || "").trim();
        try { var addr = (e.request().remoteAddr || "").split(":"); var ip = addr[0] || ""; } catch(ex) { var ip = ""; }

        // The per-client identity the activation tunnel's path filter sets; see
        // the longer note in activation.pb.js. This endpoint REQUIRES a
        // fingerprint, so its bucket is already per-client in practice — but the
        // header is read here too so that a client which fails to send one (a
        // modified build, a future release) does not fall back to the one shared
        // loopback bucket the tunnel path has.
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

        if (!code) return e.json(400, {status:"not_found", message:"Missing code"});
        if (!fp) return e.json(400, {status:"not_found", message:"Missing device fingerprint"});

        // ── Luhn-mod-N check (32-char charset, mirroring the client) ──
        // Same algorithm as activation.pb.js — a malformed code is answered
        // locally without touching the database.
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
        if (!ok) return e.json(200, {status:"not_found", message:"Invalid code format"});

        // ── Rate limiting ──
        // Keeps this from becoming an enumeration oracle.
        //
        // The key is PREFIXED per endpoint ("lookup:" vs "activate:" in
        // activation.pb.js) so the two do NOT share a budget. They previously
        // did, and that was actively harmful: this endpoint exists so a student
        // can find out whether their code is recognised BEFORE committing to a
        // full activation, but mistyping on the activation screen consumed the
        // lookup budget — so the affordance meant to explain the mistake was
        // locked out by the mistake itself. Confirmed live 2026-09-19.
        //
        // Both still share the activation_attempts TABLE and its cleanup, which
        // is fine: the table is just a log, the prefix is the bucket.
        //
        // This endpoint is strictly read-only (never binds, never writes to
        // `codes`), so the risk of a wider budget is bounded, and the threshold
        // below still caps enumeration well under a meaningful rate.
        $app.dao().db().newQuery("DELETE FROM activation_attempts WHERE created < datetime('now','-10 minutes')").execute();
        var rateKey = "lookup_" + (fp || bootstrapId || ip).replace(/[^a-zA-Z0-9]/g,"_");
        // findRecordsByExpr, not findRecordsByFilter — the list variant returns
        // zero rows on this PB build, which silently disabled the rate limit
        // (making this endpoint an unbounded code-enumeration oracle).
        var recentCount = 0;
        try {
            recentCount = $app.dao().findRecordsByExpr("activation_attempts",
                $dbx.exp("rate_key = {:k}", { k: rateKey })).length;
        } catch (countErr) {
            recentCount = 0;
        }
        if (recentCount >= 10) {
            return e.json(429, {status:"not_found", message:"Too many attempts — please wait a few minutes"});
        }
        // Log the attempt. NB: unlike /api/activate we do NOT delete this row
        // afterwards, so lookups are counted too.
        var c2 = $app.dao().findCollectionByNameOrId("activation_attempts");
        var att = new Record(c2);
        att.set("ip", bootstrapId ? "" : ip); att.set("rate_key",rateKey);
        att.set("fingerprint",fp.substring(0,16)+"****");
        att.set("code_attempted",code.substring(0,4)+"****");
        $app.dao().saveRecord(att);

        // ── Canonical-form lookup (identical to activation.pb.js) ──
        var canonical = (s.length === 15)
            ? s.substring(0,2)+"-"+s.substring(2,6)+"-"+s.substring(6,10)+"-"+s.substring(10,14)+"-"+s.substring(14,15)
            : code;
        // IMPORTANT: findFirstRecordByData THROWS "sql: no rows in result set"
        // when nothing matches — it does not return null. Without this catch, a
        // well-formed but unknown code escaped to the outer handler and was
        // reported as HTTP 500 {"status":"unknown"}, i.e. exactly the wrong
        // signal: the client treats 5xx as "cannot tell" and re-checks, instead
        // of telling the student the code is simply not recognised.
        // (Observed live 2026-09-19 against a freshly seeded, empty codes table.)
        var rec = null;
        try {
            rec = $app.dao().findFirstRecordByData("codes", "code", canonical);
        } catch (notFound) {
            rec = null;
        }
        if (!rec) return e.json(200, {status:"not_found", message:"Code not found"});

        // ── Describe the code WITHOUT binding anything ──
        var tier = rec.getString("tier");
        var exp = rec.get("expires_at");
        var resp = {status:"ok", tier:tier};
        if (exp) { resp.expires_at = exp; }

        if (rec.getBool("suspended")) {
            resp.status = "suspended";
            resp.message = "This code has been suspended — contact support";
            return e.json(200, resp);
        }
        if (exp) {
            var ed = parsePBDate(exp);
            if (!isNaN(ed) && ed < Date.now()) {
                resp.status = "expired";
                resp.message = "This code has expired";
                return e.json(200, resp);
            }
        }

        // ── Redeemed already? ──
        //
        // A code is single-use, and `activated_at`/`released_at` are that whole
        // rule (see activation.pb.js). There are two answered cases here and one
        // that is not answered at all:
        //
        //   * A sign-out is waiting (`released_at` later than `activated_at`):
        //     the next activation will consume it, so this really is ready.
        //   * A sign-out was already consumed, or there never was one: the
        //     activation is going to be REFUSED (409), so `ready: true` here
        //     would be a promise this hub does not keep.
        //
        // The second case is why the status vocabulary matters. It shipped as
        // `bound_this_device` with "entering it again will restore your access",
        // which was true only while activation re-issued every redeemed code —
        // and activation doing that is exactly the defect this change removes
        // (one code signing in on any number of machines).
        //
        // WHY `bound_other`, WHICH IS A FROZEN NAME AND NOT A NEW ONE.
        //
        // This shipped as `already_used` and BROKE every deployed client (found
        // live 2026-10-03). A deployed client's `LookupStatus` enum is an
        // ALLOW-LIST with `#[serde(other)] -> Unknown`, and `Unknown` is
        // `ready: false` — so the client showed "Could not check this code right
        // now" and never called /api/activate at all. The hub's activate path was
        // correct; the lookup gate stopped the request before it was made. The old
        // note here claimed an unrecognised status "falls back to the
        // activate-time check", which was WRONG: the fallback is a dead end.
        //
        // `bound_other` is the third entry in the SAME enum, chosen for its
        // MEANING — "this code belongs to another device" is literally what it
        // said during the device-binding era — and every deployed build already
        // decodes it (`ready: false`, with a sentence saying the code is bound to
        // another device). It is a WIRE COMPATIBILITY name: keep the meaning, never
        // re-purpose it. `bound_this_device` is now reserved for the case where the
        // code genuinely is available to this caller (a waiting sign-out), so a
        // deployed build keeps `ready: true` for exactly the codes that will
        // activate.
        var at = rec.get("activated_at");
        if (at !== null && at !== undefined && String(at).trim() !== "") {
            if (releasedSince(at, rec.get("released_at"))) {
                resp.status = "bound_this_device";
                resp.message = "Your code has been signed out and is ready to use here";
                return e.json(200, resp);
            }
            // Said FIRST, and with the SAME comparison the branch above negates.
            // An earlier draft of this change spelled the held case as a bare
            // `if (!releasedSince(...))` and that is indistinguishable, to a
            // scanner and to a reader, from the released case: see
            // `a_redeemed_code_is_refused_on_a_second_device`, whose guard reads
            // the hook, and section (e2b) of check-consistency.sh.
            if (!releasedSince(at, rec.get("released_at"))) {
                resp.status = "bound_other";
                // The message is NOT what gates anything — the status is, and it
                // maps to `ready: false`. This is so the person holding a code
                // that is not theirs reads why before spending an activation
                // attempt, and so the person whose own other machine holds it is
                // told the exact fix.
                resp.message = "This code is already in use on another device. Sign out there first, then enter it here — if this code is not yours, it has already been claimed";
                return e.json(200, resp);
            }
        }

        // A first-use code. Also true when an operator RELEASED a code: that is
        // the pool the code is supposed to go back into, and the release is the
        // deliberate act. `first_seen_at` is what tells the console the
        // difference between "never issued" and "issued once, then released";
        // the student-facing status is the same for both because the code is
        // genuinely available to whoever holds it.
        //
        // And true, in the narrow window between the sign-out and the next
        // activation, for a code a student released — which the sign-out claim
        // above is what makes reachable here at all.
        resp.status = "unbound";
        resp.message = "Ready to activate";
        return e.json(200, resp);
    } catch(err) {
        // Transient/internal failure — the client treats a 5xx as "unknown" and
        // falls back to the activate-time check, so this must not look like a
        // definitive "code not found".
        return e.json(500, {status:"unknown", message:err.message || String(err)});
    }
});
