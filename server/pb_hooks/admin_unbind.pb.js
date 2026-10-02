// Locus Admin Unbind Hook — PocketBase 0.22 compatible
//
// Auth is BODY-ONLY (`admin_token` in the JSON body). The sibling
// admin_console.pb.js also accepts an `X-Admin-Token` header; this endpoint does
// not, deliberately — it is the legacy SSH-era endpoint, the console no longer
// calls it, and a second auth shape here would only widen the surface. If a
// caller needs it, use /api/admin/console.
routerAdd("POST", "/api/admin/unbind-code", function(e) {
    // Inline helpers only: file-scope declarations are not visible inside
    // routerAdd callbacks in this PocketBase build (see FIXES.md 2026-09-19).
    function constantTimeEquals(a, b) {
        var x = String(a || ""), y = String(b || "");
        if (x.length !== y.length) return false;
        var diff = 0;
        for (var ci = 0; ci < x.length; ci++) {
            diff |= (x.charCodeAt(ci) ^ y.charCodeAt(ci));
        }
        return diff === 0;
    }

    try {
        var body = $apis.requestInfo(e).data;
        var adminToken = (body.admin_token || "").trim();
        var code = (body.code || "").trim();
        var reason = (body.reason || "Requested by admin").trim();

        // ADMIN_API_TOKEN must be set in /etc/environment on the VPS
        var validToken = $os.getenv("ADMIN_API_TOKEN") || "";
        if (!validToken) return e.json(500, {code:500, message:"ADMIN_API_TOKEN not configured"});
        // No early-exit compare: see the note in admin_console.pb.js. goja has no
        // crypto.timingSafeEqual, so this is the closest available.
        if (!constantTimeEquals(adminToken, validToken)) {
            return e.json(403, {code:403, message:"Invalid admin token"});
        }
        if (!code) return e.json(400, {code:400, message:"Missing code"});

        // NOTE: findFirstRecordByData THROWS "sql: no rows in result set" when the
        // code is absent, so the null-check below needs the try/catch to be
        // reachable (otherwise an unknown code becomes a 500, not a 404).
        var record = null;
        try {
            record = $app.dao().findFirstRecordByData("codes", "code", code);
        } catch (notFound) {
            record = null;
        }
        if (!record) return e.json(404, {code:404, message:"Code not found"});

        // ── What "unbind" means now ──
        //
        // There is no device binding any more (see activation.pb.js): a code is
        // either unused or redeemed, and the only record of that is
        // `activated_at`. So this endpoint's job is to RELEASE the code — put it
        // back to unused so it can be handed to a different student, or so a
        // student who is moving to a new machine can redeem it again.
        //
        // The appeal is unchanged; only the mechanism is simpler. It used to have
        // to clear a fingerprint AND repair a `device_bindings` index row, and a
        // mismatch between the two was a real source of "one code per device"
        // silently not applying.
        var wasRedeemed = record.get("activated_at") !== null && String(record.get("activated_at")).trim() !== "";
        if (!wasRedeemed) return e.json(400, {code:400, message:"Code has not been used yet"});

        // Log the release for audit. No fingerprint to name — that is the point
        // of the change — so the code prefix and the reason are the record.
        $app.logger().info("Admin release: code=" + code.substring(0,4) + "****, reason=" + reason);

        // Release the code: clear the redeemed stamp so a fresh activation is a
        // first redemption again. The tier and expiry are untouched — releasing a
        // code must not change what it entitles its holder to.
        record.set("activated_at", null);
        record.set("unbound_at", new Date().toISOString());
        record.set("unbind_reason", reason);
        $app.dao().saveRecord(record);

        return e.json(200, {
            code: 200,
            message: "Code released successfully.",
            tier: record.getString("tier"),
            middleman: record.getString("middleman") || ""
        });
    } catch(err) {
        return e.json(500, {code:500, message:err.message || String(err)});
    }
});
