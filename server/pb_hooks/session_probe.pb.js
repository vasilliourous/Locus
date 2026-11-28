// Chrysalis Session Probe Hook — PocketBase 0.22 compatible
//
// POST /api/session-probe
//
// ── WHAT THIS ENDPOINT IS FOR ───────────────────────────────────────────────
//
// "The hub says this client is signed out and the client says it is signed in."
// That sentence has at least three different causes, and before this endpoint an
// operator could not tell them apart, because the hub had no fact to distinguish
// them and the console rendered one label for all three. See
// docs/reference/AUTH-STATE.md for the state model and docs/operate/
// RUNBOOK-DESYNC.md for the ordered diagnosis this feeds.
//
// The endpoint reports TWO independent axes and never collapses them:
//
//   entitlement — may this code be used?      (the hub's to answer; it refuses)
//   liveness    — is this device alive?       (the client's to answer; it beats)
//
// Collapsing those two into one label is the defect this whole change exists to
// remove. The console used to answer "signed out?" from `released_at`, which is
// an ENTITLEMENT fact, and a client that was beating every five minutes was
// therefore displayed as signed out. So `entitlement` and `liveness` come back
// as separate fields, `reason_code` names the exact divergence, and an
// uncomputable answer is `unknown` — never the nearest guess.
//
// ── STRICTLY READ-ONLY ──────────────────────────────────────────────────────
//
// This hook writes NOTHING. Not even `last_seen_at`: a probe is a diagnostic,
// and a diagnostic that mutates the thing it observes is a second source of
// truth for liveness. Every write in this system belongs to a flow that already
// owns it (the beat writes liveness, release-entitlement writes `released_at`,
// activation writes `activated_at`). Repair is the operator's separate, audited
// action — `codes.unbind` on the console, which already records a `code_events`
// row. Nothing here can change what the hub will do next.
//
// ── AUTHENTICATION, AND WHAT IT COSTS ───────────────────────────────────────
//
// Possession of the code, exactly like `/api/code-lookup` and
// `/api/release-entitlement`. A student cannot hold an admin token, and this is
// a surface support will ask them to reach.
//
// That makes the request authenticated BY THE VERY VALUE IT IS PROBING, which
// risks turning this into an unauthenticated CODE-EXISTENCE ORACLE — the same
// hazard `device_release.pb.js` documents at length. It is closed in three ways,
// and the FIRST is the load-bearing one:
//
//   1. THE MACHINE CHECK RUNS FIRST, BEFORE ANY STATE IS COMPUTED. The rich
//      response is only reachable when the caller presents a `device_id` whose
//      redacted prefix matches `first_seen_fingerprint` — the machine the code
//      was first redeemed on. A holder of a PHOTO of a card has the code and not
//      the machine, so they get the uniform 404. Note what this means: a SUSPENDED
//      or EXPIRED code answers 200-with-detail to the machine that owns it, and
//      404 to everyone else. That is deliberate, and it is why the gate must stay
//      above the entitlement block — see the comment there.
//   2. An unknown code, an unparseable code, an unrecognised device id and a
//      pre-migration row with no `first_seen_fingerprint` ALL answer the same
//      bare 404. The caller learns nothing from the difference, because there is
//      no difference to learn.
//   3. No response echoes the code, in any form, ever.
//
// ── RATE LIMITING ───────────────────────────────────────────────────────────
//
// Its own Caddy zone and its own hook bucket (`probe_`), because consuming the
// `activate_` budget would lock a student out of the screen that explains their
// code — the same argument `device_release.pb.js` makes for its `release_`
// bucket. A probe is an operator-driven diagnostic, not a loop.
//
// ── VERSIONING ──────────────────────────────────────────────────────────────
//
// `probe_version` is REQUIRED. An unrecognised version answers 409 rather than
// being interpreted anyway: a probe that silently ignores a field the caller
// relies on is exactly how `uot_port` vs `server_port_uot` shipped, and an
// operator acting on a half-understood answer is worse than one who is told to
// update their tool. `probe_version` is echoed in every response so a saved
// transcript says which schema produced it.
//
// ── WIRE NAMES ARE FROZEN HERE ──────────────────────────────────────────────
//
// `reason_code` values, `entitlement` values and `liveness` values are contracts
// with the console (`server/console/src/views/codes/model.ts`). Section 43 of
// check-consistency.sh asserts the two vocabularies agree, because a string
// duplicated across languages with nothing enforcing agreement is the incident
// class this repository has paid for four times.

routerAdd("POST", "/api/session-probe", function (e) {
    // Helpers are defined INSIDE the callback on purpose: goja does not hoist
    // function declarations across scopes, so a file-level helper would be
    // invisible here. Same reason every other hook in this directory inlines
    // its own — stated once in device_release.pb.js and repeated nowhere else.

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
        if (!isNaN(swapped)) return swapped;
        if (/^[0-9]+$/.test(s)) {
            var n = parseInt(s, 10);
            return s.length <= 10 ? n * 1000 : n;
        }
        return NaN;
    }

    // The single uniform answer. Every "nothing to say" path returns THIS, and
    // the reason is the existence-oracle argument in the header: the caller must
    // not be able to tell an unknown code from a suspended one.
    function opaqueNotFound() {
        return e.json(404, { code: 404, message: "Code not found" });
    }

    var SUPPORTED_PROBE_VERSION = 1;

    // ── The lease arithmetic, in ONE place ──────────────────────────────────
    //
    // These two numbers are derived from the CLIENT's heartbeat constants, not
    // chosen by feel. `MAX_INTERVAL` 2 h, ±10 % jitter, 15 s per-attempt timeout,
    // one retry over the maintenance tunnel, 60 s slack:
    //
    //   LEASE_SECONDS = 2h * 1.1 + 15s + 60s = 2h13m15s, rounded up to 2h14m
    //
    // A shorter lease would declare a HEALTHY but unlucky device (one that has
    // backed off to MAX_INTERVAL after an outage) absent. That is the failure
    // this number is sized to prevent, and it is why the lease is not tuned
    // down to make demos look faster — see ADR 0002.
    //
    // THE FIGURE WAS 2h16m UNTIL 2026-11-28 and the two extra minutes were never
    // in the arithmetic above — the constant was picked, then this comment written
    // to describe it. `check-consistency.sh` §59 now derives the bound from
    // `heartbeat.rs` and fails if this line disagrees, so the sentence can no
    // longer drift from the number. `docs/state.toml` [liveness] is the home.
    //
    // STALE_SECONDS mirrors the client's own GRACE_PERIOD (7 days) so both sides
    // age a device out on the same clock. Two different windows would mean the
    // console says "gone" while the client still believes it is authenticated,
    // which is the desync in the other direction.
    var LEASE_SECONDS = 2 * 60 * 60 + 14 * 60;
    var STALE_SECONDS = 7 * 24 * 60 * 60;

    try {
        var data = $apis.requestInfo(e).data;

        var probeVersion = parseInt(data.probe_version, 10);
        if (!probeVersion || isNaN(probeVersion) || probeVersion < 1) {
            return e.json(400, { code: 400, message: "Missing probe_version" });
        }
        if (probeVersion !== SUPPORTED_PROBE_VERSION) {
            // Deliberately NOT interpreted. See the versioning note above.
            return e.json(409, {
                code: 409,
                message: "Unsupported probe_version " + probeVersion,
                probe_version: SUPPORTED_PROBE_VERSION,
            });
        }

        var code = (data.code || "").trim();
        if (!code) return e.json(400, { code: 400, message: "Missing code" });

        var canonical = (function (raw) {
            var s = String(raw || "").replace(/-/g, "").toUpperCase();
            return (s.length === 15)
                ? s.substring(0, 2) + "-" + s.substring(2, 6) + "-" + s.substring(6, 10) + "-" + s.substring(10, 14) + "-" + s.substring(14, 15)
                : String(raw || "");
        })(code);

        var record = null;
        try {
            record = $app.dao().findFirstRecordByData("codes", "code", canonical);
        } catch (notFound) {
            record = null;
        }
        if (!record) return opaqueNotFound();

        // ── Where the oracle is closed, exactly ─────────────────────────────
        //
        // THERE IS ONE GATE, NOT TWO, and it is the MACHINE CHECK below.
        //
        // This block used to say that suspension and expiry answer the same
        // opaque 404 as an unknown code. THEY DO NOT, and that is correct: the
        // machine check runs BEFORE the entitlement axis is computed, so a caller
        // who reaches the 200 has already proved they hold the machine that first
        // redeemed the code. To that caller, `suspended` and `expired` are not
        // secrets — the beat already tells them, on the kick-out screen, in the
        // operator's own words.
        //
        // The ordering is the security property, and it is fragile in exactly one
        // way: moving the machine check BELOW the entitlement computation would
        // publish every code's state to anyone holding a card, with nothing at
        // runtime to say so. `probe-hook-consistency.js` asserts the uniform 404
        // for an unknown code / wrong device / pre-migration row AND asserts that
        // a correct machine sees the detail — so a reorder fails the guard rather
        // than the students.
        //
        // `device_release.pb.js` makes the same single-gate choice for the same
        // reason, and documents it there.
        var nowMs = Date.now();

        // ── The machine check (the real gate) ───────────────────────────────
        //
        // `device_id` is the client's already-redacted 12-char fingerprint
        // (`store::redact`). `first_seen_fingerprint` is the value activation
        // stored, which for existing rows is the first 16 characters PLUS a
        // literal "****" redaction suffix. A caller who cannot present the
        // machine that first redeemed the code gets the uniform 404 — they have
        // a code, which may be a photograph, and nothing more.
        //
        // ── WHY THIS IS A PREFIX MATCH, NOT AN EQUALITY ──
        //
        // It was an equality, and it could never pass: the client sends 12
        // characters and the hub stores 20 (16 + "****"), so EVERY probe from a
        // real device answered the opaque 404. The endpoint's own header claimed
        // the two were stored "in the same truncated form"; the write in
        // `activation.pb.js` disproves that. The symptom was exactly the reported
        // one — an operator had no way to tell a working probe from a broken one,
        // because both looked like "Code not found".
        //
        // So the comparison accepts the shorter as a prefix of the longer, with a
        // floor of 12 characters (the redaction length the client sends). That
        // keeps the oracle closed — the attacker still has to know 12 hex
        // characters of the machine — and it works for both stored shapes.
        //
        // A code with NO `first_seen_fingerprint` (never redeemed through the
        // path that writes it, i.e. pre-migration stock) FAILS this check. That
        // is the safe direction: an operator diagnosing such a code uses the
        // console, which is authenticated by admin token, not this endpoint.
        var presentedId = String(data.device_id || "").trim().toLowerCase();
        var firstSeen = String(record.get("first_seen_fingerprint") || "").trim();
        var firstSeenLower = firstSeen.toLowerCase();
        var MIN_FINGERPRINT = 12;
        var machineMatches = false;
        if (presentedId.length >= MIN_FINGERPRINT
                && firstSeenLower.length >= MIN_FINGERPRINT) {
            machineMatches = presentedId === firstSeenLower
                || firstSeenLower.indexOf(presentedId) === 0
                || presentedId.indexOf(firstSeenLower) === 0;
        }
        if (!machineMatches) return opaqueNotFound();

        // ── The operator's override: a marker the probe MUST report ─────────
        //
        // A release by an operator is a decision, not a derived state: it clears
        // the claim and (for a live device) asks the heartbeat to cut it off.
        // Until this existed the probe could not tell an operator-run correction
        // from an ordinary redemption, so "I overrode that code — did it take?"
        // had no answer from the one endpoint built to answer questions like it.
        //
        // `force_state` and `evict_requested_at` are the STORED predicate — no
        // new column, because they are already written by the override that
        // creates the condition and read by the heartbeat. This block only reads
        // them, preserving the endpoint's write-nothing rule.
        var forceState = String(record.get("force_state") || "").trim();
        var evictAt = String(record.get("evict_requested_at") || "").trim();
        var evictReason = String(record.get("evict_reason") || "").trim();
        var overrideId = String(record.get("override_id") || "").trim();
        var hasOverride = forceState !== "" || evictAt !== "";

        // ── The entitlement axis ────────────────────────────────────────────
        var suspended = record.getBool("suspended");
        var suspendedReason = String(record.get("suspended_reason") || "").trim();
        var expMs = parsePBDate(record.get("expires_at"));
        var expired = !isNaN(expMs) && expMs > 0 && expMs < nowMs;

        var activatedAtRaw = record.get("activated_at");
        var releasedAtRaw = record.get("released_at");
        var actMs = parsePBDate(activatedAtRaw);
        var relMs = parsePBDate(releasedAtRaw);

        // The SAME comparison activation.pb.js and admin_console.pb.js use, byte
        // for byte. Three copies of one rule is three things that can disagree;
        // §43 asserts this one matches, and the console's copy does too.
        var releasedPending = (actMs > 0 && relMs > 0 && relMs > actMs);

        // ── `never-activated` is NOT the word for a released code ──
        //
        // A release clears `activated_at`, so the old chain reported every
        // operator-released code as `never-activated` — a false statement about a
        // card that was sold and freed, and the console shows it verbatim next to
        // "released". The claim state has its own word: `released`.
        var entitlement;
        if (suspended) entitlement = "suspended";
        else if (expired) entitlement = "expired";
        else if (forceState === "released") entitlement = "released";
        else if (!activatedAtRaw || actMs <= 0) entitlement = "never-activated";
        else if (releasedPending) entitlement = "released-unclaimed";
        else entitlement = "active";

        // ── The liveness axis, from the ONE column the beat writes ──────────
        //
        // `last_seen_at` null means UNKNOWN, NOT ABSENT. A code that has not
        // beaten since the column existed has no age to report, and inventing
        // "absent" from a missing value would accuse a device of being gone on
        // no evidence — the specific "unknown rendered as a definite state"
        // failure this work exists to remove.
        var seenMs = parsePBDate(record.get("last_seen_at"));
        var seenAgeS = null;
        var liveness;
        if (seenMs === 0 || isNaN(seenMs)) {
            liveness = "unknown";
        } else {
            seenAgeS = Math.max(0, Math.floor((nowMs - seenMs) / 1000));
            if (seenAgeS <= LEASE_SECONDS) liveness = "live";
            else if (seenAgeS <= STALE_SECONDS) liveness = "stale";
            else liveness = "absent";
        }

        var leaseExpiresAt = (seenMs > 0 && !isNaN(seenMs))
            ? new Date(seenMs + LEASE_SECONDS * 1000).toISOString()
            : null;

        // ── The client's own report, if it made one ─────────────────────────
        //
        // OPTIONAL, and the probe must work without it: the operator usually
        // runs this to diagnose a device they are NOT sitting at. An absent
        // report is reported as absent — never defaulted to a value, because a
        // defaulted client state would turn "I do not know what the client
        // thinks" into a divergence verdict that was never observed.
        var client = data.client || null;
        var clientState = null;
        var clientDegradation = [];
        var reportedSkew = null;
        if (client) {
            clientState = String(client.auth_state || "").trim() || null;
            if (client.degradation && client.degradation.length) {
                for (var di = 0; di < client.degradation.length && di < 8; di++) {
                    clientDegradation.push(String(client.degradation[di]));
                }
            }
            // Skew is REPORTED and never corrected. It is a fact about the two
            // clocks, not an input to any decision — the liveness arithmetic
            // above already uses only the hub's clock.
            var clientUnix = parseInt(client.clock && client.clock.unix, 10);
            if (clientUnix && !isNaN(clientUnix)) {
                reportedSkew = Math.round((clientUnix * 1000 - nowMs) / 1000);
            }
        }

        // ── The divergence verdict ──────────────────────────────────────────
        //
        // Ordered by specificity: the first match wins, because several can be
        // technically true at once and the operator needs the most actionable
        // one.
        //
        // THE CHAIN MUST BE EXHAUSTIVE, and that is not obvious. An earlier
        // version ended in `else { reasonCode = "in_sync" }`, so any combination
        // nobody had thought of — including `liveness: "unknown"`, which is the
        // one case where the hub genuinely has NO data — was reported as
        // "in_sync". An operator reading that concludes both sides agree. The
        // fallthrough is now `unknown`, and `in_sync` is reachable only when both
        // sides were actually observed.
        //
        // Every code here is a contract with model.ts and the runbook — see §43's
        // sibling in probe-hook-consistency.js, which drives this chain.
        var reasonCode;
        var divergences = [];

        var skewed = (reportedSkew !== null && Math.abs(reportedSkew) > 300);
        if (skewed) {
            // Reported AND surfaced, never acted on. A skew is a fact about two
            // clocks; the lease arithmetic uses only the hub's, so it cannot
            // corrupt a verdict. It is promoted to `reason_code` because an
            // operator seeing a client whose clock is an hour out needs to be
            // told that FIRST — every other observation from that client is
            // suspect until they know.
            divergences.push("clock_skew");
        }

        if (skewed) {
            reasonCode = "clock_skew";
        } else if (forceState === "released") {
            // THE OPERATOR'S OWN ACTION, named. Without this arm a released code
            // fell through to `unknown` (its entitlement is no longer `active` and
            // it is not a refusal), which told the operator the probe could not
            // name a case it had just described in `override`. The eviction detail
            // travels in `override` and `divergence`, so this verdict only has to
            // say why the two sides are as they are.
            reasonCode = "released_by_override";
        } else if (clientState === "activated" && entitlement === "released-unclaimed") {
            // THE REPORTED BUG, named. The hub holds a sign-out claim that a
            // beating device contradicts: something wrote `released_at` for a
            // machine that is demonstrably still using the code (either another
            // device on the same code, or a stale claim nobody spent).
            reasonCode = "stale_release_claim";
        } else if (clientState === "activated" && (suspended || expired)) {
            // The hub has already refused; the client just has not beaten since.
            // Self-healing within one beat once connected — say so, do not
            // prescribe anything.
            reasonCode = "hub_refused_client_hasnt_noticed";
        } else if (liveness === "unknown") {
            // NOTHING HAS EVER BEATEN. There is no evidence of agreement, only an
            // absence of evidence: a client that says "activated" while the hub
            // has never seen it check in is either a device that never connected
            // or a hub that is not recording, and those need different fixes.
            //
            // THIS ARM IS EXPLICIT-BUT-REDUNDANT, and that is stated rather than
            // left to look load-bearing. The final `else` also yields `unknown`,
            // so deleting this arm changes no answer — a mutation confirmed it.
            // It is kept because the explicit arm documents the case at the point
            // a reader looks for it, and because it makes the ordering visible:
            // everything above it is a positive observation, everything below is a
            // degradation. That its REMOVAL is invisible is exactly why the
            // harness asserts the PROPERTY ("no path returns in_sync for unknown
            // liveness") rather than this branch's existence.
            reasonCode = "unknown";
        } else if (liveness === "stale" || liveness === "absent") {
            // The device has stopped checking in. It may have been killed, gone
            // offline, or the student may simply have quit — and this endpoint
            // cannot and does not guess which. TTL expiry is the answer, not a
            // quit message: see ADR 0002.
            reasonCode = "client_unreachable";
        } else if (clientState && clientState !== "activated" && liveness === "live") {
            // The client says it dropped its session while the hub still holds a
            // live record — a local wipe (config reset, mirror lost) rather than
            // a sign-out. The student re-activates; nothing is orphaned.
            //
            // THIS MUST PRECEDE THE `in_sync` ARMS. `liveness: live` +
            // `entitlement: active` looks healthy from the hub's side alone, but
            // a client that reports `signed-out` is DISAGREEING — the hub holds a
            // record the device says it does not.
            //
            // It is still gated on `entitlement === "active"`, which is what
            // separates it from `genuinely_signed_out` below: when the hub has
            // ALREADY freed the code, the two sides are not in conflict, and
            // calling that a dropped session would invent a lost credential.
            if (entitlement === "active") {
                reasonCode = "client_dropped_session";
            } else {
                reasonCode = "genuinely_signed_out";
            }
        } else if (liveness === "live" && entitlement === "active" && clientState === "activated") {
            // THE PRIMARY CASE, and it has its own name on purpose.
            //
            // The operator reached this page because the console said "signed
            // out" for a device that is demonstrably checking in. `in_sync` would
            // answer "nothing to see", which is true of the DATA and useless to
            // the operator — what they need to be told is that the console's label
            // was the artifact, not the device. Grading the symptom is the whole
            // value of a reason code; folding this into `in_sync` deletes the one
            // answer the runbook opens with.
            reasonCode = "client_beating_hub_quiet";
        } else if (liveness === "live" && entitlement === "active") {
            // Same liveness and entitlement, but NO client report — the normal
            // operator case (diagnosing a device they are not sitting at). The
            // hub side is healthy and the client half is simply unobserved, which
            // the response states as `client_view: null` rather than assuming.
            //
            // Kept distinct from the arm above because the two imply different
            // operator actions: here they must still reach the student or the
            // device to check the other half.
            reasonCode = "in_sync";
        } else {
            // REACHED ONLY when the inputs contradict in a way no branch above
            // covers. Saying `in_sync` here would be a lie; saying `unknown` is
            // the truth and sends the operator to the console, which is the
            // correct destination for a case this endpoint cannot name.
            reasonCode = "unknown";
        }

        if (clientState === "activated" && liveness !== "live" && liveness !== "unknown") {
            divergences.push("client_not_beating");
        }
        if (clientState === "activated" && entitlement !== "active") {
            divergences.push("entitlement_refused");
        }
        if (hasOverride) {
            // The operator's own correction, surfaced in the same list as every
            // other observation. Deliberately NOT a `reason_code`: those name a
            // DIVERGENCE between the two sides, and an override is a decision
            // both sides are being made to agree with.
            divergences.push("operator_override");
        }
        for (var gi = 0; gi < clientDegradation.length; gi++) {
            // The client's own degradation flags are passed through verbatim so
            // the operator sees the client's words, not this hub's paraphrase.
            divergences.push(clientDegradation[gi]);
        }

        return e.json(200, {
            probe_version: SUPPORTED_PROBE_VERSION,
            server_time: new Date().toISOString(),

            // The two axes, never collapsed. A caller that needs one label has
            // to choose, which is the point.
            entitlement: entitlement,
            liveness: liveness,

            // The operator's decision, when one is in force. `null` is the
            // ordinary case and MUST render as "no override", not as "unknown".
            override: hasOverride ? {
                force_state: forceState || null,
                override_id: overrideId || null,
                evicted: evictAt !== "",
                evicted_at: evictAt || null,
                evicted_reason: evictReason || null,
            } : null,

            hub_view: {
                activated_at: activatedAtRaw ? String(activatedAtRaw) : null,
                released_at: releasedAtRaw ? String(releasedAtRaw) : null,
                released_pending: releasedPending,
                suspended: suspended,
                suspended_reason: suspended ? suspendedReason : null,
                expires_at: record.get("expires_at") ? String(record.get("expires_at")) : null,
                tier: record.getString("tier") || "",
                // null, not 0: "never seen" and "seen 0 seconds ago" are
                // different facts and must not render the same. Same rule as
                // `last_confirmed_at` in cmd/chrysalis.rs.
                last_seen_at: (seenMs > 0 && !isNaN(seenMs)) ? new Date(seenMs).toISOString() : null,
                last_seen_age_s: seenAgeS,
                first_seen_fingerprint: firstSeen,
            },

            client_view: client ? {
                auth_state: clientState,
                app_version: String(client.app_version || ""),
                platform: String(client.platform || ""),
                last_heartbeat_ok_unix: client.last_heartbeat_ok_unix || null,
                heartbeat_failures: client.heartbeat_failures || 0,
                grace_remaining_s: (client.grace_remaining_s === undefined ? null : client.grace_remaining_s),
                hub_enforcement: (client.hub_enforcement === undefined ? null : client.hub_enforcement),
                recorded_refusal: client.recorded_refusal || null,
                transport: client.transport || null,
                clock_skew_s: reportedSkew,
            } : null,

            lease_expires_at: leaseExpiresAt,
            reason_code: reasonCode,
            divergence: divergences,
        });
    } catch (err) {
        // A probe that throws is a probe that says nothing — and 500 is the
        // honest answer for that. It must never fall through to a 200 with
        // partial data, which an operator would read as a verdict.
        return e.json(500, { code: 500, message: err.message || String(err) });
    }
});
