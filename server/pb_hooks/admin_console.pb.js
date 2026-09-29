// Locus Admin API — server-side surface for the web console.
//
// WHY THIS EXISTS: managing the hub previously required SSH + Python + sqlite.
// That is fine for an engineer and hopeless day-to-day. This hook exposes the
// operations an operator actually performs as a small JSON API, so the console
// never needs shell access.
//
// Auth: every route requires the ADMIN_API_TOKEN (from /etc/environment) in the
// JSON body as `admin_token`, or as `X-Admin-Token`. The console holds the token
// in memory only — it is never embedded in the served bundle.
//
// CONVENTIONS (learned the hard way — see FIXES.md 2026-09-19):
//   * Use $app.dao().findFirstRecordByFilter / findRecordsByExpr.
//     findRecordsByFilter returns ZERO rows on this PocketBase build with no
//     error, which silently breaks anything built on it.
//   * Helper functions must be declared INSIDE the handler — file-scope
//     declarations are not visible to routerAdd callbacks.
//   * findFirstRecordByData THROWS (rather than returning null) when absent.

routerAdd("POST", "/api/admin/console", function(e) {
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

    // ── Inline helpers (file scope is NOT visible here) ──
    function ok(data) {
        var out = {ok: true};
        if (data) { for (var k in data) out[k] = data[k]; }
        return e.json(200, out);
    }
    function bad(status, message) { return e.json(status, {ok: false, message: message}); }
    function nowISO() { return new Date().toISOString(); }

    // Compares two secrets without an early exit on the first differing byte.
    //
    // Declared INSIDE the handler like every other helper here — file-scope
    // declarations are not visible to routerAdd callbacks in this PocketBase
    // build (the "helperFn is not defined" trap, see FIXES.md 2026-09-19).
    //
    // goja has no `crypto.timingSafeEqual`. The length check does leak the
    // token length, which is acceptable: it is a fixed-format high-entropy
    // secret, not a user password, and this endpoint is rate-limited. The point
    // is to avoid leaking *how many leading characters* matched.
    function constantTimeEquals(a, b) {
        var x = String(a || ""), y = String(b || "");
        if (x.length !== y.length) return false;
        var diff = 0;
        for (var ci = 0; ci < x.length; ci++) {
            diff |= (x.charCodeAt(ci) ^ y.charCodeAt(ci));
        }
        return diff === 0;
    }

    // Whether a value is plausibly a `signature_<platform>` field in the wire
    // format the client's updater expects: base64(ENTIRE .sig file text).
    //
    // The plugin runs `base64_to_string()` over this field BEFORE parsing the
    // result as minisign text, so the stored value must be base64 of the
    // four-line .sig file — not the file text itself. Storing raw text passes a
    // mere presence check, is advertised to every client, and then fails at
    // install time with `Invalid byte …, offset N` from the base64 decoder.
    //
    // Declared at the top of the handler, alongside the other helpers, because
    // BOTH releases.publish and releases.set must agree on this rule: the
    // publish path writes the field and the set path activates it, and a
    // release that one accepts while the other rejects is exactly how a broken
    // row reached production. File-scope is still NOT visible here (goja does
    // not hoist across scopes) — this is handler scope, shared by every action.
    //
    // It cannot VERIFY the signature (this hook has no public key, and should
    // not hold one); it catches a wrong-format or truncated field only.
    function isPlausibleMinisignSignature(value) {
        var s = String(value || "").trim();
        if (!s) return false;
        // Must be base64 — the .sig file's spaces and newlines are excluded by
        // this alphabet, which is what rejects raw-text values.
        if (!/^[A-Za-z0-9+/]+=*$/.test(s)) return false;
        // Long enough to hold minisign's four lines (~250-350 chars pre-wrap).
        if (s.length < 200) return false;
        try {
            var decoded = $os ? atob(s) : "";
            if (!decoded) return false;
            var lines = decoded.split("\n");
            var nonEmpty = [];
            for (var li = 0; li < lines.length; li++) {
                if (lines[li].replace(/\s/g, "")) nonEmpty.push(lines[li]);
            }
            if (nonEmpty.length < 4) return false;
            return nonEmpty[0].indexOf("untrusted comment: ") === 0 &&
                   nonEmpty[2].indexOf("trusted comment: ") === 0;
        } catch (decErr) {
            return false;
        }
    }

    var CHARSET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    var N = CHARSET.length;

    // Luhn-mod-N checksum over the full body INCLUDING the "RQ" prefix.
    // Must stay byte-identical to the client's luhn.go and the sibling hooks,
    // or generated codes fail validation on the device.
    function checksum(body) {
        var sum = 0, alt = false;
        for (var i = body.length - 1; i >= 0; i--) {
            var v = CHARSET.indexOf(body[i]);
            if (v < 0) return null;
            if (alt) { v = v * 2; if (v >= N) v = v - N + 1; }
            sum += v;
            alt = !alt;
        }
        return CHARSET[(N - (sum % N)) % N];
    }
    function formatCode(body) {
        // body = "RQ" + 12 chars -> RQ-XXXX-XXXX-XXXX-C
        return body.substring(0,2) + "-" + body.substring(2,6) + "-" +
               body.substring(6,10) + "-" + body.substring(10,14) + "-" +
               checksum(body);
    }
    function randomBody() {
        var s = "RQ";
        for (var i = 0; i < 12; i++) {
            s += CHARSET[Math.floor(Math.random() * N)];
        }
        return s;
    }
    function canonical(input, lengthOfBody) {
        // Accepts any pasted variant and returns the stored hyphenated form.
        var s = String(input || "").replace(/-/g, "").toUpperCase();
        if (s.length !== 15) return input;
        return s.substring(0,2) + "-" + s.substring(2,6) + "-" + s.substring(6,10) +
               "-" + s.substring(10,14) + "-" + s.substring(14,15);
    }
    function logEvent(code, event, detail, fingerprint) {
        try {
            var coll = $app.dao().findCollectionByNameOrId("code_events");
            var rec = new Record(coll);
            rec.set("code", code);
            rec.set("event", event);
            rec.set("detail", detail || "");
            rec.set("actor", "console");
            rec.set("fingerprint", fingerprint || "");
            $app.dao().saveRecord(rec);
        } catch (evErr) { /* audit is best-effort; never fail the action for it */ }
    }

    // ── Device-binding index ──
    //
    // The `device_bindings` collection's `fingerprint` field is UNIQUE, so
    // "one code per device" is enforced by the schema rather than by every hook
    // remembering to check. These helpers maintain the index; the constraint
    // itself is what makes the rule hold.
    //
    // ALL OF THESE ARE BEST-EFFORT. The entitlement lives on the code row, and
    // failing an operator action because an index write failed would be worse
    // than an index that is briefly behind.
    //
    // A released row is KEPT (released_at set) rather than deleted, so "has
    // this device ever been bound, and to what" stays answerable — the same
    // append-only reasoning as code_events.
    //
    // Fingerprints are interpolated into the filter string because no caller in
    // this tree passes findFirstRecordByFilter a parameter object, and they are
    // alphanumeric by construction (a hex digest), so stripping is enough to
    // make the interpolation safe.
    function bindingRow(fp) {
        var safe = String(fp || "").replace(/[^a-zA-Z0-9]/g, "");
        if (!safe) return null;
        try {
            return $app.dao().findFirstRecordByFilter("device_bindings",
                "fingerprint = '" + safe + "'");
        } catch (none) { return null; }
    }
    function releaseBinding(fp, reason) {
        if (!fp) return;
        try {
            var row = bindingRow(fp);
            if (!row) return;
            row.set("released_at", nowISO());
            row.set("release_reason", reason || "");
            $app.dao().saveRecord(row);
        } catch (relErr) { /* best-effort */ }
    }
    function claimBinding(fp, code, tier) {
        if (!fp) return;
        try {
            var row = bindingRow(fp);
            if (row) {
                row.set("code", code);
                row.set("tier", tier || "");
                row.set("bound_at", nowISO());
                row.set("released_at", null);
                row.set("release_reason", "");
                $app.dao().saveRecord(row);
                return;
            }
            var coll = $app.dao().findCollectionByNameOrId("device_bindings");
            var rec = new Record(coll);
            rec.set("fingerprint", String(fp).replace(/[^a-zA-Z0-9]/g, ""));
            rec.set("code", code);
            rec.set("tier", tier || "");
            rec.set("bound_at", nowISO());
            $app.dao().saveRecord(rec);
        } catch (claimErr) { /* best-effort */ }
    }

    try {
        var body = $apis.requestInfo(e).data;
        var supplied = String(body.admin_token || "").trim();
        if (!supplied) {
            // The header is read from the REQUEST, not from `e`.
            //
            // `e.requestInfo().data` carries only the request BODY — PocketBase
            // copies body keys into the key-value store, and header names like
            // `X-Admin-Token` are not valid identifiers and are never copied. So
            // the previous `e.request().header.get(...)` read an empty store and
            // this fallback had NEVER run; it only appeared to work because the
            // console also sends the token in the body.
            //
            // `$apis.requestInfo(e).headers` is the documented accessor for
            // request headers in PocketBase 0.22+, and header lookup is
            // case-insensitive. Kept as a fallback because a curl or CI caller
            // passing only the header is a supported shape — and one the
            // fetch-release service (fetch-release.py) honours properly, so the
            // two admin surfaces must agree.
            try {
                var headers = $apis.requestInfo(e).headers || {};
                supplied = String(headers["x-admin-token"] || headers["X-Admin-Token"] || "").trim();
            } catch (hErr) {
                supplied = "";
            }
        }
        var validToken = $os.getenv("ADMIN_API_TOKEN") || "";
        if (!validToken) return bad(500, "ADMIN_API_TOKEN is not configured on the server");
        // Length-checked, per-char XOR accumulate. goja exposes no
        // `crypto.timingSafeEqual`, so this is the closest available: it removes
        // the early-exit a plain `!==` gives a timing attacker. It is not a
        // rigorous constant-time guarantee (V8 may still vary per comparison),
        // and it is defence in depth — the token is a high-entropy secret, not a
        // password, and it only ever travels over TLS to a rate-limited endpoint.
        if (!constantTimeEquals(supplied, validToken)) return bad(403, "Invalid admin token");

        var action = String(body.action || "");

        // ─────────────────────────────────────────────────────────────
        // dashboard: counts and recent activity
        // ─────────────────────────────────────────────────────────────
        if (action === "dashboard") {
            var allCodes = $app.dao().findRecordsByExpr("codes", $dbx.exp("id != ''"));
            var bound = 0, suspended = 0, expired = 0, available = 0;
            var byTier = {};
            var expiringSoon = 0;
            var now = Date.now();
            var in30 = now + (30 * 24 * 3600 * 1000);

            for (var i = 0; i < allCodes.length; i++) {
                var c = allCodes[i];
                var t = c.getString("tier") || "unknown";
                byTier[t] = (byTier[t] || 0) + 1;
                var isSusp = c.getBool("suspended");
                var exp = c.get("expires_at");
                var expMs = parsePBDate(exp);
                var isExp = expMs && !isNaN(expMs) && expMs < now;
                if (isSusp) suspended++;
                else if (isExp) expired++;
                else if (c.getString("bound_fingerprint")) bound++;
                else available++;
                if (expMs && !isNaN(expMs) && expMs > now && expMs < in30) expiringSoon++;
            }

            var attempts = $app.dao().findRecordsByExpr("activation_attempts", $dbx.exp("id != ''"));
            var recent = $app.dao().findRecordsByExpr("code_events", $dbx.exp("id != ''"));

            // Newest first, capped — the console only shows a short feed.
            recent.sort(function(a, b) {
                return String(b.get("created") || "").localeCompare(String(a.get("created") || ""));
            });
            var feed = [];
            for (var r = 0; r < recent.length && r < 15; r++) {
                feed.push({
                    code: recent[r].getString("code"),
                    event: recent[r].getString("event"),
                    detail: recent[r].getString("detail"),
                    created: recent[r].getString("created"),
                });
            }

            return ok({
                codes: {
                    total: allCodes.length,
                    available: available,
                    bound: bound,
                    suspended: suspended,
                    expired: expired,
                    byTier: byTier,
                    expiringSoon: expiringSoon,
                },
                attempts: attempts.length,
                recent: feed,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.list: searchable/filterable listing
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.list") {
            var q = String(body.query || "").trim().toUpperCase().replace(/-/g, "");
            var tierFilter = String(body.tier || "").trim();
            var statusFilter = String(body.status || "").trim();
            var mmFilter = String(body.middleman || "").trim().toLowerCase();

            // The renewal worklist: only codes whose term ends within N days.
            //
            // WHY THIS FILTER EXISTS. Renewal is an operator action and nothing
            // in the system prompts one — the hub holds no identity, so it
            // cannot know a payment is due, only that a date is approaching.
            // That makes this queue the revenue mechanism rather than a
            // convenience: if nobody looks at it, students lapse silently and
            // no money is collected.
            //
            // `days_until_expiry` is 0/absent = no window (every code). A
            // negative value is refused because it would be meaningless.
            //
            // Codes with NO expiry never appear in a window, which is correct:
            // a permanent code has nothing to renew.
            var windowDays = Number(body.expiring_within_days);
            var useWindow = isFinite(windowDays) && windowDays > 0;
            if (useWindow && windowDays > 3650) {
                return bad(400, "expiring_within_days must be 3650 (10 years) or less");
            }
            // `nowMs` is declared below (it was already needed for the status
            // computation); the window reuses it rather than reading the clock
            // twice, so a code cannot be classified against two different
            // instants within one request.
            var recs = $app.dao().findRecordsByExpr("codes", $dbx.exp("id != ''"));
            var nowMs = Date.now();
            var windowEndMs = useWindow ? (nowMs + (windowDays * 24 * 60 * 60 * 1000)) : 0;
            // Suspended codes are excluded from the worklist: the operator
            // deliberately cut that student off, and prompting a renewal for
            // them would be asking to undo their own decision.
            var excludeSuspended = useWindow;

            var out = [];
            for (var j = 0; j < recs.length; j++) {
                var rec = recs[j];
                var codeVal = rec.getString("code");
                var fp = rec.getString("bound_fingerprint");
                var expRaw = rec.get("expires_at");
                var expMs2 = parsePBDate(expRaw);
                var isExp2 = expMs2 && !isNaN(expMs2) && expMs2 < nowMs;
                var susp2 = rec.getBool("suspended");

                var status = susp2 ? "suspended"
                           : isExp2 ? "expired"
                           : fp ? "bound"
                           : "available";

                // Whether the binding index knows about this code's device.
                //
                // Reported per row so the console can flag a DISAGREEMENT
                // between the code (`bound_fingerprint` set) and the index (no
                // live row). That state is reachable two ways: a code bound
                // before the index existed, or an index row an operator
                // released while the code still holds a fingerprint. Either way
                // it means the "one code per device" rule cannot see this
                // binding, which is worth showing rather than discovering later.
                //
                // Guarded by `fp`: an unbound code has no binding to check, and
                // looking one up per row would be a pointless scan.
                var indexLive = false;
                if (fp) {
                    try {
                        var idxRows = $app.dao().findRecordsByExpr("device_bindings",
                            $dbx.exp("fingerprint = {:f}", { f: String(fp).replace(/[^a-zA-Z0-9]/g, "") }));
                        for (var ix = 0; ix < idxRows.length; ix++) {
                            if (!idxRows[ix].getString("released_at") &&
                                idxRows[ix].getString("code") === codeVal) {
                                indexLive = true;
                                break;
                            }
                        }
                    } catch (idxErr) { indexLive = false; }
                }

                // The window filter. A code must have a parsable expiry in the
                // FUTURE and within the window — so already-lapsed codes are
                // absent (they are a different problem: the student has already
                // lost access and needs a catch-up, not a reminder).
                if (useWindow) {
                    if (excludeSuspended && susp2) continue;
                    if (!expMs2 || isNaN(expMs2)) continue;
                    if (expMs2 <= nowMs) continue;
                    if (expMs2 > windowEndMs) continue;
                }

                // The query matches the CODE, or the operator's NAME for the
                // student (`label`), or `notes`, or the middleman.
                //
                // It used to match the code string only, which made the normal
                // support call impossible: an operator holding a student's name
                // had no way to find their code, so every renewal began with
                // guessing which code was whose. `label` and `notes` were
                // already returned in every row — they were simply never
                // searched.
                //
                // `q` arrives upper-cased and hyphen-stripped (see the header),
                // so the code comparison stays as it was. The free-text fields
                // are matched case-insensitively against the SAME normalised
                // needle: label/notes are typed by hand, so case must not
                // matter, and stripping hyphens matches nothing extra there.
                if (q && codeVal.toUpperCase().replace(/-/g, "").indexOf(q) === -1) {
                    var hay = (String(rec.getString("label") || "") + " " +
                               String(rec.getString("notes") || "") + " " +
                               String(rec.getString("middleman") || ""))
                              .toUpperCase().replace(/-/g, "");
                    if (hay.indexOf(q) === -1) continue;
                }
                if (tierFilter && rec.getString("tier") !== tierFilter) continue;
                if (statusFilter && status !== statusFilter) continue;
                if (mmFilter && String(rec.getString("middleman") || "").toLowerCase().indexOf(mmFilter) === -1) continue;

                out.push({
                    id: rec.id,
                    code: codeVal,
                    tier: rec.getString("tier"),
                    status: status,
                    bound: !!fp,
                    // Truncated for display. See `fingerprint_full` below.
                    fingerprint: fp ? fp.substring(0, 12) + "…" : "",
                    // The untruncated value, so the console can pass it to
                    // `device.get` without reconstructing it (and without
                    // guessing at an ellipsis). This is a device identifier the
                    // client itself put on the wire on every heartbeat, not a
                    // secret — but it is still shown truncated in the UI, and
                    // the full value is only ever used to look up a binding.
                    fingerprint_full: fp || "",
                    suspended: susp2,
                    expires_at: expRaw ? String(expRaw) : "",
                    activated_at: rec.getString("activated_at") || "",
                    middleman: rec.getString("middleman") || "",
                    label: rec.getString("label") || "",
                    notes: rec.getString("notes") || "",
                    // The term, so the console can show what a renewal will add
                    // and pre-fill the renew prompt. 0/absent means the code
                    // never expires.
                    term_days: rec.get("term_days") || 0,
                    term_kind: rec.getString("term_kind") || "",
                    // Whole days until expiry, rounded UP, or null when there
                    // is no parsable expiry. The worklist sorts by this and
                    // shows it, so the operator reads "3 days" rather than
                    // doing date arithmetic — the same reason the client shows
                    // days rather than a raw date.
                    //
                    // Rounded up so "expires tomorrow" is 1, not 0: a 0 that
                    // means "less than 24 hours" would be read as "today".
                    days_remaining: (function() {
                        if (!expMs2 || isNaN(expMs2)) return null;
                        if (expMs2 <= nowMs) return 0;
                        return Math.ceil((expMs2 - nowMs) / (24 * 60 * 60 * 1000));
                    })(),
                    // True when the code holds a fingerprint the binding index
                    // cannot see. See the note above: a code bound before the
                    // index existed, or a released index row.
                    binding_unindexed: !!fp && !indexLive,
                });
            }
            // Sort order depends on what the caller is looking at.
            //
            // A WORKLIST is sorted by urgency — soonest expiry first — because
            // the operator is working through "who do I need to chase" and the
            // top of the list should be the most urgent, not the most recent.
            // Every other view keeps most-recently-activated first, which is
            // what it has always done.
            if (useWindow) {
                out.sort(function(a, b) {
                    var da = (a.days_remaining === null) ? 1e9 : a.days_remaining;
                    var db = (b.days_remaining === null) ? 1e9 : b.days_remaining;
                    if (da !== db) return da - db;
                    return (b.activated_at || "").localeCompare(a.activated_at || "");
                });
            } else {
                // Most recent first.
                out.sort(function(a, b) { return (b.activated_at || "").localeCompare(a.activated_at || ""); });
            }
            return ok({
                total: out.length,
                codes: out,
                // Echoed so the console can label the view without re-deriving
                // it, and so a caller that passed a window can tell it was
                // applied.
                expiring_within_days: useWindow ? windowDays : 0,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.generate
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.generate") {
            var count = parseInt(body.count || "1", 10);
            if (isNaN(count) || count < 1 || count > 500) return bad(400, "count must be between 1 and 500");
            var tier = String(body.tier || "").trim();
            if (!tier) return bad(400, "tier is required");
            var expires = String(body.expires_at || "").trim();
            var middleman = String(body.middleman || "").trim();
            var label = String(body.label || "").trim();
            var notes = String(body.notes || "").trim();

            // The term, in days. Preferred over `expires_at` for new stock, and
            // the two are not both needed: a term starts its clock at
            // ACTIVATION, so the card does not decay on the shelf, whereas an
            // absolute `expires_at` start its clock at MINT.
            //
            // `expires_at` is kept working because it is how goodwill
            // extensions and edge cases are expressed, and because existing
            // batches were minted that way. A code may carry both; if it does,
            // the term recomputes the expiry at activation, since that is the
            // moment the student's clock starts.
            var termDays = Number(body.term_days);
            if (String(body.term_days || "") === "" || !isFinite(termDays) || termDays < 0) {
                termDays = 0; // 0 = no term = never expires
            } else {
                termDays = Math.floor(termDays);
            }
            var termKind = String(body.term_kind || "").trim();

            var collC = $app.dao().findCollectionByNameOrId("codes");
            var created = [], skipped = 0;
            for (var k = 0; k < count; k++) {
                var made = null;
                // Retry on the (astronomically unlikely) collision or any
                // uniqueness error, rather than aborting the whole batch.
                for (var attempt = 0; attempt < 5 && !made; attempt++) {
                    var codeStr = formatCode(randomBody());
                    var exists = null;
                    try { exists = $app.dao().findFirstRecordByData("codes", "code", codeStr); } catch (nf) { exists = null; }
                    if (exists) continue;
                    try {
                        var nr = new Record(collC);
                        nr.set("code", codeStr);
                        nr.set("tier", tier);
                        nr.set("used", false);
                        nr.set("suspended", false);
                        nr.set("bound_fingerprint", "");
                        nr.set("middleman", middleman);
                        nr.set("label", label);
                        nr.set("notes", notes);
                        if (termDays > 0) nr.set("term_days", termDays);
                        if (termKind) nr.set("term_kind", termKind);
                        if (expires) nr.set("expires_at", expires);
                        $app.dao().saveRecord(nr);
                        made = codeStr;
                        created.push(codeStr);
                    } catch (saveErr) {
                        skipped++;
                    }
                }
                if (!made) skipped++;
            }
            logEvent(created.length ? created[0] : "(batch)", "generated",
                     created.length + " x " + tier +
                     (termDays > 0 ? " (" + termDays + "d term)" : " (no term)") +
                     (middleman ? " for " + middleman : ""), "");
            return ok({created: created, skipped: skipped, tier: tier, term_days: termDays});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.suspend / codes.unsuspend
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.suspend" || action === "codes.unsuspend") {
            var target = canonical(body.code);
            if (!target) return bad(400, "code is required");
            var recS = null;
            try { recS = $app.dao().findFirstRecordByData("codes", "code", target); } catch (nf2) { recS = null; }
            if (!recS) return bad(404, "Code not found");
            var wantSuspend = action === "codes.suspend";
            recS.set("suspended", wantSuspend);
            $app.dao().saveRecord(recS);
            logEvent(target, wantSuspend ? "suspended" : "unsuspended",
                     String(body.reason || ""), "");
            return ok({code: target, suspended: wantSuspend});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.unbind — release the device binding
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.unbind") {
            var targetU = canonical(body.code);
            if (!targetU) return bad(400, "code is required");
            var recU = null;
            try { recU = $app.dao().findFirstRecordByData("codes", "code", targetU); } catch (nf3) { recU = null; }
            if (!recU) return bad(404, "Code not found");
            var oldFp = recU.getString("bound_fingerprint");
            if (!oldFp) return bad(400, "That code is not bound to a device");
            var reason = String(body.reason || "Unbound from console").trim();
            recU.set("bound_fingerprint", "");
            recU.set("activated_at", null);
            // These two columns previously did not exist, so the audit trail
            // recorded nothing. They are part of the schema now.
            recU.set("unbound_at", nowISO());
            recU.set("unbind_reason", reason);
            $app.dao().saveRecord(recU);
            // Free the device in the binding index, or it could never activate
            // another code — the uniqueness check would keep seeing a live
            // binding to the code we just released.
            releaseBinding(oldFp, "unbind: " + reason);
            logEvent(targetU, "unbound", reason, oldFp.substring(0, 12));
            return ok({code: targetU, previous_fingerprint: oldFp.substring(0, 12) + "…"});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.renew — the payment operation
        //
        // WHAT THIS IS FOR: a middleman reports that a student has paid. This
        // extends their term. It is the revenue operation the business plan has
        // always assumed and the code never had — `codes.expire` could set a
        // date, but only by the operator doing the date arithmetic in their
        // head, which is both error-prone and not the same thing as "renew for
        // another term".
        //
        // THE ARITHMETIC IS THE POINT: the new expiry is measured from the
        // LATER of now and the existing expiry. A student who renews two weeks
        // early keeps those two weeks and has the new term added on top.
        // Measuring from `now` would silently delete time the student already
        // paid for, which is the kind of thing that costs a customer.
        //
        // Explicitly does NOT clear `suspended`. A renewal is a payment event,
        // not an abuse pardon; lifting a suspension as a side effect of taking
        // money would let a suspended account buy its way back without the
        // operator ever deciding that. An operator who wants both performs both
        // — two audited actions, two clear records.
        //
        // Also does NOT touch `bound_fingerprint` or `tier`. Renewal extends
        // time; it does not move a device or change what was bought.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.renew") {
            var targetR = canonical(body.code);
            if (!targetR) return bad(400, "code is required");
            var recR = null;
            try { recR = $app.dao().findFirstRecordByData("codes", "code", targetR); } catch (nfRen) { recR = null; }
            if (!recR) return bad(404, "Code not found");

            // The term to apply. An explicit override wins, because the real
            // case on day one is a student paying for a span other than the
            // code's own term (a month rather than a term, or two terms at
            // once). Without it, expressing that means minting a new code —
            // exactly the friction this exists to remove.
            var termDays;
            if (body.term_days !== undefined && body.term_days !== null && String(body.term_days) !== "") {
                termDays = Number(body.term_days);
            } else {
                termDays = Number(recR.get("term_days"));
            }
            if (!isFinite(termDays) || termDays <= 0) {
                return bad(400,
                    "This code has no term set, so there is nothing to renew it by. " +
                    "Pass term_days with the number of days being purchased, or set a " +
                    "term on the code first.");
            }

            var nowMs = Date.now();
            var prevRaw = recR.get("expires_at");
            var prevMs = parsePBDate(prevRaw);
            var prevValid = !isNaN(prevMs) && prevMs > 0;

            // Base = the later of now and a still-running expiry. See above.
            var baseMs = (prevValid && prevMs > nowMs) ? prevMs : nowMs;
            var newMs = baseMs + (termDays * 24 * 60 * 60 * 1000);
            var newIso = new Date(newMs).toISOString();

            recR.set("expires_at", newIso);
            $app.dao().saveRecord(recR);

            // The before → after is recorded because "why is this code's expiry
            // different from its term" is otherwise unanswerable later — and
            // because it is what makes retention countable rather than inferred
            // (see docs/business/redesign/08-metrics-and-instrumentation.md).
            //
            // `price`/`middleman` are carried here rather than as columns: cash
            // is collected by middlemen, and once a code can be renewed
            // repeatedly "how much did X collect" stops being derivable from a
            // code count. One string now avoids a data migration later.
            var price = String(body.price || "").trim();
            var mm = String(body.middleman || "").trim();
            var detail = (prevValid ? String(prevRaw) : "(none)") + " → " + newIso +
                         " (term_days=" + termDays +
                         (price ? ", price=" + price : "") +
                         (mm ? ", middleman=" + mm : "") + ")";
            logEvent(targetR, "renewed", detail, recR.getString("bound_fingerprint").substring(0, 12));

            return ok({
                code: targetR,
                previous_expires_at: prevValid ? String(prevRaw) : "",
                expires_at: newIso,
                term_days: termDays,
                extended: prevValid && prevMs > nowMs,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.rebind — move a code to a different device, deliberately
        //
        // WHY THIS IS SEPARATE FROM unbind: a student replacing a laptop is a
        // normal event. Today the only route is unbind (which clears the
        // binding and the activation time) followed by a fresh activation on
        // the new machine. That works, but it is two steps with no record
        // tying them together, and `codes.unbind` clears `activated_at` — which
        // under the term model is the input to a recomputed expiry. An expiry
        // recomputed from a fresh activation time would silently reset a term
        // the student had already paid for.
        //
        // So this action exists to make the safe thing the easy thing: it
        // changes ONLY the binding, and never touches `expires_at`.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.rebind") {
            var targetRb = canonical(body.code);
            if (!targetRb) return bad(400, "code is required");
            var recRb = null;
            try { recRb = $app.dao().findFirstRecordByData("codes", "code", targetRb); } catch (nfRb) { recRb = null; }
            if (!recRb) return bad(404, "Code not found");

            var newFp = String(body.fingerprint || "").trim();
            if (!newFp) {
                return bad(400,
                    "A fingerprint is required. Move the device to the new machine by " +
                    "activating the code there, then pass the fingerprint it reports.");
            }
            // Normalised ONCE and used for every write in this action.
            //
            // The code's `bound_fingerprint` and the binding index MUST hold the
            // same value, or the two halves of "one code per device" would
            // disagree: the code would name a device the index does not know,
            // and the check would miss it. `recordBinding` in activation.pb.js
            // enforces the same rule — see the note there.
            //
            // The response and the audit entry use the normalised form too, so
            // what an operator sees is what was stored.
            var safeNewFp = newFp.replace(/[^a-zA-Z0-9]/g, "");
            if (!safeNewFp) {
                return bad(400, "That fingerprint has no usable characters — check it and try again.");
            }
            var rbReason = String(body.reason || "").trim();
            if (!rbReason) {
                // Required, matching codes.unbind. The audit trail is the only
                // place the reason a customer's code moved is recorded.
                return bad(400, "A reason is required so the audit trail says why this code moved.");
            }

            var oldFpRb = recRb.getString("bound_fingerprint");
            // Guard against fat-fingering the rebind onto the wrong code: if the
            // caller tells us what it expects to replace and it does not match,
            // refuse rather than silently moving someone else's binding.
            var expect = String(body.expected_fingerprint || "").trim();
            if (expect && expect !== oldFpRb) {
                return bad(409,
                    "This code is not bound to the fingerprint you expected, so nothing " +
                    "was changed. Reload the code and check before rebinding.");
            }

            // Release the old binding in the index, then claim the new one.
            releaseBinding(oldFpRb, "rebind: " + rbReason);
            recRb.set("bound_fingerprint", safeNewFp);
            // NOTE: `expires_at` is deliberately untouched — see the header.
            $app.dao().saveRecord(recRb);
            claimBinding(safeNewFp, targetRb, recRb.getString("tier"));

            logEvent(targetRb, "rebound",
                     (oldFpRb ? oldFpRb.substring(0, 12) + "…" : "(unbound)") +
                     " → " + safeNewFp.substring(0, 12) + "… (" + rbReason + ")",
                     safeNewFp.substring(0, 12));
            return ok({
                code: targetRb,
                previous_fingerprint: oldFpRb ? oldFpRb.substring(0, 12) + "…" : "",
                fingerprint: safeNewFp.substring(0, 12) + "…",
                expires_at: recRb.get("expires_at") ? String(recRb.get("expires_at")) : "",
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.set-term — set or clear a code's term, deliberately
        //
        // Separate from `codes.renew`, which EXTENDS an expiry from a term.
        // This sets the term itself: it is the migration's write path, and the
        // operator's route to correcting a code that was minted without one.
        //
        // It does NOT recompute `expires_at`. Setting a term changes what
        // FUTURE renewals and activations produce; it must never silently
        // rewrite a date a student is already running on. A code that should
        // also move its expiry gets `codes.renew` or `codes.expire` for that,
        // as a second, separately-audited decision.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.set-term") {
            var targetT = canonical(body.code);
            if (!targetT) return bad(400, "code is required");
            var recT = null;
            try { recT = $app.dao().findFirstRecordByData("codes", "code", targetT); } catch (nfT) { recT = null; }
            if (!recT) return bad(404, "Code not found");

            var daysT = String(body.term_days === undefined ? "" : body.term_days).trim();
            if (daysT === "") return bad(400, "term_days is required");
            var numDaysT = Number(daysT);
            if (!isFinite(numDaysT) || numDaysT < 0) {
                return bad(400, "term_days must be a number of days (0 means never expires)");
            }
            numDaysT = Math.floor(numDaysT);

            var prevTerm = recT.get("term_days");
            var prevTxt = (prevTerm === null || prevTerm === undefined || prevTerm === "")
                ? "(none)" : String(prevTerm) + "d";

            if (numDaysT > 0) recT.set("term_days", numDaysT);
            else recT.set("term_days", null); // 0/cleared = never expires

            if (body.term_kind !== undefined) {
                recT.set("term_kind", String(body.term_kind || "").trim());
            }
            $app.dao().saveRecord(recT);

            var whyT = String(body.reason || "").trim();
            logEvent(targetT, "term-set",
                     prevTxt + " → " + (numDaysT > 0 ? numDaysT + "d" : "(none)") +
                     (whyT ? " (" + whyT + ")" : "") +
                     " — expires_at NOT changed",
                     "");
            return ok({
                code: targetT,
                previous_term_days: (prevTerm === null || prevTerm === undefined) ? 0 : prevTerm,
                term_days: numDaysT,
                expires_at: recT.get("expires_at") ? String(recT.get("expires_at")) : "",
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.expire — set or clear an expiry
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.expire") {
            var targetE = canonical(body.code);
            if (!targetE) return bad(400, "code is required");
            var recE = null;
            try { recE = $app.dao().findFirstRecordByData("codes", "code", targetE); } catch (nf4) { recE = null; }
            if (!recE) return bad(404, "Code not found");
            var when = String(body.expires_at || "").trim();
            recE.set("expires_at", when ? when : null);
            $app.dao().saveRecord(recE);
            logEvent(targetE, "expiry-changed", when || "(cleared)", "");
            return ok({code: targetE, expires_at: when});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.update — label / notes / middleman
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.update") {
            var targetUp = canonical(body.code);
            if (!targetUp) return bad(400, "code is required");
            var recUp = null;
            try { recUp = $app.dao().findFirstRecordByData("codes", "code", targetUp); } catch (nf5) { recUp = null; }
            if (!recUp) return bad(404, "Code not found");
            if (body.middleman !== undefined) recUp.set("middleman", String(body.middleman || "").trim());
            if (body.label !== undefined) recUp.set("label", String(body.label || "").trim());
            if (body.notes !== undefined) recUp.set("notes", String(body.notes || "").trim());
            $app.dao().saveRecord(recUp);
            logEvent(targetUp, "updated", "details changed", "");
            return ok({code: targetUp});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.history — per-code event trail
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.history") {
            var targetH = canonical(body.code);
            if (!targetH) return bad(400, "code is required");
            var evs = $app.dao().findRecordsByExpr("code_events",
                $dbx.exp("code = {:c}", { c: targetH }));
            evs.sort(function(a, b) {
                return String(b.get("created") || "").localeCompare(String(a.get("created") || ""));
            });
            var trail = [];
            for (var m = 0; m < evs.length; m++) {
                trail.push({
                    event: evs[m].getString("event"),
                    detail: evs[m].getString("detail"),
                    actor: evs[m].getString("actor"),
                    created: evs[m].getString("created"),
                });
            }
            return ok({code: targetH, events: trail});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.delete — permanently remove ONE code
        //
        // Deliberately the only action that destroys a code row. The project's
        // standing rule is "suspend or unbind instead" (see docs/CONTEXT.md),
        // because bulk-deleting live inventory has broken things before — so
        // this exists for the cases the rule does not cover (typos, duplicate
        // batches, test rows) and refuses the cases it does.
        //
        // Refusals are the point, not decoration:
        //   * a BOUND code is refused — a student is using it. Unbind first, so
        //     the act of releasing the device is a separate, deliberate step.
        //   * an audit event is written BEFORE the row goes, so a deleted code
        //     still leaves a trace (the code_events row outlives the code).
        //
        // NOTE the explicit action guard. Without it this block runs for EVERY
        // action that reaches this point, so any call lacking a body.code —
        // codes.deleteBatch above all — dies with "code is required" before its
        // own handler is ever consulted. Caught by testing the batch path.
        if (action === "codes.delete") {
            var targetD = canonical(body.code);
            if (!targetD) return bad(400, "code is required");
            var recD = null;
            try { recD = $app.dao().findFirstRecordByData("codes", "code", targetD); } catch (nf6) { recD = null; }
            if (!recD) return bad(404, "Code not found");
            var dFp = recD.getString("bound_fingerprint");
            if (body.force_delete_bound !== true && dFp) {
                return bad(409, "That code is bound to a device. Unbind it first, or pass force_delete_bound to override.");
            }
            logEvent(targetD, "deleted",
                     "tier=" + recD.getString("tier") + " middleman=" + recD.getString("middleman") +
                     (dFp ? " was-bound=" + dFp.substring(0, 12) : ""),
                     dFp ? dFp.substring(0, 12) : "");
            // Release the binding index even when a code is force-deleted while
            // bound. Without this, the index keeps a live row pointing at a code
            // that no longer exists, and the uniqueness check would then refuse
            // EVERY future activation on that device — with no code left on the
            // hub for an operator to unbind. The device would be bricked by a
            // deletion, which is not a side effect any operator would expect.
            releaseBinding(dFp, "code deleted");
            $app.dao().deleteRecord(recD);
            return ok({code: targetD, deleted: true});
        }

        // ─────────────────────────────────────────────────────────────
        // codes.deleteBatch — remove MANY codes in one call
        //
        // For clearing an old test batch. Same refusals as codes.delete, applied
        // per code, and it reports exactly which codes were deleted and which
        // were skipped and why — an operator deleting 50 rows needs to know
        // that 3 of them survived and why, not a bare count.
        //
        // A bound code is ALWAYS skipped in a batch (no force flag): the whole
        // point of the batch path is cleanup, and silently releasing a student's
        // device as part of a bulk action is the failure this guards against.
        if (action === "codes.deleteBatch") {
            var codesIn = body.codes;
            if (!codesIn || !codesIn.length) return bad(400, "codes array is required");
            if (codesIn.length > 500) return bad(400, "refusing to delete more than 500 codes in one call");
            var deleted = [], skipped = [];
            for (var di = 0; di < codesIn.length; di++) {
                var cd = canonical(codesIn[di]);
                if (!cd) { skipped.push({code: String(codesIn[di]), why: "invalid format"}); continue; }
                var rd = null;
                try { rd = $app.dao().findFirstRecordByData("codes", "code", cd); } catch (nf7) { rd = null; }
                if (!rd) { skipped.push({code: cd, why: "not found"}); continue; }
                var dfp = rd.getString("bound_fingerprint");
                if (dfp) { skipped.push({code: cd, why: "bound to a device"}); continue; }
                logEvent(cd, "deleted",
                         "batch: tier=" + rd.getString("tier") + " middleman=" + rd.getString("middleman"), "");
                $app.dao().deleteRecord(rd);
                deleted.push(cd);
            }
            return ok({deleted: deleted, skipped: skipped,
                       deleted_count: deleted.length, skipped_count: skipped.length});
        }

        // ─────────────────────────────────────────────────────────────
        // middlemen.list — distinct labels currently in use
        // ─────────────────────────────────────────────────────────────
        if (action === "middlemen.list") {
            var all = $app.dao().findRecordsByExpr("codes", $dbx.exp("id != ''"));
            var seen = {};
            for (var n = 0; n < all.length; n++) {
                var mm = String(all[n].getString("middleman") || "").trim();
                if (mm) seen[mm] = (seen[mm] || 0) + 1;
            }
            var list = [];
            for (var key in seen) list.push({name: key, codes: seen[key]});
            list.sort(function(a, b) { return b.codes - a.codes; });
            return ok({middlemen: list});
        }

        // ─────────────────────────────────────────────────────────────
        // device.get — what is this machine entitled to?
        //
        // The binding index was written by the activation and admin paths but
        // never read back, which made it a write-only audit trail. That left
        // the "one code per device" rule unverifiable from the console: an
        // operator could not answer "does this device hold more than one
        // entitlement?" — the exact integrity question the rule creates.
        //
        // Also gives support a way to confirm a re-bind landed, and to see
        // whether a device has a history of moving (the signature of a card
        // changing hands, versus a laptop replacement).
        // ─────────────────────────────────────────────────────────────
        if (action === "device.get") {
            var wantedFp = String(body.fingerprint || "").trim().replace(/[^a-zA-Z0-9]/g, "");
            if (!wantedFp) return bad(400, "fingerprint is required");

            var row = null;
            try {
                row = $app.dao().findFirstRecordByFilter("device_bindings",
                    "fingerprint = '" + wantedFp + "'");
            } catch (none) { row = null; }

            if (!row) return ok({found: false, fingerprint: wantedFp});

            var heldCode = row.getString("code");
            var released = !!row.getString("released_at");

            // Read the code the device is bound to, if it still exists, so the
            // response can carry the student-facing facts (label, expiry)
            // alongside the binding. A binding whose code is missing is worth
            // reporting rather than hiding: it means the index and the codes
            // table disagree.
            var heldRec = null;
            try { heldRec = $app.dao().findFirstRecordByData("codes", "code", heldCode); } catch (nfHeld) { heldRec = null; }

            return ok({
                found: true,
                fingerprint: wantedFp,
                code: heldCode,
                tier: row.getString("tier") || "",
                bound_at: row.getString("bound_at") || "",
                released: released,
                released_at: row.getString("released_at") || "",
                release_reason: row.getString("release_reason") || "",
                // A live binding whose code no longer exists. The uniqueness
                // check would still refuse a new activation, so this is a state
                // an operator needs to see and clear, not a silent oddity.
                code_missing: !released && !heldRec,
                label: heldRec ? (heldRec.getString("label") || "") : "",
                expires_at: (heldRec && heldRec.get("expires_at")) ? String(heldRec.get("expires_at")) : "",
                suspended: heldRec ? heldRec.getBool("suspended") : false,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // devices.list — every binding, for integrity checking
        //
        // The point is to make the "one code per device" rule AUDITABLE: a
        // fingerprint appearing more than once as live would mean the rule was
        // violated (by a bug, by a direct write, or by a migration that predates
        // the index). The unique constraint should prevent that, so this is the
        // check that the constraint is actually doing its job.
        // ─────────────────────────────────────────────────────────────
        if (action === "devices.list") {
            var bindings = null;
            try {
                bindings = $app.dao().findRecordsByExpr("device_bindings", $dbx.exp("id != ''"));
            } catch (devErr) {
                return bad(500, "could not read the binding index: " + (devErr && devErr.message ? devErr.message : String(devErr)));
            }

            var live = {}, rows = [];
            for (var b = 0; b < bindings.length; b++) {
                var br = bindings[b];
                var bfp = br.getString("fingerprint");
                var bCode = br.getString("code");
                var bReleased = !!br.getString("released_at");
                var bRec = null;
                try { bRec = $app.dao().findFirstRecordByData("codes", "code", bCode); } catch (nfB) { bRec = null; }
                if (!bReleased) live[bfp] = (live[bfp] || 0) + 1;
                rows.push({
                    fingerprint: bfp ? bfp.substring(0, 12) + "…" : "",
                    code: bCode,
                    tier: br.getString("tier") || "",
                    bound_at: br.getString("bound_at") || "",
                    released: bReleased,
                    released_at: br.getString("released_at") || "",
                    release_reason: br.getString("release_reason") || "",
                    code_missing: !bReleased && !bRec,
                    label: bRec ? (bRec.getString("label") || "") : "",
                    expires_at: (bRec && bRec.get("expires_at")) ? String(bRec.get("expires_at")) : "",
                });
            }

            // Duplicates should be impossible given the unique index. Report
            // them rather than assuming, so a violated assumption is visible.
            var duplicates = [];
            for (var dfp in live) { if (live[dfp] > 1) duplicates.push(dfp.substring(0, 12) + "…"); }

            rows.sort(function(a, b) { return (b.bound_at || "").localeCompare(a.bound_at || ""); });
            return ok({
                total: rows.length,
                live_count: (function() { var c = 0; for (var k in live) c++; return c; })(),
                duplicate_live_fingerprints: duplicates,
                bindings: rows,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // tiers.list / tiers.update — connection settings
        // ─────────────────────────────────────────────────────────────
        if (action === "tiers.list") {
            var tc = $app.dao().findRecordsByExpr("tier_configs", $dbx.exp("id != ''"));
            var tiers = [];
            for (var p = 0; p < tc.length; p++) {
                var cfg = {};
                try { cfg = JSON.parse(tc[p].getString("config")); } catch (parseErr) { cfg = {}; }
                tiers.push({
                    id: tc[p].id,
                    tier: tc[p].getString("tier"),
                    active: tc[p].getBool("active"),
                    udp_relay: tc[p].getBool("udp_relay"),
                    server: cfg.server || "",
                    server_port: cfg.server_port || 0,
                    method: cfg.method || "",
                    // The UoT endpoint lives inside the tier's config JSON. It is
                    // surfaced here because udp_relay is only HALF the switch:
                    // the client builds a UDP-over-TCP outbound only when
                    // udp_relay is set AND uot_port > 0. An operator toggling
                    // udp_relay without being able to see uot_port is editing a
                    // setting whose effect they cannot observe.
                    uot_port: parseInt(cfg.uot_port || "0", 10) || 0,
                });
            }
            return ok({tiers: tiers});
        }

        // ─────────────────────────────────────────────────────────────
        // tiers.update — change port/method/active/udp_relay
        // NOTE: the password is deliberately NOT editable here. Changing it
        // silently invalidates every issued client. That stays a deliberate,
        // documented operation (see OPS.md), not a stray click.
        // ─────────────────────────────────────────────────────────────
        if (action === "tiers.update") {
            var tierName = String(body.tier || "").trim();
            if (!tierName) return bad(400, "tier is required");
            var recT = null;
            try { recT = $app.dao().findFirstRecordByData("tier_configs", "tier", tierName); } catch (nf6) { recT = null; }
            if (!recT) return bad(404, "Tier not found");
            var cfgT = {};
            try { cfgT = JSON.parse(recT.getString("config")); } catch (parseErr2) { cfgT = {}; }
            if (body.server !== undefined && String(body.server).trim()) cfgT.server = String(body.server).trim();
            if (body.server_port !== undefined) {
                var port = parseInt(body.server_port, 10);
                if (isNaN(port) || port < 1 || port > 65535) return bad(400, "server_port must be 1-65535");
                cfgT.server_port = port;
            }
            if (body.method !== undefined && String(body.method).trim()) cfgT.method = String(body.method).trim();
            // UoT endpoint. 0 (or absent) means "no UDP-over-TCP endpoint for
            // this tier", which is the correct default: shadowsocks-rust does
            // not implement sing-box's UoT protocol, so a non-zero port must
            // only ever point at a sing-box UoT listener (see enable-uot.sh).
            // Kept optional so an older console build keeps working.
            if (body.uot_port !== undefined) {
                var uot = parseInt(body.uot_port, 10);
                if (isNaN(uot) || uot < 0 || uot > 65535) return bad(400, "uot_port must be 0-65535");
                if (uot === 0) { delete cfgT.uot_port; } else { cfgT.uot_port = uot; }
            }
            recT.set("config", JSON.stringify(cfgT));
            if (body.active !== undefined) recT.set("active", !!body.active);
            if (body.udp_relay !== undefined) recT.set("udp_relay", !!body.udp_relay);
            $app.dao().saveRecord(recT);
            logEvent("(tier " + tierName + ")", "tier-updated", JSON.stringify(cfgT), "");
            // Deliberately NOT returning cfgT.
            //
            // cfgT contains the tier password in cleartext (the shadowsocks PSK
            // every client on that tier shares). tiers.list already omits it for
            // exactly that reason, but this endpoint echoed the whole config
            // object — so anyone using curl instead of the Web UI leaked the
            // shared secret into a response body, an operator's terminal
            // scrollback, browser devtools and any body-capturing proxy.
            //
            // Nothing consumed it: the console never read this field, and the
            // Tiers page re-renders from tiers.list. Callers that want the
            // current values should call tiers.list, which returns the safe
            // subset. Echo only what an operator can actually edit here.
            return ok({
                tier: tierName,
                server: cfgT.server || "",
                server_port: parseInt(cfgT.server_port || "0", 10) || 0,
                method: cfgT.method || "",
                uot_port: parseInt(cfgT.uot_port || "0", 10) || 0,
                udp_relay: recT.getBool("udp_relay"),
                active: recT.getBool("active"),
            });
        }

        // ─────────────────────────────────────────────────────────────
        // releases.get — current update_config, plus what is on disk
        // ─────────────────────────────────────────────────────────────
        if (action === "releases.get") {
            var rel = null;
            try { rel = $app.dao().findFirstRecordByFilter("update_config", "id != ''"); } catch (nf7) { rel = null; }
            var payload = {version: "", active: false, platforms: {}};
            if (rel) {
                payload.version = rel.getString("version");
                payload.active = rel.getBool("active");
                var platKeys = ["linux", "windows", "macos_intel", "macos_arm"];
                for (var pi = 0; pi < platKeys.length; pi++) {
                    var pk = platKeys[pi];
                    payload.platforms[pk] = {
                        url: rel.getString("download_" + pk) || "",
                        sha256: rel.getString("sha256_" + pk) || "",
                        // Surfaced so the console can show whether each platform
                        // is actually installable. A published release with no
                        // signature looks fine but reaches nobody, because the
                        // updater verifies one mandatorily.
                        signed: !!rel.getString("signature_" + pk),
                    };
                }
            }
            return ok({release: payload});
        }

        // ─────────────────────────────────────────────────────────────
        // releases.fetchLink — mint a one-shot trigger link
        // ─────────────────────────────────────────────────────────────
        //
        // The signing key lives in the fetch service (it owns the secret and the
        // nonce store), so this action forwards there over loopback rather than
        // duplicating HMAC logic inside a hook — two implementations of a
        // signature check is how they drift apart.
        //
        // It routes through PocketBase so the console keeps using the SAME
        // authenticated endpoint for everything (one auth check, one audit
        // point), instead of calling the loopback service directly.
        if (action === "releases.fetchLink") {
            var lv = String(body.version || "").trim().replace(/^v/, "");
            if (!/^[0-9]+\.[0-9]+\.[0-9]+/.test(lv)) {
                return bad(400, "version must look like 1.2.3");
            }
            var svcUrl = $os.getenv("FETCH_SERVICE_URL") || "http://127.0.0.1:8091";
            var linkRes = null;
            try {
                linkRes = $http.send({
                    url: svcUrl + "/api/admin/fetch-link?version=" + lv,
                    method: "GET",
                    headers: { "X-Admin-Token": validToken },
                    timeout: 15,
                });
            } catch (httpErr) {
                return bad(502, "the release fetch service is not reachable: " +
                                (httpErr.message || String(httpErr)));
            }
            if (!linkRes || linkRes.statusCode !== 200) {
                return bad(502, "the release fetch service refused to mint a link (HTTP " +
                                (linkRes ? linkRes.statusCode : "?") + ")");
            }
            var parsedLink = null;
            try { parsedLink = JSON.parse(linkRes.raw); } catch (pjErr) { parsedLink = null; }
            if (!parsedLink || !parsedLink.ok) {
                return bad(502, "the release fetch service returned an unusable link");
            }
            logEvent("(release " + lv + ")", "fetch-link-minted", "one-shot link issued", "");
            return ok({ link: parsedLink.link, expires_in: parsedLink.expires_in });
        }

        // ─────────────────────────────────────────────────────────────
        // releases.publish — write the DB half of a published release
        // ─────────────────────────────────────────────────────────────
        //
        // The bytes are fetched from the GitHub Release by the fetch service
        // (scripts/fetch-release.py), which owns /var/www/updates and verifies
        // every hash and file format. This action is the OTHER half: it points
        // update_config at those artifacts.
        //
        // WHY IT IS STILL SEPARATE FROM releases.set
        // releases.set only flips `active`, and refuses to activate a release
        // whose artifacts are missing, unsigned, or point at a different
        // version. So the artifacts must be recorded first: fetch -> publish ->
        // activate. This action is the middle step, and it is the only place the
        // download_* / sha256_* / signature_* columns are written from a fetch
        // result.
        //
        // The per-platform hashes come from the files the service actually
        // hashed on disk, not from what the caller claims — the fetcher returns
        // the digest it computed while streaming the bytes.
        if (action === "releases.publish") {
            var pubVersion = String(body.version || "").trim().replace(/^v/, "");
            if (!pubVersion) return bad(400, "version is required");
            if (!/^[0-9]+\.[0-9]+\.[0-9]+/.test(pubVersion)) {
                return bad(400, "version must look like 1.2.3");
            }

            // artifacts: { linux: {sha256, bytes, filename, signature}, ... }
            var arts = body.artifacts || {};
            var platKeysP = ["linux", "windows", "macos_intel", "macos_arm"];
            var missingP = [];
            for (var pi2 = 0; pi2 < platKeysP.length; pi2++) {
                var pk2 = platKeysP[pi2];
                var a = arts[pk2];
                // signature is required, not optional. The Tauri updater
                // verifies a minisign signature MANDATORILY and has no bypass,
                // so a release published without one is uninstallable by every
                // client — while looking perfectly healthy from here.
                if (!a || !a.sha256 || !a.filename || !a.signature) { missingP.push(pk2); continue; }
                // Presence is not enough — the SHAPE must be the wire format the
                // client decodes: base64 of the whole .sig text. A raw-text
                // value (the .sig file verbatim) passes a presence check, is
                // written to signature_<platform>, advertised to every client, and
                // then fails at install time with `Invalid byte … , offset N`.
                // The fetch step now encodes this correctly; this guard is the
                // backstop, and it is the SAME predicate releases.set uses, so
                // publish and activate can no longer disagree about what is valid.
                if (!isPlausibleMinisignSignature(a.signature)) missingP.push(pk2 + " (signature not base64 of a minisign .sig file)");
            }
            if (missingP.length) {
                return bad(400,
                    "refusing to publish " + pubVersion + " — the fetch did not " +
                    "produce a verified, signed artifact for: " + missingP.join(", ") +
                    ". Every artifact needs a minisign signature (CI signs them; " +
                    "see client/docs/SIGNING.md); without one no client can " +
                    "install the update, and the omission is invisible from here.");
            }

            var baseUrl = String(body.base_url || "").trim().replace(/\/$/, "");
            if (!baseUrl) {
                return bad(400, "base_url is required (e.g. https://<domain>) so " +
                                "the download URLs are absolute");
            }

            var recP = null;
            try { recP = $app.dao().findFirstRecordByFilter("update_config", "id != ''"); } catch (nfP) { recP = null; }
            if (!recP) {
                var collP = $app.dao().findCollectionByNameOrId("update_config");
                recP = new Record(collP);
            }

            for (var pi3 = 0; pi3 < platKeysP.length; pi3++) {
                var pk3 = platKeysP[pi3];
                var art = arts[pk3];
                // The client's updater builds these URLs itself, but the
                // heartbeat hands it whatever is stored here — so the stored
                // form must be the exact path Caddy serves (handle_path
                // /updates/*), with the version and filename already in it.
                recP.set("download_" + pk3,
                         baseUrl + "/updates/" + pubVersion + "/" + art.filename);
                recP.set("sha256_" + pk3, String(art.sha256));
                recP.set("signature_" + pk3, String(art.signature));
            }
            recP.set("version", pubVersion);
            recP.set("active", true);
            // Publishing IS offering. There is no rollout percentage any more:
            // a published release is served to every client on its next check.
            // `active` remains as the single off switch — see releases.set.
            $app.dao().saveRecord(recP);

            logEvent("(release " + pubVersion + ")", "release-published",
                     "artifacts verified, signed and written; live to all clients", "");
            return ok({
                version: pubVersion,
                active: recP.getBool("active"),
                platforms: platKeysP,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // releases.set — turn an update on or off
        //
        // There is NO rollout percentage. A published release is served to every
        // client on its next check; this action exists only to flip `active`.
        //
        // WHY THE PERCENTAGE WENT
        // It compensated for having no rollback: the client's only automatic
        // recovery is the installer's own atomicity, so a build that launches
        // but misbehaves stays installed, and the gate limited how many users
        // found out first. It was removed deliberately — the cost was a fleet
        // where a low percentage means your own test device is probably not in
        // the bucket, so a working updater looks broken and is misdiagnosed.
        //
        // `active` is kept, and it is the ONLY remaining lever. Without it a bad
        // build would have no off switch at all. Note what it does and does not
        // do: it stops the hub OFFERING the update. Clients that already
        // installed it stay on it, and there is still no server-driven
        // downgrade — the fix for a bad build is a higher version.
        // ─────────────────────────────────────────────────────────────
        if (action === "releases.set") {
            var version = String(body.version || "").trim().replace(/^v/, "");
            if (!version) return bad(400, "version is required");
            if (!/^[0-9]+\.[0-9]+\.[0-9]+/.test(version)) {
                return bad(400, "version must look like 1.2.3");
            }
            var wantActive = body.active === undefined ? true : !!body.active;

            var recR = null;
            try { recR = $app.dao().findFirstRecordByFilter("update_config", "id != ''"); } catch (nf8) { recR = null; }
            if (!recR) {
                var collR = $app.dao().findCollectionByNameOrId("update_config");
                recR = new Record(collR);
            }

            // ── Guard: never ADVERTISE a version we cannot serve ──
            //
            // The hook serves whatever URLs sit in this row, and those URLs are
            // NOT derived from what is on disk — they are written by the fetch
            // service (which verifies the served bytes) or by releases.publish.
            // Turning a release on while its URLs are empty or stale sends every
            // client to a 404, and a client that cannot download cannot update:
            // the release silently fails for the whole fleet with no error
            // anywhere but the client log.
            //
            // This has actually happened on this hub: an old row pointed at
            // /updates/1.0.2/* after those test artifacts were deleted.
            //
            // A PocketBase hook cannot stat the filesystem (only $os.getenv is
            // exposed), so the checks available here are that every platform has
            // a URL, a hash AND a signature, and that the URL names the version
            // being advertised — a stale row claiming 1.0.0 while pointing at
            // /updates/1.0.2/ is served as though it were current.
            //
            // This guard now applies whenever the release is being ACTIVATED.
            // Previously it only ran for rollout > 0, so a release could be
            // marked active with no artifacts at all.
            //
            // It checks a signature is PRESENT and, since 2026-09-29, that it has
            // the right SHAPE. Presence alone was not enough: a value that was
            // non-empty but not what the plugin's decoder expects passed here,
            // was advertised to every client, and failed at install time with
            //
            //   "The signature … could not be decoded, please check if it is a
            //    valid base64 string."
            //
            // — a failure the operator saw as a student-facing bug, days after
            // the pipeline reported the release healthy. The shape check is the
            // shared `isPlausibleMinisignSignature` declared at the top of this
            // handler, so the set path and the publish path apply the SAME rule;
            // a release one accepts while the other rejects was how a broken row
            // reached production. It cannot verify the signature (this hook has
            // no public key, and should not hold one) but it does catch a
            // wrong-format or truncated field.
            if (wantActive) {
                var platKeysG = ["linux", "windows", "macos_intel", "macos_arm"];
                var missingG = [];
                for (var gi = 0; gi < platKeysG.length; gi++) {
                    var gk = platKeysG[gi];
                    var gurl = recR.getString("download_" + gk);
                    var gsha = recR.getString("sha256_" + gk);
                    var gsig = recR.getString("signature_" + gk);
                    if (!gurl || !gsha) { missingG.push(gk); continue; }
                    if (!gsig) { missingG.push(gk + " (no signature)"); continue; }
                    if (!isPlausibleMinisignSignature(gsig)) {
                        missingG.push(gk + " (signature is not a minisign signature)");
                        continue;
                    }
                    if (gurl.indexOf("/updates/" + version + "/") === -1) {
                        missingG.push(gk + " (url is for a different version)");
                    }
                }
                if (missingG.length) {
                    return bad(400,
                        "refusing to activate " + version + " — no usable artifact for: " +
                        missingG.join(", ") +
                        ". Publish the release first (all four signed binaries), " +
                        "or clients will be sent to a 404, or handed an update " +
                        "the updater cannot verify. A signature that is not real " +
                        "minisign output is refused here rather than at install " +
                        "time on every client.");
                }
            }

            recR.set("version", version);
            recR.set("active", wantActive);
            $app.dao().saveRecord(recR);
            logEvent("(release " + version + ")", "release-set",
                     "active=" + recR.getBool("active"), "");
            return ok({version: version, active: recR.getBool("active")});
        }

        return bad(400, "Unknown action: " + action);
    } catch (err) {
        return e.json(500, {ok: false, message: (err && err.message) ? err.message : String(err)});
    }
});
