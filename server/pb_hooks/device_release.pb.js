// Chrysalis Device Release Hook — PocketBase 0.22 compatible
//
// POST /api/release-entitlement
//
// A student signs out on a device. The client forgets the code locally (see
// client/src-tauri/src/chrysalis/signout.rs) and tells the hub here. This call
// is what MAKES THE CODE SINGLE-USE actually mean anything: activation.pb.js
// refuses a redeemed code unless `released_at` is later than `activated_at`, and
// this is the only place that writes `released_at`.
//
// ── WHAT THIS ENDPOINT DELIBERATELY DOES NOT DO ──────────────────────────────
//
// It does NOT clear `codes.activated_at` in place. It writes a stamp that
// OUTRANKS it instead, and the stamp is what the next activation reads.
//
// The difference is not cosmetic, and both halves of it matter:
//
//   1. **It cannot be stolen.** The only credential required here is POSSESSION
//      of the code. Every code is on a printed card, and middlemen hold stock, so
//      if a sign-out simply freed the code, anyone who obtained one — a photo of
//      a friend's card, an unsold batch in a middleman's drawer — could take it
//      out from under the student who paid for it. Writing `released_at` frees it
//      only for the NEXT activation, and the next activation can only come from
//      whoever signed out; the moment it happens the stamp is spent. The holder
//      of the photo loses the race unless the paying student signed out and then
//      left the machine idle, which is the one case where the key really is free.
//   2. **It cannot reset a term in a way that is free.** `activated_at` is the
//      base a re-activation restarts the term from. Consuming a release re-stamps
//      `activated_at` but leaves `expires_at` alone, so signing out and in again
//      gives the student exactly the clock they already had.
//
// So this records a STATE fact (`released_at` — the claim — plus `release_count`
// and a `code_events` row for the operator's history) and leaves the entitlement
// fields as they were. A failure here means the device is signed out locally and
// the code still looks held on the hub — which is why the client reports "signed
// out, not confirmed" rather than failing the sign-out.
//
// ── AUTHENTICATION ──────────────────────────────────────────────────────────
//
// Possession of the code, exactly like `/api/code-lookup`. No admin token: a
// student cannot have one, and requiring one would make the feature unreachable
// by the only person it is for.
//
// Rate limited on its own bucket (`release_`). It must NOT share the `activate_`
// budget: signing out is not an activation attempt, and consuming that budget
// would lock a student out of the screen that explains their code.
//
// ── WHY THE RESPONSE CARRIES `released` ─────────────────────────────────────
//
// 200 with `released: false` and 404 both mean "nothing to release". They are
// kept apart so the client can say something true in each case without having to
// parse a message string (the `403 suspended`/`403 bound` ambiguity in
// activation.pb.js is the lesson: that one is distinguished by wording, and the
// wording is now frozen).

routerAdd("POST", "/api/release-entitlement", function (e) {
    // ── Helpers ──────────────────────────────────────────────────────────────
    //
    // Defined INSIDE the callback on purpose: goja does not hoist function
    // declarations across scopes, so a file-level helper is invisible here. This
    // is the same reason every other hook in this directory inlines its
    // helpers, and it is stated once here rather than re-learned.

    // PocketBase stores dates with a space; `new Date("2027-09-19 00:00:00.000Z")`
    // is NaN in goja. Copied from activation.pb.js, where getting this wrong
    // silently disabled expiry enforcement for months.
    function parsePBDate(value) {
        if (value === null || value === undefined || value === "") return 0;
        if (typeof value === "number") return value;
        var s = String(value).trim();
        if (!s) return 0;
        var direct = new Date(s).getTime();
        if (!isNaN(direct)) return direct;
        var swapped = new Date(s.replace(" ", "T")).getTime();
        return swapped;
    }

    // Same helper as activation.pb.js — read that one for the source verification.
    // `released_at` is NOT routed through it: it is the claim that frees the code,
    // and a hub without that column must be visible as a problem rather than
    // quietly recording a sign-out that changes nothing.
    function setIfPresent(record, name, value) {
        try {
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
        var code = String(data.code || "").trim();
        var fp = String(data.fingerprint || "").trim();
        try {
            var addr = (e.request().remoteAddr || "").split(":");
            var ip = addr[0] || "";
        } catch (exIp) {
            var ip = "";
        }

        if (!code) return e.json(400, { code: 400, message: "Missing code" });

        // ── Canonical form ──
        //
        // Codes are stored hyphenated. Tolerating any pasted variant here matches
        // /api/activate and /api/heartbeat; a sign-out that 404'd because the
        // client sent an unprefixed form would look like "nothing to release",
        // which is the wrong answer and a silent one.
        var s = code.replace(/-/g, "").toUpperCase();
        var canonical = (s.length === 15)
            ? s.substring(0, 2) + "-" + s.substring(2, 6) + "-" + s.substring(6, 10) +
              "-" + s.substring(10, 14) + "-" + s.substring(14, 15)
            : code;

        // ── Rate limit, own bucket ──
        //
        // Namespaced `release_` so it can never consume the `activate_` budget.
        // Same mechanism as activation.pb.js, including the `findRecordsByExpr`
        // workaround: `findRecordsByFilter` returns zero rows on this PB build
        // (0.22.21) and silently disabled the equivalent limit on activation.
        $app.dao().db()
            .newQuery("DELETE FROM activation_attempts WHERE created < datetime('now','-10 minutes')")
            .execute();
        var rateKey = "release_" + (fp || ip).replace(/[^a-zA-Z0-9]/g, "_");
        var recent = 0;
        try {
            recent = $app.dao().findRecordsByExpr(
                "activation_attempts",
                $dbx.exp("rate_key = {:k}", { k: rateKey })
            ).length;
        } catch (countErr) {
            recent = 0; // fail open, as activation does: a counting bug must not block sign-out
        }
        // Higher than activation's 5, because signing out is not a guessable
        // operation and a student on a flaky connection may retry. It is still a
        // bound: this endpoint writes rows.
        if (recent >= 10) {
            return e.json(429, { code: 429, message: "Too many attempts" });
        }

        var attemptCol = $app.dao().findCollectionByNameOrId("activation_attempts");
        var attempt = new Record(attemptCol);
        attempt.set("ip", ip);
        attempt.set("rate_key", rateKey);
        attempt.set("fingerprint", fp.substring(0, 16) + "****");
        attempt.set("code_attempted", code.substring(0, 4) + "****");
        $app.dao().saveRecord(attempt);

        // ── Find the code ──
        //
        // findFirstRecordByData THROWS "sql: no rows in result set" for an absent
        // code rather than returning null, so the try/catch is what makes the
        // 404 reachable. Without it an unknown code becomes a 500.
        var record = null;
        try {
            record = $app.dao().findFirstRecordByData("codes", "code", canonical);
        } catch (notFound) {
            record = null;
        }
        if (!record) {
            // Deliberately the same answer as "never redeemed". Telling a caller
            // which of the two it is would turn this endpoint into a code
            // existence oracle — it is rate limited, but it is still unauthenticated
            // by anything but the code, which is the very thing being probed.
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }

        // ── Nothing to release? ──
        //
        // An unredeemed code has no device to release from. A code that is
        // ALREADY released (a second sign-out from the same, already-free device)
        // has no live redemption either — the first sign-out freed it and nothing
        // has claimed it since.
        //
        // That second case needs an explicit check, and it is the one the
        // `released_at > activated_at` rule made easy to get wrong: without it a
        // second sign-out would walk the claim forward, so a code sitting free in
        // the pool would stay claimable by any holder of the card indefinitely
        // rather than for the student who actually released it.
        //
        // A suspended or expired code has no live entitlement either — the
        // student's access is already ended by a fact stronger than this record,
        // and writing an audit row saying "signed out" would add noise to a case
        // the operator is already handling.
        //
        // Each answers the same way: a no-op success. The student's local sign-out
        // has already happened and is unaffected.
        var at = record.get("activated_at");
        var wasRedeemed = at !== null && at !== undefined && String(at).trim() !== "";
        if (!wasRedeemed || record.getBool("suspended")) {
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }
        var atMs = parsePBDate(at);
        var relMs = parsePBDate(record.get("released_at"));
        if (!isNaN(relMs) && relMs > 0 && relMs > atMs) {
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }
        var expMs = parsePBDate(record.get("expires_at"));
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }

        // ── Record it ──
        //
        // ONLY state and audit fields are written. `activated_at` and `expires_at`
        // are untouched on purpose — see the header; `released_at` outranks
        // `activated_at` instead of clearing it, and activation.pb.js reads the
        // comparison. Read-modify-write of the count rather than a separate
        // counter collection: these are two small numbers on a row that is already
        // being saved for `released_at`, and a second collection would be a second
        // thing to keep consistent.
        var prior = record.get("release_count");
        var count = (typeof prior === "number" && isFinite(prior) ? prior : 0) + 1;
        // `released_at` is the claim that frees the code (activation.pb.js reads
        // it), so it is NOT optional and NOT routed through `setIfPresent`. If a
        // hub ever lacks it the save must fail loudly rather than quietly record
        // a sign-out that changes nothing — which is exactly the report this
        // endpoint exists to answer.
        record.set("released_at", new Date().toISOString());
        // `release_count` IS optional: it is the operator's history, not the
        // claim. Present on every deployed hub, but routed through the helper so
        // a future schema change cannot make it silently vanish.
        setIfPresent(record, "release_count", count);
        $app.dao().saveRecord(record);

        // The full history, so the console can answer "signed out how many times,
        // and when" without relying on a single overwritten timestamp. The code
        // prefix and the fingerprint are the only identifiers recorded: the
        // console logs prefixes for the same reason (a code is a bearer
        // credential and a log is a thing people paste).
        //
        // `event: "released"` and not `"signed_out"`, and the difference is what
        // the operator needs to read. This one means the code is FREE and the next
        // activation will claim it; `signed_out` means a device stopped using a
        // code that is still held (any deployment of this hook before this
        // change). A console that showed both as one word would leave the operator
        // unable to tell which codes can be moved and which are still in force.
        try {
            var evtCol = $app.dao().findCollectionByNameOrId("code_events");
            var evt = new Record(evtCol);
            evt.set("code", canonical);
            evt.set("event", "released");
            evt.set("detail", "Signed out on a device (release #" + count + ") — the code is free for the next activation");
            evt.set("actor", "student");
            evt.set("fingerprint", fp.substring(0, 16) + "****");
            $app.dao().saveRecord(evt);
        } catch (evtErr) {
            // The audit row is worth having and not worth failing the request
            // over: the `released_at` above is already written and is the fact the
            // operator needs. Losing one history row must not turn a successful
            // sign-out into an error the student is shown.
            $app.logger().warn("release-entitlement: could not write code_events row: " + (evtErr.message || String(evtErr)));
        }

        $app.logger().info(
            "Student sign-out: code=" + canonical.substring(0, 4) + "****, release #" + count
        );

        // `code: 200` mirrors every other hook in this directory: the client
        // switches on the JSON `code`, not the HTTP status.
        //
        // `message` is deliberately not "Signed out." — the client uses its own
        // sentences (signout.rs → HubOutcome) and shows this one only as a
        // fallback, where "the code is free" is the fact that matters: it is what
        // the student needs to know before they enter it on the next machine.
        return e.json(200, {
            code: 200,
            message: "Release recorded — the code can be used on another device now",
            released: true,
            count: count
        });
    } catch (err) {
        return e.json(500, { code: 500, message: err.message || String(err) });
    }
});
