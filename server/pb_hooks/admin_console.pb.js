// Chrysalis Admin API — server-side surface for the web console.
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

    // Base64-decode to a binary string, in PURE JS.
    //
    // WHY THIS IS HAND-ROLLED: PocketBase 0.22.21's goja defines NO base64
    // builtin. `atob`, `btoa`, `Buffer` and `TextDecoder` are all undefined,
    // and neither `$os` (filesystem/exec only) nor `$security` (hashes, JWT,
    // encrypt/decrypt) exposes one. Measured on the live hub 2026-09-30.
    //
    // This mattered: the previous decoder line was `$os ? atob(s) : ""`. `$os`
    // is truthy, so it called an undefined `atob`, threw ReferenceError, and the
    // enclosing try/catch turned that CRASH into the verdict "not base64 of a
    // minisign .sig file" — for EVERY input, correct ones included. The console
    // refused to publish or activate any release, blaming the fetch, which was
    // innocent throughout. See FIXES.md 2026-09-30.
    //
    // Standard alphabet, big-endian bit accumulation, '=' terminates. Returns
    // "" on anything that is not clean base64 so a malformed value fails the
    // shape check below rather than decoding to garbage.
    function base64ToBinary(str) {
        var ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        var s = String(str || "").replace(/[=\s]+$/, "");
        if (!s) return "";
        var out = "", bits = 0, acc = 0;
        for (var i = 0; i < s.length; i++) {
            var v = ALPHABET.indexOf(s.charAt(i));
            if (v < 0) return "";        // not base64 at all
            acc = (acc << 6) | v;
            bits += 6;
            if (bits >= 8) {
                bits -= 8;
                out += String.fromCharCode((acc >> bits) & 0xFF);
            }
        }
        return out;
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
        var decoded = base64ToBinary(s);
        if (!decoded) return false;
        var lines = decoded.split("\n");
        var nonEmpty = [];
        for (var li = 0; li < lines.length; li++) {
            if (lines[li].replace(/\s/g, "")) nonEmpty.push(lines[li]);
        }
        if (nonEmpty.length < 4) return false;
        return nonEmpty[0].indexOf("untrusted comment: ") === 0 &&
               nonEmpty[2].indexOf("trusted comment: ") === 0;
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

    // ── Writing a column the live hub may not have yet ──
    //
    // Same helper as activation.pb.js — read that one for the source-verified
    // account. The short version, because the first draft of this comment was
    // WRONG and the correction matters:
    //
    // PocketBase does NOT fail a save for an undefined field. `Record.Set` keeps
    // it in the record's custom data and `Dao.update` writes only the collection
    // SCHEMA's columns, so the value is silently discarded. Writing
    // `suspended_reason` on the live hub — whose `codes` table has no such column
    // — was therefore harmless to the save, not fatal to it.
    //
    // What it cost is real but different from what was first written here: the
    // operator's reason was accepted and thrown away, so a kicked student saw the
    // generic "contact your middleman" instead of the sentence the operator
    // actually typed. Nothing said the reason had been dropped — which is exactly
    // the "operator presses the button and nothing happens" shape, and it is why
    // this helper makes the drop REPORTABLE rather than silent.
    //
    // `suspended` itself does NOT go through this: it decides whether a device is
    // cut off, it exists on every deployed hub, and if it were ever missing that
    // should be loud.
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

    // ── The liveness axis, as the HUB needs it for a confirmation ──
    //
    // This is a COARSE classification, used only to decide (a) whether a release
    // should ask the heartbeat to evict a live client and (b) what evidence to
    // record in the audit row. It is NOT the console's rendering rule: the
    // vocabulary and the lease are owned by `session_probe.pb.js` and
    // `views/codes/model.ts`, and §58/§59 execute those two against each other.
    // Do not grow this into a third opinion — it answers one yes/no question.
    //
    // Returns "live" | "stale" | "absent" | "unknown". `unknown` on an empty or
    // unparseable value, NEVER "absent": a code that has not beaten has no
    // evidence either way, and calling it gone is the accusation this product
    // already shipped once (see docs/reference/AUTH-STATE.md).
    function livenessForConfirmation(record) {
        var raw = record.get("last_seen_at");
        if (raw === null || raw === undefined || String(raw).trim() === "") return "unknown";
        var seenMs = parsePBDate(raw);
        if (seenMs === 0 || isNaN(seenMs)) return "unknown";
        var nowMs = Date.now();
        // A future stamp means two clocks disagree; the age is not knowable.
        if (seenMs > nowMs + 5 * 60 * 1000) return "unknown";
        var age = nowMs - seenMs;
        var leaseMs = (2 * 60 * 60 + 14 * 60) * 1000;   // §59: 2h14m
        var staleMs = 7 * 24 * 60 * 60 * 1000;          // mirrors GRACE_PERIOD
        if (age <= leaseMs) return "live";
        if (age <= staleMs) return "stale";
        return "absent";
    }

    // ── The override audit row — HARD FAIL, unlike logEvent ──
    //
    // `code_events` is best-effort: `logEvent` swallows its exception, so it can
    // record an action that never happened. An override needs the opposite
    // guarantee, because the audit row IS the justification for a decision that
    // cuts a paying student off. So this THROWS, and the caller lets the throw
    // become a 500 with nothing else applied.
    //
    // Ordering in the caller: this runs BEFORE the record is saved. If the row
    // cannot be written, no state changes. `code_events` is still written after
    // the save, as a best-effort MIRROR so the existing history drawer needs no
    // change.
    function writeOverride(o) {
        var coll = $app.dao().findCollectionByNameOrId("code_overrides");
        var rec = new Record(coll);
        rec.set("code", o.code);
        rec.set("override_id", o.override_id);
        rec.set("aspect", o.aspect);
        rec.set("from_value", o.from_value || "");
        rec.set("to_value", o.to_value || "");
        rec.set("reason", o.reason);
        rec.set("actor", o.actor || "console");
        rec.set("observed_state", o.observed_state || "unknown");
        if (o.observed_at_override) rec.set("observed_at_override", o.observed_at_override);
        rec.set("outcome", o.outcome);
        $app.dao().saveRecord(rec);   // throws → caller returns 500
    }

    // Has this override already been applied? The console mints one id per
    // confirmed action and reuses it on retry, so a retried request must be a
    // no-op rather than a second side effect. Matched on (code, override_id).
    function findOverride(code, overrideId) {
        // `canonical()` passes a non-conforming string THROUGH unchanged (verified
        // against its own logic), so a crafted `code` could otherwise reach this
        // filter. A code is `RQ-XXXX-XXXX-XXXX-C`; anything else cannot be in
        // `code_overrides`, so return null rather than building a filter from it.
        // Belt-and-braces on top of the quote-stripping below: the strip makes the
        // filter safe, this makes the answer honest.
        if (!/^[A-Z0-9]{2}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]$/.test(String(code))) {
            return null;
        }
        try {
            return $app.dao().findFirstRecordByFilter(
                "code_overrides",
                "code = '" + String(code).replace(/'/g, "") + "' && override_id = '" +
                String(overrideId).replace(/'/g, "") + "'");
        } catch (none) {
            return null;
        }
    }

    // ── Device binding: REMOVED ──
    //
    // There used to be a `device_bindings` index here, keyed on a fingerprint,
    // enforcing "one code per device" at the schema level. Both the rule and the
    // index are gone (see activation.pb.js): a code is single-use, and the only
    // record of that is `activated_at` on the code row itself. There is no
    // device left to name, so there is nothing to index, claim, release or
    // reconcile — which is exactly the class of bug (two structures disagreeing
    // about the same binding) this removes.
    //
    // The actions that used these helpers are rewritten below to operate on
    // `activated_at` directly.

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
            //
            // READ IT DEFENSIVELY — observed 2026-10-18 returning 403 against a
            // correct token that the BODY path accepted on the next line.
            //
            // THE HEADER NAME IS NOT WHAT THE CLIENT SENDS. `requestInfo(e).headers`
            // exposes each header with its `-` replaced by `_` and lower-cased, so
            // `X-Admin-Token` arrives as `x_admin_token` — never `x-admin-token`.
            // The old code looked up the hyphenated form, which is why this path
            // had never once authenticated; it only appeared to work because the
            // console also sends the token in the body.
            //
            // Established by probing the live hub rather than by reading the docs,
            // because the docs describe the Go type and this is the goja bridge's
            // presentation of it: a probe endpoint returned the real key list
            // (`content_type`, `x_admin_token`, `x_forwarded_for`, …). Both spellings
            // are tried so the code survives a future PocketBase presenting either.
            try {
                var headers = $apis.requestInfo(e).headers || {};
                var hdr = "";
                var headerNames = ["x_admin_token", "X-Admin-Token", "x-admin-token", "X_Admin_Token"];
                for (var hn = 0; hn < headerNames.length && !hdr; hn++) {
                    try {
                        var cand = headers[headerNames[hn]];
                        if (cand !== null && cand !== undefined && cand !== "") hdr = String(cand);
                    } catch (idxErr) { /* fall through to the next spelling */ }
                }
                if (!hdr) {
                    // Last resort: walk the map's own keys and normalise BOTH sides
                    // (`-` and `_` collapse to `_`), so any casing or separator
                    // works.
                    var keys = [];
                    try { keys = Object.keys(headers); } catch (kErr) { keys = []; }
                    for (var hi = 0; hi < keys.length; hi++) {
                        var norm = String(keys[hi]).toLowerCase().replace(/-/g, "_");
                        if (norm === "x_admin_token") {
                            var hv = headers[keys[hi]];
                            // A header may arrive as an array of values.
                            if (hv && typeof hv.length === "number" && typeof hv !== "string") hv = hv[0];
                            hdr = String(hv || "");
                            break;
                        }
                    }
                }
                supplied = hdr.trim();
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
            var redeemed = 0, suspended = 0, expired = 0, available = 0;
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
                var isUsed = c.get("activated_at") !== null && c.get("activated_at") !== undefined
                             && String(c.get("activated_at")).trim() !== "";
                if (isSusp) suspended++;
                else if (isExp) expired++;
                // "Redeemed" replaces the old "bound": a code is single-use now,
                // and `activated_at` is the record of that (see activation.pb.js).
                // No device is named, so there is nothing per-device to count.
                else if (isUsed) redeemed++;
                else available++;
                // ── expiringSoon: the RENEWAL queue's size ──
                //
                // Counted with the same rule `codes.list` applies when
                // `expiring_within_days` is set — because the console's card
                // links straight to that filtered list, and a card that says 7
                // while the page shows 11 is a bug an operator would read as a
                // stale page. Measured: the old line counted every future
                // expiry inside 30 days, including UNUSED stock, which the
                // worklist correctly excludes — an unsold card cannot lapse, so
                // there is nothing to renew.
                //
                // Suspended codes are excluded for the same reason the worklist
                // excludes them: the operator deliberately cut that student off,
                // and prompting a renewal would be asking them to undo it.
                // `<=`, not `<`. The list this card links to keeps a code whose
                // expiry is exactly `windowEnd` (`expMs2 > windowEndMs` skips it,
                // so equal passes). With `<` here, a code expiring in exactly 30
                // days appeared in the list and not in the count — the card said
                // 3 and the page showed 4. Found by the fixture in
                // console-card-consistency.js, which now carries a row on the
                // boundary; the two expressions must stay the same shape.
                if (expMs && !isNaN(expMs) && expMs > now && expMs <= in30 && !isSusp && isUsed) expiringSoon++;
            }

            // `attempts` was a bare count of `activation_attempts` — a table
            // activation.pb.js PURGES at ten minutes, holding a code truncated to
            // four characters. As a number alone it answered nothing: no window
            // was stated, no code shown, no way to tell a mistyped card from a
            // scanning run. The console labelled it "in the last 10 minutes
            // window", which was true only by accident of the purge.
            //
            // It is now reported with the window it actually covers and split
            // by the namespace `rate_key` already carries (`activate_` vs
            // `lookup_` — see activation.pb.js), so a spike in REFUSED ACTIVATIONS
            // reads differently from a spike in LOOKUPS, which is the difference
            // between "someone cannot get in" and "someone is guessing".
            var attempts = $app.dao().findRecordsByExpr("activation_attempts", $dbx.exp("id != ''"));
            var attemptActivate = 0, attemptLookup = 0;
            for (var ati = 0; ati < attempts.length; ati++) {
                var rk = String(attempts[ati].getString("rate_key") || "");
                if (rk.indexOf("lookup_") === 0) attemptLookup++;
                else attemptActivate++;
            }
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
                    redeemed: redeemed,
                    suspended: suspended,
                    expired: expired,
                    byTier: byTier,
                    expiringSoon: expiringSoon,
                },
                // `total` is kept for older callers; the split is what the
                // console reads. `window_minutes` states the window rather than
                // leaving the console to imply one, because the table is
                // purged by the hooks that write to it and a hard-coded "10
                // minutes" in the template would be a guess that can silently
                // stop being true.
                attempts: {
                    total: attempts.length,
                    activate: attemptActivate,
                    lookup: attemptLookup,
                    window_minutes: 10,
                },
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

            // ── Facet counts ──
            //
            // Counts for the console's filter bar, over the SAME snapshot the
            // caller's rows come from — so the number beside "suspended" cannot
            // describe a different instant than the list below it.
            //
            // Deliberately UNFILTERED except by the window: the point of a facet
            // count is "how many are over there", so counting within the current
            // filter would make every unpressed facet read 0 and the control
            // useless. With thousands of codes this costs one extra pass over
            // rows the handler already loaded, which is why it is here rather
            // than a second round-trip from the browser.
            //
            // The window still applies, because a worklist facet that counted
            // codes outside the worklist would be actively misleading.
            var facetStatus = {"available": 0, "redeemed": 0, "expired": 0, "suspended": 0};
            var facetTier = {};
            for (var fj = 0; fj < recs.length; fj++) {
                var fr = recs[fj];
                var fexpRaw = fr.get("expires_at");
                var fexp = parsePBDate(fexpRaw);
                var fisExp = fexp && !isNaN(fexp) && fexp < nowMs;
                var fsusp = fr.getBool("suspended");
                var fred = !!String(fr.getString("activated_at") || "");
                if (useWindow) {
                    if (excludeSuspended && fsusp) continue;
                    if (!fexp || isNaN(fexp) || fexp <= nowMs || fexp > windowEndMs) continue;
                }
                // FOUR BUCKETS, exactly as the filter bar renders and
                // console-card-consistency.js asserts. The override is a flag on
                // a row, not a fifth bucket — see the note at `var status` above.
                var fstatus = fsusp ? "suspended" : fisExp ? "expired" : fred ? "redeemed" : "available";
                facetStatus[fstatus] = (facetStatus[fstatus] || 0) + 1;
                var ftier = fr.getString("tier") || "(none)";
                facetTier[ftier] = (facetTier[ftier] || 0) + 1;
            }

            var out = [];
            for (var j = 0; j < recs.length; j++) {
                var rec = recs[j];
                var codeVal = rec.getString("code");
                // A code is single-use now; `activated_at` is the whole record of
                // that (see activation.pb.js). There is no fingerprint and no
                // binding index to cross-check.
                var redeemedAt = rec.getString("activated_at") || "";
                var expRaw = rec.get("expires_at");
                var expMs2 = parsePBDate(expRaw);
                var isExp2 = expMs2 && !isNaN(expMs2) && expMs2 < nowMs;
                var susp2 = rec.getBool("suspended");
                // ── `status` IS A FOUR-VALUE PARTITION, AND THE OVERRIDE IS NOT ONE OF THEM ──
                //
                // This was wrong in the first version of this change and the
                // error is worth recording. Adding "released" as a fifth status
                // broke three things at once, all silently:
                //
                //   * `?status=available` stopped listing released stock, so the
                //     "Unused" filter — which an operator uses to find cards to
                //     sell — hid exactly the codes that were free to sell.
                //   * the facet counts gained a "released" bucket that the filter
                //     bar's four <option>s do not render, so those codes vanished
                //     from every count.
                //   * `dashboard` counts available/redeemed/suspended/expired and
                //     never read `force_state`, so the dashboard and the list
                //     disagreed about the same code in one console.
                //
                // `server/scripts/console-card-consistency.js` already asserts
                // "the four buckets partition every code". That guard was right
                // and the change was wrong.
                //
                // So status keeps its four values and the override is ORTHOGONAL:
                // `force_state` and `evict_requested_at` are passed through as
                // their own fields and rendered BESIDE the status, never as it.
                // A released code is `status: "available"` (it IS available to a
                // new holder — see activation.pb.js) that also carries
                // `force_state: "released"`.
                var forceState = String(rec.get("force_state") || "");
                var evictAt = String(rec.get("evict_requested_at") || "");

                var status = susp2 ? "suspended"
                           : isExp2 ? "expired"
                           : redeemedAt ? "redeemed"
                           : "available";

                // ── Is a sign-out waiting to be claimed? ──
                //
                // `device_release.pb.js` writes `released_at`, and
                // `activation.pb.js` refuses a redeemed code unless `released_at`
                // is LATER than `activated_at`. Until this existed the console
                // could not tell the two post-sign-out states apart, and they
                // mean opposite things to an operator:
                //
                //   released   — the code is FREE. The student's next activation
                //                claims it, and a code sitting released is one
                //                whose holder is between machines.
                //   redeemed   — in force on a machine. `release_count > 0` now
                //                means "moved machines at some point", not
                //                "shared", which is the whole change: a second
                //                device is REFUSED while this is the state.
                //
                // Same comparison as the two hooks, byte for byte, because the
                // console's answer and the hub's answer must not disagree about
                // whether a student can get in.
                var relAt2 = rec.get("released_at") || "";
                var isReleased2 = status === "redeemed"
                    && parsePBDate(relAt2) > 0
                    && parsePBDate(relAt2) > parsePBDate(redeemedAt);

                // The window filter. A code must have a parsable expiry in the
                // FUTURE and within the window — so already-lapsed codes are
                // absent (they are a different problem: the student has already
                // lost access and needs a catch-up, not a reminder).
                if (useWindow) {
                    if (excludeSuspended && susp2) continue;
                    if (!expMs2 || isNaN(expMs2)) continue;
                    if (expMs2 <= nowMs) continue;
                    if (expMs2 > windowEndMs) continue;
                    // Only codes somebody HOLDS belong in a renewal queue.
                    //
                    // An unsold card carries a mint-time `expires_at` and cannot
                    // lapse: there is no student to lose access, and nothing to
                    // collect. It was included, so the queue held stock — which
                    // is also why `dashboard.expiringSoon` (which now applies the
                    // same rule) and this list disagreed.
                    // server/scripts/console-card-consistency.js is the guard.
                    if (!redeemedAt) continue;
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
                    // Whether a sign-out is waiting to be claimed. The row is
                    // "redeemed" either way — the difference is whether the next
                    // machine can get in, which is the operator's actual question.
                    released_pending: isReleased2,
                    // Whether this code has been redeemed. No device is named:
                    // a code is single-use, not device-bound.
                    redeemed: !!redeemedAt,
                    suspended: susp2,
                    expires_at: expRaw ? String(expRaw) : "",
                    activated_at: redeemedAt,
                    // ── What the code has DONE, as opposed to what it is now ──
                    //
                    // These three exist because `status: "available"` was
                    // ambiguous in the one case an operator most needs it. A
                    // released code and a freshly minted one both read
                    // "available", and only one of them has already been handed
                    // to a paying student — which is how the same code gets sold
                    // twice. `first_seen_at` survives a release, so it is the
                    // distinction; `released_at`/`release_count` are the
                    // student's own sign-outs (see device_release.pb.js).
                    //
                    // `release_count > 0` is the answer to "has this student
                    // been moving machines, or is this key being shared?" — the
                    // question the sign-out endpoint was built to answer and
                    // that no console view was showing.
                    first_seen_at: rec.getString("first_seen_at") || "",
                    first_seen_fingerprint: rec.getString("first_seen_fingerprint") || "",
                    released_at: rec.getString("released_at") || "",
                    release_count: rec.get("release_count") || 0,
                    // ── The LIVENESS axis, which is not the entitlement axis ──
                    //
                    // `last_seen_at` is the only trace a heartbeat leaves (see
                    // heartbeat.pb.js). It is passed through RAW and unjudged:
                    // this hook does not compute "live"/"stale"/"absent",
                    // because a second implementation of the lease arithmetic
                    // is a second thing that can disagree with
                    // session_probe.pb.js. The console classifies it in ONE
                    // place (`views/codes/model.ts: livenessOf`), and §43
                    // asserts the vocabulary matches the hook's.
                    //
                    // EMPTY STRING, NOT A FABRICATED DATE. A code that has never
                    // beaten has no liveness, and the console renders that as
                    // "—" rather than as "away"/"signed out". Turning a missing
                    // value into a definite state is the exact defect this whole
                    // change removes; it must not be reintroduced here.
                    last_seen_at: rec.getString("last_seen_at") || "",
                    // ── The OVERRIDE, which is not a second status ──
                    //
                    // Passed through RAW and unjudged, like `last_seen_at`: the
                    // console decides how to render it, and there is exactly one
                    // classifier on the client (§58/§59 guard the liveness one).
                    //
                    // `evict_requested_at` is the eviction predicate the heartbeat
                    // reads. Its presence means "a live device is being cut off",
                    // which is different from "released" and must be shown beside
                    // it, not instead of it.
                    force_state: forceState,
                    evict_requested_at: evictAt,
                    evict_reason: rec.getString("evict_reason") || "",
                    override_id: rec.getString("override_id") || "",
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
                // Counts for the filter bar. `facets.tier` is keyed by the
                // stored tier string, so the console renders only tiers the hub
                // actually holds rather than a literal list that can drift from
                // the seed — the failure check-consistency.sh §27(f) exists to
                // catch.
                facets: { status: facetStatus, tier: facetTier },
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.import — add codes that are MISSING from the hub
        // ─────────────────────────────────────────────────────────────
        //
        // WHY THIS EXISTS
        //
        // A code that the hub does not hold is not a cosmetic gap. heartbeat.pb.js
        // answers 404 "Code not found"; the client files 404 as a REFUSAL, and
        // handle_refusal() stops the tunnel and calls clear_entitlement(), which
        // DELETES the code from the device and its durable mirror. The student is
        // left logged in with no tunnel and no code, and no error is ever shown —
        // because the state that would have explained it was erased a beat
        // earlier. The code set and the hub disagreeing is what makes that
        // reachable, so reconciling them has to be an operation the console can
        // do idempotently, not a hand-written INSERT.
        //
        // THREE OUTCOMES, REPORTED SEPARATELY. "skipped" is not "created": a code
        // already on the hub may carry real history (activated_at, events), and
        // an import must never overwrite that. Use codes.renew / codes.update for
        // changes to an existing row.
        //
        //   created    — the row did not exist and was added
        //   skipped    — already present; left byte-for-byte alone
        //   remapped   — created, but the requested tier has no config row, so it
        //                was written under `remap_to` (the caller decides this)
        //   rejected   — would not be servable; NOT written. The reason is
        //                returned per code so the operator can act on it rather
        //                than discovering a dead batch weeks later.
        //
        // THE TIER GUARD IS THE POINT OF THE GUARD. activation.pb.js returns the
        // stored `tier` to the client, and the client resolves server_config by
        // that string alone. A tier with no active tier_configs row therefore
        // activates "successfully" and hands the device NO server_config — a
        // connected app that cannot reach the internet, which is the failure mode
        // this whole handler is trying to remove. So a code is only written for a
        // tier that can actually be served, and `stealth` (retired, merged into
        // `strike`, 04-tiers.md §4.2.4) is remapped by the caller, never silently.
        //
        // Luhn is re-checked here for the same reason: formatCode() enforces it on
        // generation, but an import accepts externally-supplied strings, and a
        // malformed code is rejected by code_lookup.pb.js before it ever reaches
        // the database — it would look present in the console and be unusable on
        // the device.
        if (action === "codes.import") {
            var items = body.codes;
            if (!items || typeof items.length !== "number") return bad(400, "codes must be an array");
            if (items.length === 0) return bad(400, "codes is empty");
            if (items.length > 500) return bad(400, "at most 500 codes per import");

            var remapTo = String(body.remap_to || "").trim();
            var skipDup = body.on_duplicate !== "error";

            // Which tiers can actually be served right now? Read once, not per code.
            var servable = {};
            var tierRows = $app.dao().findRecordsByExpr("tier_configs", $dbx.exp("id != ''"));
            for (var ti = 0; ti < tierRows.length; ti++) {
                if (tierRows[ti].getBool("active")) servable[tierRows[ti].getString("tier")] = true;
            }
            if (remapTo && !servable[remapTo]) {
                return bad(400, "remap_to '" + remapTo + "' has no active tier config — it would be unservable too");
            }

            var collI = $app.dao().findCollectionByNameOrId("codes");
            var created = [], skipped = [], remapped = [], rejected = [];

            // De-duplicate WITHIN the request first. codes.json is hand-maintained
            // and has shipped the same code twice; without this the second copy
            // would be reported as rejected-by-unique-constraint, which reads like
            // a database fault rather than a duplicate in the source list.
            var seen = {};

            for (var k = 0; k < items.length; k++) {
                var entry = items[k] || {};
                var rawCode = String(entry.code || "").trim().toUpperCase();
                var wantTier = String(entry.tier || "").trim();

                if (!rawCode) { rejected.push({code: "(empty)", reason: "no code"}); continue; }

                var canon = canonical(rawCode);
                var flat = canon.replace(/-/g, "");
                if (flat.length !== 15) {
                    rejected.push({code: rawCode, reason: "not in RQ-XXXX-XXXX-XXXX-C form"});
                    continue;
                }
                var bodyPart = flat.substring(0, 14);
                if (checksum(bodyPart) !== flat.substring(14)) {
                    rejected.push({code: canon, reason: "checksum does not match (Luhn)"});
                    continue;
                }
                if (seen[canon]) { skipped.push(canon); continue; }
                seen[canon] = true;

                var useTier = wantTier;
                var wasRemapped = false;
                if (!servable[useTier]) {
                    if (!remapTo) {
                        rejected.push({code: canon, reason: "tier '" + useTier + "' has no active tier config"});
                        continue;
                    }
                    useTier = remapTo;
                    wasRemapped = true;
                }

                var exists = null;
                try { exists = $app.dao().findFirstRecordByData("codes", "code", canon); } catch (nfI) { exists = null; }
                if (exists) {
                    if (!skipDup) { rejected.push({code: canon, reason: "already exists"}); continue; }
                    skipped.push(canon);
                    continue;
                }

                try {
                    var nri = new Record(collI);
                    nri.set("code", canon);
                    nri.set("tier", useTier);
                    nri.set("used", false);
                    nri.set("suspended", false);
                    nri.set("notes", String(entry.notes || ""));
                    if (entry.label) nri.set("label", String(entry.label));
                    if (entry.middleman) nri.set("middleman", String(entry.middleman));
                    if (entry.expires_at) nri.set("expires_at", String(entry.expires_at));
                    $app.dao().saveRecord(nri);
                    if (wasRemapped) {
                        remapped.push(canon);
                        // The remap is recorded as its own event, because "why is
                        // this card strike when it was sold as stealth" is a
                        // question someone will ask, and the answer must not live
                        // only in a chat log.
                        logEvent(canon, "generated",
                                 "imported from code set; tier remapped '" + wantTier + "' -> '" + useTier + "'", "");
                    } else {
                        created.push(canon);
                        logEvent(canon, "generated", "imported from code set (" + useTier + ")", "");
                    }
                } catch (saveI) {
                    rejected.push({code: canon, reason: "save failed: " + String(saveI)});
                }
            }

            return ok({
                created: created,
                skipped: skipped,
                remapped: remapped,
                rejected: rejected,
                servable_tiers: Object.keys(servable),
                remap_to: remapTo || null,
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
            // ── Clearing the reason matters as much as setting it ──
            //
            // `suspended_reason` is shown to the STUDENT on the kick-out screen,
            // so leaving it behind after an unsuspend would keep a reinstated
            // student on a page explaining why they were cut off. Cleared with the
            // flag, in the same write.
            setIfPresent(recS, "suspended_reason", wantSuspend ? String(body.reason || "").trim() : "");
            $app.dao().saveRecord(recS);
            logEvent(target, wantSuspend ? "suspended" : "unsuspended",
                     String(body.reason || ""), "");
            var replyS = ok({code: target, suspended: wantSuspend});
            // ── Unsuspend does NOT clear an override, and must say so ──
            //
            // `suspended` and the override are independent: a code can be
            // "released by override" AND run through this action. Clearing the
            // suspension alone leaves `evict_requested_at` set, so the device is
            // refused again at its next beat. That is correct — the code IS still
            // released — but an operator who meant "let them back in" would read
            // the 200 as success and then see the eviction persist. Warn instead
            // of clearing, because silently clearing would undo a release nobody
            // asked to undo.
            if (!wantSuspend && String(recS.get("evict_requested_at") || "")) {
                replyS.warning = "This code is still released by override, so the device "
                    + "will be refused again at its next check-in. Use codes.reactivate to undo the release.";
            }
            return replyS;
        }

        // ─────────────────────────────────────────────────────────────
        // codes.kick — suspend, with a reason the student is shown
        //
        // ── Why this exists when `codes.suspend` already does ──
        //
        // The operator's question was "how do I actually kick someone out?". The
        // answer used to be "suspend, and wait up to five minutes" — and when it
        // worked, the student found out by opening an app that had quietly lost
        // its button, with no explanation anywhere.
        //
        // So this is not a second way to suspend. It is the same state change
        // with the two things the operator was missing:
        //
        //   1. A REASON the student will see, verbatim, on the kick-out screen.
        //      Free text, because the honest answer varies ("chargeback", "key
        //      shared with three devices", "mistaken suspension, sorry").
        //   2. An audit row that says a human decided this, not a lapsed date.
        //
        // The client half is already there: `suspended` is refused on the
        // heartbeat, the client keeps its code (so this is REVERSIBLE), shows the
        // kick-out screen, and is returned to normal by an unsuspend with no
        // re-entry and no restart.
        //
        // Rate-limit safety: nothing here is reachable by a student. The action
        // requires the admin token like every other console route.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.kick") {
            var targetK = canonical(body.code);
            if (!targetK) return bad(400, "code is required");
            var reasonK = String(body.reason || "").trim();
            // Refused rather than defaulted. A kick-out with no reason leaves the
            // student on a screen that says "contact support" about a decision
            // nobody wrote down, which is the support conversation this exists to
            // avoid — and it cannot be reconstructed later from the audit row
            // because the student has already asked.
            if (!reasonK) return bad(400, "a reason is required — the student is shown it");
            if (reasonK.length > 200) {
                return bad(400, "keep the reason under 200 characters; it is displayed in full");
            }
            var recK = null;
            try { recK = $app.dao().findFirstRecordByData("codes", "code", targetK); } catch (nfK) { recK = null; }
            if (!recK) return bad(404, "Code not found");

            recK.set("suspended", true);
            setIfPresent(recK, "suspended_reason", reasonK);
            $app.dao().saveRecord(recK);
            logEvent(targetK, "kicked", reasonK, "");

            // The student does not learn this until their client's next beat.
            // Said plainly rather than implied, because an operator who expects
            // the tunnel to drop instantly will otherwise conclude it failed — and
            // the honest bound is the heartbeat interval, not "immediately".
            return ok({
                code: targetK,
                suspended: true,
                reason: reasonK,
                note: "The device is cut off at its next heartbeat (within about five "
                    + "minutes while connected). Its code is kept, so unsuspending "
                    + "restores access without the student re-entering anything.",
            });
        }

        // ─────────────────────────────────────────────────────────────
        // codes.release — release a redeemed code back to unused
        //
        // Formerly `codes.unbind`, which is kept as an alias below because the
        // `codes.rebind` refusal names it in prose and scripts may call it.
        //
        // There is no device binding any more (see activation.pb.js): the action
        // is to clear the single-use stamp so the code can be activated again —
        // by the same student on a new machine, or handed to a different one.
        //
        // ── WHY THIS NOW CARRIES AN EVICTION ──
        //
        // Releasing a code that a device is STILL USING used to leave the two
        // records disagreeing forever: `activated_at` was cleared, and the live
        // client's next heartbeat never read it, got 200, and kept serving a code
        // the hub considered free. That was a silent bug, not a feature — so a
        // release of a live code now also asks the heartbeat to cut the device
        // off, and records what it did.
        //
        // ── THE ONE THING THIS MUST NOT DO ──
        //
        // It must NOT set `codes.suspended`. That column is read by
        // activation.pb.js (:351, :381), so "evict" expressed as a suspension
        // would refuse the NEXT holder and break the resale this action exists
        // for. The eviction predicate is `evict_requested_at`, which only the
        // heartbeat reads. The student's kick-out screen still says "suspended"
        // on the wire, because that is the only refusal deployed clients treat
        // as reversible — but that is a RESPONSE field, not this column.
        //
        // See docs/reference/OVERRIDES.md for the sequence and the failure modes.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.release" || action === "codes.unbind") {
            var targetU = canonical(body.code);
            if (!targetU) return bad(400, "code is required");
            var recU = null;
            try { recU = $app.dao().findFirstRecordByData("codes", "code", targetU); } catch (nf3) { recU = null; }
            if (!recU) return bad(404, "Code not found");

            var prevAt = recU.getString("activated_at") || "";
            // ── ALREADY RELEASED IS NOT AN ERROR, AND MUST NOT CLAIM OTHERWISE ──
            //
            // This used to return "That code has not been used yet" whenever
            // `activated_at` was empty — including for a code that WAS used and
            // has already been released. Two things were wrong with that:
            //
            //   * the sentence is false, and an operator reading it concludes the
            //     code was never sold when it was;
            //   * a second operator (or a double-click, or a retried request)
            //     releases the same code, gets a 400 that reads like a bug, and
            //     has no way to tell "already done" from "failed".
            //
            // A release is a SET operation: "make this code not-held". Asking for
            // it twice is not an error, it is the same request. So a code with no
            // live claim is answered with what is already true, and says so — the
            // console renders a replay as a no-op rather than a failure.
            if (!prevAt) {
                var alreadyReleased = String(recU.get("force_state") || "") === "released"
                    || !!String(recU.get("unbound_at") || "");
                if (alreadyReleased || String(recU.get("first_seen_at") || "") === "") {
                    // Distinguish the two truthful cases. A code never sold has
                    // nothing to release; a released code is already as asked.
                    if (alreadyReleased) {
                        return ok({
                            code: targetU,
                            replay: true,
                            already_released: true,
                            observed_state: livenessForConfirmation(recU),
                            evicted: !!String(recU.get("evict_requested_at") || ""),
                            outcome: "already_released",
                            consequences: ["This code was already released. Nothing changed."]
                        });
                    }
                    return bad(400, "That code has never been issued, so there is nothing to release");
                }
                // Issued once, claim already spent by a sign-out: releasing is a
                // no-op on the claim but still records the operator's intent.
                // Fall through — the write below is harmless and idempotent.
            }

            var reason = String(body.reason || "Released from console").trim();
            if (reason.length > 200) {
                return bad(400, "keep the reason under 200 characters; it is shown to the student");
            }
            // `codes.unbind` predates the idempotency key and must keep working
            // for existing callers, so a missing id is generated rather than
            // refused. The console always sends one.
            var ovId = String(body.override_id || "").trim();
            if (!ovId) ovId = "unbind-" + targetU + "-" + nowISO();

            // WHERE THE DECISION IS MADE. "live" and "stale" both warrant an
            // eviction: a stale device is within the 7-day window and may come
            // back at any beat. "absent"/"unknown" do NOT — a code nobody has
            // touched in a week is very likely genuinely free, and a false
            // suspension on a code we are about to resell outlives the mistake.
            var observed = livenessForConfirmation(recU);
            var willEvict = (observed === "live" || observed === "stale");

            // The consequence list is computed from the ROW, before any write, so
            // `dry_run` and the real call describe the same thing by construction.
            var consequences = [
                "The single-use claim is cleared: this code becomes available to a new holder.",
                "Its tier and expiry are NOT changed."
            ];
            if (observed === "unknown") {
                consequences.push("Observed: no check-in on record, so this device may be offline or may never have beaten.");
            } else {
                consequences.push("Observed: " + observed + " (last check-in " + String(recU.get("last_seen_at") || "").substring(0, 16) + ").");
            }
            if (willEvict) {
                consequences.push("That app will be cut off at its next check-in — within about five minutes while connected.");
                // Quote what the device ACTUALLY renders. The first version quoted
                // "This code was released by your provider —", which is not what
                // the heartbeat sends (it must lead with "Account suspended" so
                // deployed clients take the reversible branch). A preview that
                // misquotes the student-facing text is a desync between what the
                // operator confirms and what the student sees.
                consequences.push("It will see: \"Account suspended — this code was released: " + reason + "\".");
                consequences.push("Its current connection drops; in-flight traffic is interrupted, not drained.");
                // ── THE WORST CASE, STATED, BECAUSE IT OTHERWISE LOOKS LIKE A FAILURE ──
                //
                // A device that cannot reach the hub at all keeps its tunnel for
                // the 7-day grace period, because nothing on the hub can be
                // delivered to a client that is not asking. An operator who is not
                // told this watches a released code keep working and concludes the
                // release did not take. Stating the bound is what makes the two
                // distinguishable: "not yet told" is not "not applied".
                consequences.push("A device that cannot reach the hub at all keeps working for up to 7 days (its offline grace), then stops. This only applies to an unresponsive device — it is refused at the first check-in that reaches us.");
            } else {
                consequences.push("Nothing is cut off. If that device reappears later it will not be refused by this release, because it was not considered live when you released it.");
            }
            consequences.push("This is recorded as an override, with the observation above.");

            if (body.dry_run === true || body.dry_run === "true") {
                return ok({
                    code: targetU,
                    dry_run: true,
                    observed_state: observed,
                    will_evict: willEvict,
                    severity: willEvict ? "evicts-live-client" : "release-only",
                    consequences: consequences,
                    // A token the caller echoes on the commit. It is the claim the
                    // preview was computed against; if it has moved by the time the
                    // operator confirms, the server must NOT act on a decision made
                    // about a different state. Returned even on a dry run so the
                    // drawer can hold it between preview and submit.
                    claim_token: String(recU.get("activated_at") || "")
                });
            }

            // ── The preview and the commit must describe the SAME state ──
            //
            // Between an operator reading the consequences and pressing the button,
            // another tab (or another operator) can release, suspend or renew this
            // code. Without this check the release applies to a state the operator
            // never saw — the "I confirmed something else" failure. The console
            // sends the `claim_token` the preview returned; a mismatch is a 409 that
            // says exactly what moved, and the drawer re-previews instead of
            // silently committing.
            //
            // Only enforced when the caller sends a token, so `codes.unbind` (which
            // predates this and sends none) is unaffected.
            var sentToken = body.claim_token;
            if (sentToken !== undefined && sentToken !== null
                    && String(sentToken) !== String(recU.get("activated_at") || "")) {
                return bad(409, "This code changed while you were confirming — it is no longer in the state you reviewed. Re-open the release to see the current consequences.");
            }

            // Idempotent replay: same (code, override_id) → same answer, no
            // second audit row and no second save.
            var prior = findOverride(targetU, ovId);
            if (prior) {
                return ok({
                    code: targetU,
                    replay: true,
                    previous_fingerprint: prevAt.substring(0, 16),
                    observed_state: String(prior.get("observed_state") || "unknown"),
                    outcome: String(prior.get("outcome") || "applied"),
                    consequences: consequences
                });
            }

            var fromValue = "redeemed@" + prevAt.substring(0, 19);
            var toValue = willEvict ? "released+evict" : "released";

            // HARD FAILURE POINT. If this throws, the handler's catch turns it
            // into a 500 and NOTHING below runs — no state change without an
            // audit row, which is the whole reason this is not logEvent.
            writeOverride({
                code: targetU,
                override_id: ovId,
                aspect: "release",
                from_value: fromValue,
                to_value: toValue,
                reason: reason,
                actor: String(body.actor || "console"),
                observed_state: observed,
                observed_at_override: String(recU.get("last_seen_at") || ""),
                outcome: willEvict ? "applied_with_eviction" : "applied"
            });

            // ONE save for every field below: two saves would be two chances to
            // record only one of them (the `first_seen_at` precedent in
            // activation.pb.js).
            recU.set("activated_at", null);
            setIfPresent(recU, "unbound_at", nowISO());
            setIfPresent(recU, "unbind_reason", reason);
            setIfPresent(recU, "force_state", "released");
            setIfPresent(recU, "override_id", ovId);
            if (willEvict) {
                setIfPresent(recU, "evict_requested_at", nowISO());
                setIfPresent(recU, "evict_reason", reason);
            }
            // `first_seen_at` is DELIBERATELY left alone. Releasing a code puts
            // it back into the pool for a different student, and that is the
            // operator's decision — but it does not un-issue it. Erasing the
            // first-redemption marker here would make a re-issued code
            // indistinguishable from a fresh one, and the operator who released
            // it would lose the only record that it had ever been sold.
            $app.dao().saveRecord(recU);

            logEvent(targetU, willEvict ? "released+evicted" : "released", reason, prevAt.substring(0, 16));

            var reply = ok({
                code: targetU,
                // The key is KEPT for the console, which has rendered it since
                // before this change; it carries the redemption time because no
                // device exists to name.
                previous_fingerprint: prevAt.substring(0, 16),
                observed_state: observed,
                evicted: willEvict,
                outcome: willEvict ? "applied_with_eviction" : "applied",
                override_id: ovId,
                consequences: consequences
            });
            if (willEvict) {
                reply.note = "The device is cut off at its next heartbeat (within about five "
                    + "minutes while connected). Use codes.reactivate to undo this.";
            }
            return reply;
        }

        // ─────────────────────────────────────────────────────────────
        // codes.reactivate — undo an override
        //
        // The counterpart to `codes.release`: clears the override fields, and the
        // derived status snaps back to whatever the underlying fields say. That
        // is the property the "override is an input, not a status" design buys —
        // undo is a delete, not a reconstruction.
        //
        // What it deliberately does NOT do: it does not restore `activated_at`.
        // If the code was re-redeemed by a different student between the release
        // and the undo, that redemption is a real fact and must survive — undo
        // removes the EVICTION and the override PIN, never someone's claim.
        //
        // The evicted device is still beating every two minutes (the client keeps
        // the code on a suspension-shaped refusal), so clearing the predicate puts
        // it straight back in at its next beat with no re-entry and no restart.
        // ─────────────────────────────────────────────────────────────
        if (action === "codes.reactivate") {
            var targetR2 = canonical(body.code);
            if (!targetR2) return bad(400, "code is required");
            var recR2 = null;
            try { recR2 = $app.dao().findFirstRecordByData("codes", "code", targetR2); } catch (nfR2) { recR2 = null; }
            if (!recR2) return bad(404, "Code not found");

            var hadEvict = !!String(recR2.get("evict_requested_at") || "");
            var hadForce = String(recR2.get("force_state") || "");
            if (!hadEvict && !hadForce) {
                return bad(400, "That code has no override to undo");
            }
            var reasonR2 = String(body.reason || "Override reversed from console").trim();
            if (reasonR2.length > 200) {
                return bad(400, "keep the reason under 200 characters");
            }
            var ovId2 = String(body.override_id || "").trim();
            if (!ovId2) ovId2 = "reactivate-" + targetR2 + "-" + nowISO();
            var priorR2 = findOverride(targetR2, ovId2);
            if (priorR2) {
                return ok({code: targetR2, replay: true});
            }

            writeOverride({
                code: targetR2,
                override_id: ovId2,
                aspect: "reactivate",
                from_value: (hadForce || "none") + (hadEvict ? "+evict" : ""),
                to_value: "none",
                reason: reasonR2,
                actor: String(body.actor || "console"),
                observed_state: livenessForConfirmation(recR2),
                observed_at_override: String(recR2.get("last_seen_at") || ""),
                outcome: "applied"
            });

            // Cleared in ONE save, together, so a crash cannot leave "released"
            // without its eviction or the reverse.
            setIfPresent(recR2, "force_state", "");
            setIfPresent(recR2, "evict_requested_at", null);
            setIfPresent(recR2, "evict_reason", "");
            setIfPresent(recR2, "override_id", "");
            $app.dao().saveRecord(recR2);
            logEvent(targetR2, "reactivated", reasonR2, "");
            return ok({code: targetR2, evicted_cleared: hadEvict, force_state_cleared: hadForce !== ""});
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
        // Also does NOT touch the single-use state or `tier`. Renewal extends
        // time; it does not change what was bought or whether the code is in use.
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
            // An omitted `middleman` falls back to the code's own, because the
            // two are the same fact: the code carries who distributed it, and a
            // renewal for that code is that middleman's sale. Without the
            // fallback, a renewal recorded from a screen that does not show the
            // field loses the attribution — which is the ONE thing a ledger
            // cannot recover later (docs/business/redesign/08-metrics §8.4). An
            // explicit `middleman` still wins, for the rare case where a code
            // has been handed on to someone else.
            var mm = String(body.middleman || "").trim() || String(recR.getString("middleman") || "").trim();
            var detail = (prevValid ? String(prevRaw) : "(none)") + " → " + newIso +
                         " (term_days=" + termDays +
                         (price ? ", price=" + price : "") +
                         (mm ? ", middleman=" + mm : "") + ")";
            logEvent(targetR, "renewed", detail, "");

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
            // REMOVED with the device binding. A code is no longer tied to a
            // machine, so there is nothing to move it between: a student who
            // changes machine just activates the code again, and it restores
            // because re-activating a redeemed code is allowed (see
            // activation.pb.js). The action is kept as an explicit refusal
            // rather than deleted, so an operator following an old runbook gets a
            // sentence explaining the change instead of a silent no-op.
            return bad(410,
                "Rebinding is no longer needed: codes are not tied to a device. Your " +
                "student can simply enter the code again on the new machine — it will " +
                "restore their access. Use codes.unbind only if you need to release the " +
                "code for a different student entirely.");
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
            // WHICH fields changed, not just "changed".
            //
            // The audit row used to read "details changed", which is the one
            // thing an operator already knows. The question actually asked of
            // this trail is "who set this label, and to what" — e.g. a student
            // is unfindable because a label was overwritten in a bulk fix. The
            // field NAMES are recorded; the values are not, because `label` is
            // the student's name and `notes` is free text (see
            // docs/operate/CLAIMS.md §4: the trail is not a second copy of the
            // data, and it outlives the row it describes).
            var changedUp = [];
            if (body.middleman !== undefined) { recUp.set("middleman", String(body.middleman || "").trim()); changedUp.push("middleman"); }
            if (body.label !== undefined) { recUp.set("label", String(body.label || "").trim()); changedUp.push("label"); }
            if (body.notes !== undefined) { recUp.set("notes", String(body.notes || "").trim()); changedUp.push("notes"); }
            $app.dao().saveRecord(recUp);
            logEvent(targetUp, "updated",
                     changedUp.length ? "changed: " + changedUp.join(", ") : "no fields changed",
                     "");
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
        //   * a REDEEMED code is refused — a student is using it. Release it
        //     first, so letting a code go is a separate, deliberate step.
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
            var dRedeemed = recD.getString("activated_at") || "";
            // ── A RELEASED CODE IS STILL "USED", AND DELETING IT IS A DATA LOSS ──
            //
            // This used to refuse only while `activated_at` was set, and told the
            // operator to "Release it first". After a release `activated_at` is
            // null, so the guard stopped protecting the row — and worse, the
            // message actively guided the operator toward releasing a sold code
            // just to delete it, which throws away the only record that it was ever
            // sold (`first_seen_at`) along with its audit trail.
            //
            // A code that has been issued is one a student paid for. It stays
            // protected whether or not it currently has a holder, and whether or
            // not it has been released, renewed or overridden. Test rows and
            // typos never had a holder and are untouched by this.
            var dIssued = !!String(recD.get("first_seen_at") || "") || !!dRedeemed
                || !!String(recD.get("unbound_at") || "");
            if (body.force_delete_bound !== true && dIssued) {
                return bad(409, "That code has been issued to a student. Deleting it destroys the record "
                    + "that it was ever sold. Pass force_delete_bound to delete it anyway.");
            }
            logEvent(targetD, "deleted",
                     "tier=" + recD.getString("tier") + " middleman=" + recD.getString("middleman") +
                     (dRedeemed ? " was-redeemed=" + dRedeemed.substring(0, 16) : ""),
                     "");
            // No binding index to release any more. The old code had to release
            // one here or a device would be permanently unable to activate again
            // (the uniqueness check kept seeing a row pointing at a deleted code);
            // there is nothing equivalent now, which is one less way a deletion
            // could have bricked a machine.
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
                // A redeemed code is always skipped in a batch: the whole point
                // of the batch path is cleanup, and silently throwing away a
                // code a student is actively using is the failure this guards
                // against. Release it deliberately first if that is intended.
                if (rd.getString("activated_at")) { skipped.push({code: cd, why: "already used by a student"}); continue; }
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
        // middlemen.summary — attribution AND remittance, per middleman
        //
        // WHY THIS EXISTS
        //
        // `middlemen.list` answers "how many codes carry this name", which was
        // sufficient while revenue was derivable from a code count. Renewal
        // breaks that: one code sold once can now be paid for five times, so
        // "how many did Sarah sell" and "how much did Sarah collect" stopped
        // being the same question (docs/business/07-billing.md §7.4). Both the
        // billing file and the console design record (06-console §6.6) name the
        // missing per-middleman view; this is it, built on data that already
        // exists. No schema change: the money comes out of the `price=` token
        // `codes.renew` writes into `code_events.detail`.
        //
        // THE PARSING IS DELIBERATELY TOLERANT. The detail string is free text
        // written by previous versions of this file, so a row may carry no
        // price at all (renewal from before the capture landed, or an operator
        // who left it blank). Those parse to "no money recorded" rather than to
        // zero — the console shows a dash for them, because a zero in a money
        // column reads as "collected $0", which is a different and much worse
        // claim than "not recorded".
        //
        // STOCK CLASSES, not a second list of middlemen. 08-distribution §8.5
        // makes free stock a different thing commercially (no cut, operator-
        // given, foot-in-the-door) while remaining the same entity
        // operationally, so it is a segment here rather than a separate page —
        // splitting it would reintroduce the Tiers problem.
        if (action === "middlemen.summary") {
            // The window for "renewals" and "collected". Defaults to 30 days,
            // because the natural question is "this month" and a rolling window
            // needs no month-boundary handling.
            var windowDaysM = Number(body.window_days);
            if (!isFinite(windowDaysM) || windowDaysM <= 0) windowDaysM = 30;
            if (windowDaysM > 3650) return bad(400, "window_days must be 3650 or less");
            var sinceMs = Date.now() - (windowDaysM * 24 * 60 * 60 * 1000);

            var allM = $app.dao().findRecordsByExpr("codes", $dbx.exp("id != ''"));
            var nowM = Date.now();
            var in7 = nowM + (7 * 24 * 60 * 60 * 1000);
            var byName = {};

            // NOTE: helper declarations are repeated inside each `if` block on
            // purpose — goja does not hoist a function declaration past the
            // enclosing block to the routerAdd callback (the "helperFn is not
            // defined" trap, FIXES.md 2026-09-19), and a closure that captures
            // nothing is cheap.
            var bucketFor = function(nm) {
                if (!byName[nm]) {
                    byName[nm] = {
                        name: nm,
                        codes: 0,           // every code carrying this name
                        available: 0,       // minted, never redeemed
                        in_term: 0,         // redeemed and still running
                        due_7d: 0,          // in term, inside the 7-day worklist
                        suspended: 0,
                        expired: 0,
                        released_once: 0,   // handed out then released: resale risk
                        free_codes: 0,      // a segment, not a second page
                        paid_codes: 0,
                        renewals: 0,        // in the window
                        collected: 0,       // in the window, priced renewals only
                        priced_renewals: 0, // how many of `renewals` carried a price
                    };
                }
                return byName[nm];
            };

            for (var mi = 0; mi < allM.length; mi++) {
                var cm = allM[mi];
                // Unattributed stock gets its own bucket rather than being
                // dropped: a code with no middleman is the operator's own, and
                // hiding it would make this page's totals disagree with the
                // Overview's without either being obviously wrong.
                var nmM = String(cm.getString("middleman") || "").trim() || "(unattributed)";
                var bM = bucketFor(nmM);
                bM.codes++;
                if (String(cm.getString("tier") || "") === "free") bM.free_codes++;
                else bM.paid_codes++;

                var expM = parsePBDate(cm.get("expires_at"));
                var hasExpM = expM && !isNaN(expM);
                var lapsedM = hasExpM && expM < nowM;
                var redeemedM = !!String(cm.getString("activated_at") || "");

                if (cm.getBool("suspended")) bM.suspended++;
                else if (lapsedM) bM.expired++;
                else if (redeemedM) {
                    bM.in_term++;
                    if (hasExpM && expM <= in7) bM.due_7d++;
                } else bM.available++;
                if (String(cm.getString("first_seen_at") || "") && !redeemedM) bM.released_once++;
            }

            // The money. One pass over the renewals in the window; the audit
            // trail is a ledger's only source until a price column exists.
            var evsM = $app.dao().findRecordsByExpr("code_events", $dbx.exp("event = {:e}", { e: "renewed" }));
            for (var ei = 0; ei < evsM.length; ei++) {
                var evM = evsM[ei];
                var whenM = parsePBDate(evM.get("created"));
                if (isNaN(whenM) || whenM < sinceMs) continue;
                var detM = String(evM.getString("detail") || "");
                var mmMatch = detM.match(/middleman=([^,)]+)/);
                var nmE = mmMatch ? String(mmMatch[1]).trim() : "";
                if (!nmE) {
                    // Fall back to the code's own attribution when the event
                    // recorded none — a renewal from before the capture landed.
                    var rcM = null;
                    try { rcM = $app.dao().findFirstRecordByData("codes", "code", String(evM.getString("code") || "")); } catch (nxM) { rcM = null; }
                    nmE = rcM ? String(rcM.getString("middleman") || "").trim() : "";
                }
                if (!nmE) nmE = "(unattributed)";
                var bE = bucketFor(nmE);
                bE.renewals++;
                var prMatch = detM.match(/price=([0-9]+(\.[0-9]+)?)/);
                if (prMatch) {
                    var amountM = Number(prMatch[1]);
                    if (isFinite(amountM)) { bE.collected += amountM; bE.priced_renewals++; }
                }
            }

            var outM = [];
            for (var kM in byName) {
                var bO = byName[kM];
                // A money total is only a total if EVERY renewal in the window
                // carried a price. Otherwise it is a floor, and saying which is
                // the difference between a report and a plausible-looking lie.
                bO.collected_complete = bO.renewals > 0 && bO.priced_renewals === bO.renewals;
                bO.collected = Math.round(bO.collected * 100) / 100;
                outM.push(bO);
            }
            outM.sort(function(a, b) {
                // Busiest first: the operator's attention follows stock, not the
                // alphabet.
                if (b.codes !== a.codes) return b.codes - a.codes;
                return a.name < b.name ? -1 : (a.name > b.name ? 1 : 0);
            });
            return ok({middlemen: outM, window_days: windowDaysM, since: new Date(sinceMs).toISOString()});
        }

        // ─────────────────────────────────────────────────────────────
        // attention.list — the checks the Overview strip runs
        //
        // Each is a FILTER over fields `codes.list` already returns. It lives
        // server-side so the console gets counts plus a few examples instead of
        // every row, and so one definition of each check serves every surface
        // that shows it.
        //
        // Every check answers "what is wrong that nobody has looked at", and
        // each is backed by a failure this project has already paid for:
        //
        //   unservable_tier — a code whose tier has no ACTIVE tier_configs row.
        //     activation.pb.js returns the stored tier and the client resolves
        //     server_config by that string alone, so such a code activates
        //     "successfully" and hands the device nothing: a connected app that
        //     reaches no internet. It is the exact failure `codes.import`
        //     guards against, and nothing was watching for it afterwards.
        //
        //   lapsed — past `expires_at`, not suspended. The 7-day worklist
        //     EXCLUDES these by design (it is for codes still running), so
        //     without this bucket a student who has already lost access is
        //     invisible rather than being a catch-up call.
        //
        //   shared — this one CHANGED MEANING when activation started refusing a
        //     second device, and leaving the old rule in place would have made it
        //     lie in the most reassuring direction.
        //
        //     It used to be `release_count >= 2` while in term, read as "moving
        //     machines, or sharing this key?" — two readings it could not tell
        //     apart, because a sign-out recorded nothing and a second device was
        //     admitted silently. A careless holder therefore left exactly the same
        //     trail as an honest student on their third laptop: no trace at all.
        //
        //     A second device is now REFUSED (activation.pb.js answers 409), so a
        //     code in force on a machine cannot be in use anywhere else. The
        //     question the check can actually answer is the one an operator can act
        //     on: "somebody signed out and has not come back" — a code that is
        //     free, still has term left, and whose holder may be mid-move, or may
        //     have handed their card to someone who has not activated it yet.
        //
        //     Lapsing codes stay in `lapsed`, where they already were.
        //
        //   no_label — redeemed, in term, `label` empty. 08-distribution §8.3.1
        //     makes the label THE link between a student and their code, so
        //     these are support calls waiting to be unanswerable. Not an error:
        //     the gap worth counting.
        if (action === "attention.list") {
            var allA = $app.dao().findRecordsByExpr("codes", $dbx.exp("id != ''"));
            var nowA = Date.now();

            // Which tiers can actually be served right now.
            var served = {};
            var cfgsA = $app.dao().findRecordsByExpr("tier_configs", $dbx.exp("id != ''"));
            for (var ci = 0; ci < cfgsA.length; ci++) {
                if (cfgsA[ci].getBool("active")) served[cfgsA[ci].getString("tier")] = true;
            }

            // A representative handful per check, for the card's preview. 8 is
            // enough to answer "is this the row I think it is" without turning
            // an overview into a list.
            var CAP = 8;
            var groupsA = { unservable_tier: [], lapsed: [], shared: [], no_label: [], gone_quiet: [] };
            var countsA = { unservable_tier: 0, lapsed: 0, shared: 0, no_label: 0, gone_quiet: 0 };

            for (var ai = 0; ai < allA.length; ai++) {
                var ca = allA[ai];
                var codeA = ca.getString("code");
                var expA = parsePBDate(ca.get("expires_at"));
                var hasExpA = expA && !isNaN(expA);
                var lapsedA = hasExpA && expA < nowA;
                var suspA = ca.getBool("suspended");
                var redeemedA = !!String(ca.getString("activated_at") || "");

                var tierA = String(ca.getString("tier") || "");
                // Unservable is counted even on suspended/expired rows: the row
                // would bite the moment either is cleared, and the fix (mint a
                // config row, or remap the code) is the same either way.
                if (tierA && !served[tierA]) {
                    countsA.unservable_tier++;
                    if (groupsA.unservable_tier.length < CAP) {
                        groupsA.unservable_tier.push({ code: codeA, tier: tierA, middleman: ca.getString("middleman") || "" });
                    }
                }
                if (lapsedA && !suspA) {
                    countsA.lapsed++;
                    if (groupsA.lapsed.length < CAP) {
                        groupsA.lapsed.push({
                            code: codeA,
                            label: ca.getString("label") || "",
                            middleman: ca.getString("middleman") || "",
                            days_past: Math.floor((nowA - expA) / (24 * 60 * 60 * 1000)),
                        });
                    }
                }
                // A sign-out that nobody has claimed. See the note on `shared`
                // above for why this replaced a `release_count` threshold.
                var relARaw = ca.get("released_at") || "";
                var relAMs = parsePBDate(relARaw);
                var heldA = !!(ca.getString("activated_at") || "");
                var releasedA = heldA && relAMs > 0 && relAMs > parsePBDate(ca.get("activated_at"));
                if (releasedA && !suspA && !lapsedA) {
                    countsA.shared++;
                    if (groupsA.shared.length < CAP) {
                        groupsA.shared.push({
                            code: codeA,
                            label: ca.getString("label") || "",
                            release_count: Number(ca.get("release_count") || 0),
                            released_at: relARaw ? String(relARaw) : "",
                            expires_at: ca.getString("expires_at") || "",
                        });
                    }
                }
                if (redeemedA && !lapsedA && !suspA && !String(ca.getString("label") || "").trim()) {
                    countsA.no_label++;
                    if (groupsA.no_label.length < CAP) {
                        groupsA.no_label.push({
                            code: codeA,
                            middleman: ca.getString("middleman") || "",
                            activated_at: ca.getString("activated_at") || "",
                        });
                    }
                }

                // ── gone_quiet: in term, has beaten before, has stopped ──
                //
                // The liveness axis, surfaced where an operator looks first.
                // Without this, `last_seen_at` was a column only the Codes table
                // and the probe used, so "how many paying devices have stopped
                // checking in" was a question no screen could answer — and that
                // is the question a hub outage, a bad hook deploy or a blocked
                // school network all present as.
                //
                // STRICTLY requires a PRIOR beat (`seenMs > 0`). A code that has
                // never beaten is `unknown`, not "quiet" — it may be stock nobody
                // has activated, and counting it here would open every fresh
                // import as an emergency. The two are different questions and
                // this check answers only the one with evidence behind it.
                //
                // The threshold is the same lease the console and the probe use,
                // written out in full rather than as `2 * 60 * 60`: the lease is
                // 2h MAX_INTERVAL * 1.1 jitter + 15s timeout + 60s slack, rounded
                // UP to the next whole minute = 2h14m, and the rounded-down
                // version says "quiet" about 14 minutes before the probe does.
                // That is exactly the drift `liveness-consistency.js` §4b exists
                // to catch — it caught this line when it was the round number.
                //
                // Duplicated rather than shared because this hook cannot import
                // from `session_probe.pb.js` (goja hoists nothing across files);
                // §4b evaluates all three and fails when they disagree.
                var seenAMs = parsePBDate(ca.get("last_seen_at"));
                var seenAValid = seenAMs > 0 && !isNaN(seenAMs);
                var goneQuietA = redeemedA && !lapsedA && !suspA && seenAValid && (nowA - seenAMs) > (2 * 60 * 60 + 14 * 60) * 1000;
                if (goneQuietA) {
                    countsA.gone_quiet++;
                    if (groupsA.gone_quiet.length < CAP) {
                        groupsA.gone_quiet.push({
                            code: codeA,
                            label: ca.getString("label") || "",
                            middleman: ca.getString("middleman") || "",
                            last_seen_at: String(ca.getString("last_seen_at")),
                            hours_quiet: Math.floor((nowA - seenAMs) / (60 * 60 * 1000)),
                        });
                    }
                }
            }

            return ok({
                total_codes: allA.length,
                tiers_served: Object.keys(served),
                counts: countsA,
                groups: groupsA,
            });
        }

        // ─────────────────────────────────────────────────────────────
        // device.get / devices.list — REMOVED with the device binding
        //
        // These read the `device_bindings` index, which no longer exists (see
        // activation.pb.js). A code is single-use and not tied to a machine, so
        // there is no per-device view to show and no "one code per device"
        // invariant to audit.
        //
        // Support still needs to answer "is this code in use, and by when does
        // it run out?" — that is `codes.list`, which reports `redeemed` and the
        // expiry per code.
        //
        // Kept as explicit refusals rather than deleted, so an operator (or an
        // old script) following a previous runbook gets a sentence explaining the
        // change instead of a silent no-op.
        // ─────────────────────────────────────────────────────────────
        if (action === "device.get" || action === "devices.list") {
            return bad(410,
                "Device lookups are gone: codes are no longer tied to a device. Use " +
                "codes.list to see which codes have been redeemed and when they expire.");
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
                    // A hub address this fleet should prefer, delivered on the
                    // heartbeat. Surfaced for the same reason uot_port is: an
                    // operator editing a value they cannot see is editing blind,
                    // and this one exists specifically for the case where the
                    // normal address is unreachable — the worst moment to be
                    // guessing whether it was set.
                    hub_alias: cfg.hub_alias || "",
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
            // ── Hub address to offer this fleet ──
            //
            // Validated HERE as well as in the client, because the two failures
            // are different: a value the hub accepts but the client rejects is
            // silently ignored forever, and the operator sees a saved setting
            // that does nothing. Absolute https, no path — the same rule the
            // client applies in `chrysalis::contract::resolve_hub_url`.
            //
            // Empty string CLEARS it (the operator withdrawing the offer). A
            // missing key leaves it alone, so an older console cannot wipe a
            // setting it does not know about.
            if (body.hub_alias !== undefined) {
                var alias = String(body.hub_alias || "").trim();
                if (alias === "") {
                    delete cfgT.hub_alias;
                } else {
                    if (!/^https:\/\/[^/?#\s]+$/.test(alias)) {
                        return bad(400, "hub_alias must be an absolute https URL with no path, e.g. https://hub.example.org");
                    }
                    cfgT.hub_alias = alias;
                }
            }
            recT.set("config", JSON.stringify(cfgT));
            if (body.active !== undefined) recT.set("active", !!body.active);
            if (body.udp_relay !== undefined) recT.set("udp_relay", !!body.udp_relay);
            $app.dao().saveRecord(recT);
            // NEVER log cfgT.
            //
            // This wrote `JSON.stringify(cfgT)` — the tier's shadowsocks
            // password in cleartext — into `code_events`, which is a table the
            // console reads, an operator can export, and a backup carries off
            // the box. The whole shape of `tiers.*` is built to keep that secret
            // out of responses (tiers.list omits it, tiers.update refuses to
            // echo it), and this one line put it back into a place with weaker
            // handling than the tier row it came from.
            //
            // The audit question is "what changed and when", so record the
            // KEYS that changed. Who changed it needs no field: `logEvent`
            // stamps actor="console". Add a field name here if a new editable
            // key is added below, or the trail silently under-reports it.
            var tierChanged = [];
            if (body.server !== undefined && String(body.server).trim()) tierChanged.push("server");
            if (body.server_port !== undefined) tierChanged.push("server_port");
            if (body.method !== undefined && String(body.method).trim()) tierChanged.push("method");
            if (body.uot_port !== undefined) tierChanged.push("uot_port");
            if (body.hub_alias !== undefined) tierChanged.push("hub_alias");
            if (body.active !== undefined) tierChanged.push("active");
            if (body.udp_relay !== undefined) tierChanged.push("udp_relay");
            logEvent("(tier " + tierName + ")", "tier-updated",
                     tierChanged.length ? "changed: " + tierChanged.join(", ") : "no fields changed",
                     "");
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

            // ── The LEGACY single-platform fields ──────────────────────
            //
            // `update_url` / `update_sha256` predate the per-platform columns and
            // describe the LINUX binary only. They exist so a client that cannot
            // find its own platform's key still has something to fall back to.
            //
            // THEY WERE NEVER WRITTEN HERE. `publish-release.sh` sets both, but
            // this action — the one the console's "Fetch & publish" button
            // actually calls — did not. So every console publish since the
            // per-platform columns were added left these two at whatever the
            // PREVIOUS release had put there.
            //
            // Observed live on 2026-10-18: version was 3.3.1 while `update_url`
            // still named 3.2.30's Linux binary, with a `update_sha256` hashing a
            // *different file from the one advertised*. A client with no platform
            // key would download the old binary under the new version's name and
            // fail the hash — and the hash is what the updater trusts, so it fails
            // closed on a release that is otherwise perfectly published.
            //
            // This is the two-sided contract DEBUGGING-METHOD.md §3 describes:
            // two writers of one record, only one of which knew the field existed.
            // NOTE: the linux artifact is read into a local BEFORE use. Written
            // inline as a bracketed index on the artifacts map, the substring
            // was scanned by check-consistency.sh §1 as a truncated platform-key
            // LIST — that section scans every `.pb.js` for such literals, and a
            // dangling one looks exactly like a hook whose platform list has lost
            // three entries. Binding it to a variable keeps §1 strict (a real
            // list anywhere in a hook still fails the build) without teaching it
            // an exception it would later apply to a genuine list. Keep bracket
            // literals out of this file and its comments.
            var linuxArt;
            try { linuxArt = arts.linux; } catch (laErr) { linuxArt = null; }
            if (linuxArt) {
                recP.set("update_url", baseUrl + "/updates/" + pubVersion + "/" + linuxArt.filename);
                recP.set("update_sha256", String(linuxArt.sha256 || ""));
            } else {
                // No Linux artifact: clear rather than leave a stale pointer to a
                // previous version's binary. An empty hash refuses the update
                // (fail-closed) instead of applying the wrong bytes.
                recP.set("update_url", "");
                recP.set("update_sha256", "");
            }

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
