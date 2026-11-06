// Locus Device Release Hook — PocketBase 0.22 compatible
//
// POST /api/release-entitlement
//
// A student signs out on a device. The client forgets the code locally (see
// client/src-tauri/src/locus/signout.rs) and tells the hub here, so the
// operator can tell "this student moved to a new computer" apart from "this key
// is being shared". From the console those two look identical otherwise.
//
// ── WHAT THIS ENDPOINT DELIBERATELY DOES NOT DO ──────────────────────────────
//
// It does NOT clear `codes.activated_at`.
//
// `/api/admin/unbind-code` does clear it, and that is correct FOR AN OPERATOR:
// releasing a code puts it back to unused so it can be handed to someone else.
//
// This endpoint must not, for two reasons that both bite:
//
//   1. `activated_at` is the base a re-activation restarts the term from
//      (activation.pb.js → `expiryFromTerm(termDays, now)`). Clearing it lets a
//      student reset their own clock by signing out and in again.
//   2. The only credential required here is POSSESSION of the code. Every code
//      is on a printed card, and middlemen hold stock. If a sign-out cleared the
//      redemption, anyone who obtained a code — a photo of a friend's card, an
//      unsold batch in a middleman's drawer — could release it out from under
//      the student who paid for it. That is a griefing vector against paying
//      customers, and it would be created by a convenience feature.
//
// So this records an AUDIT fact (`released_at`, `release_count`, a `code_events`
// row) and leaves every entitlement field exactly as it was. Re-activation was
// ALREADY allowed for a redeemed code ("Already activated" restores access), so
// nothing about the student's next device depends on this call succeeding —
// which is why the client treats a failure here as "signed out, not confirmed"
// rather than failing the sign-out.
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
        // An unredeemed code has no device to release from. A suspended or
        // expired code has no live entitlement either — the student's access is
        // already ended by a fact stronger than this record, and writing an audit
        // row saying "signed out" would add noise to a case the operator is
        // already handling.
        //
        // All three answer the same way: a no-op success. The student's local
        // sign-out has already happened and is unaffected.
        var at = record.get("activated_at");
        var wasRedeemed = at !== null && at !== undefined && String(at).trim() !== "";
        if (!wasRedeemed || record.getBool("suspended")) {
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }
        var expMs = parsePBDate(record.get("expires_at"));
        if (!isNaN(expMs) && expMs > 0 && expMs < Date.now()) {
            return e.json(404, { code: 404, message: "Nothing to release", released: false });
        }

        // ── Record it ──
        //
        // ONLY audit fields are written. `activated_at` and `expires_at` are
        // untouched on purpose — see the header. Read-modify-write of the count
        // rather than a separate counter collection: these are two small numbers
        // on a row that is already being saved for `released_at`, and a second
        // collection would be a second thing to keep consistent.
        var prior = record.get("release_count");
        var count = (typeof prior === "number" && isFinite(prior) ? prior : 0) + 1;
        record.set("released_at", new Date().toISOString());
        record.set("release_count", count);
        $app.dao().saveRecord(record);

        // The full history, so the console can answer "signed out how many times,
        // and when" without relying on a single overwritten timestamp. The code
        // prefix and the fingerprint are the only identifiers recorded: the
        // console logs prefixes for the same reason (a code is a bearer
        // credential and a log is a thing people paste).
        try {
            var evtCol = $app.dao().findCollectionByNameOrId("code_events");
            var evt = new Record(evtCol);
            evt.set("code", canonical);
            evt.set("event", "signed_out");
            evt.set("detail", "Signed out on a device (release #" + count + ")");
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
        return e.json(200, {
            code: 200,
            message: "Release recorded",
            released: true,
            count: count
        });
    } catch (err) {
        return e.json(500, { code: 500, message: err.message || String(err) });
    }
});
