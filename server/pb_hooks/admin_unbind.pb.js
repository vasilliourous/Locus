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

        var boundFp = record.getString("bound_fingerprint");
        if (!boundFp) return e.json(400, {code:400, message:"Code is not bound to any device"});

        // Log the unbind for audit
        var auditFp = boundFp.substring(0, 8) + "****";
        $app.logger().info("Admin unbind: code=" + code.substring(0,4) + "****, old_fingerprint=" + auditFp + ", reason=" + reason);

        // Clear the binding
        record.set("bound_fingerprint", "");
        record.set("activated_at", null);
        record.set("unbound_at", new Date().toISOString());
        record.set("unbind_reason", reason);
        $app.dao().saveRecord(record);

        return e.json(200, {
            code: 200,
            message: "Code unbound successfully.",
            tier: record.getString("tier"),
            middleman: record.getString("middleman") || ""
        });
    } catch(err) {
        return e.json(500, {code:500, message:err.message || String(err)});
    }
});
