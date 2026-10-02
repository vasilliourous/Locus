//! The activation/lookup refusal contract, pinned across the language boundary.
//!
//! # Why this test exists
//!
//! Six live documents and two code comments assert that this contract is
//! "pinned by `activation_contract_test`". **No such test existed.** The claim
//! was repeated until it read as fact, and every reader — including one
//! planning this very work — trusted it and moved on. That is the failure mode
//! this file closes: a guard that is described but absent is worse than no
//! guard, because it stops anyone from checking.
//!
//! The contract itself is genuinely fragile and genuinely load-bearing:
//!
//! The client classifies an activation refusal by **status code first, then by
//! the wording of a 403** (`classify` in `locus/activation.rs`). `403` carries
//! two different conditions — "code bound to another device" and "code
//! suspended" — and the ONLY thing distinguishing them is whether the message
//! contains the substring `suspended`. Deployed clients in the field run that
//! exact comparison, and they cannot be updated.
//!
//! So a hub-side reword is not a cosmetic change. Renaming "Code suspended" to
//! "This account is on hold" would make every deployed client report a
//! suspension as "your code is bound to another device", and send the student to
//! a middleman with the wrong question. Nothing in the build would have failed.
//! This test is what fails.
//!
//! # What it checks, and what it deliberately does not
//!
//! It reads the hub's hook source and asserts that the literal status codes and
//! message strings the hub emits are the ones the client's classifier was
//! written against. It is a *blunt text check*: the hooks are goja scripts that
//! cannot be imported or called, so the assertion is on their source text. That
//! is exactly the trade-off `update_signature_contract.rs` makes for
//! `publish-release.sh`, for the same reason.
//!
//! It does NOT re-implement the client's classifier — that is unit-tested in
//! `locus::activation::tests`. If it did, the two could drift together and
//! agree on the wrong thing. This test pins the *hub's* side, which is the
//! side that can change without any Rust building differently.

// Integration tests are their own crates, so the `cfg_attr(test, ...)` lint
// scope in `lib.rs` does not reach here. `panic!`/`expect()` are the correct
// idioms in a test — see that comment for the reasoning.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity
)]

use std::path::{Path, PathBuf};

/// The repository root, found by walking up from this test.
///
/// `CARGO_MANIFEST_DIR` is `client/src-tauri`, so the root is two levels up.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent")
        .parent()
        .expect("client must have a parent")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The hub's activation hook, as text.
fn activation_hook() -> String {
    read(&repo_root().join("server/pb_hooks/activation.pb.js"))
}

/// The hub's lookup hook, as text.
fn lookup_hook() -> String {
    read(&repo_root().join("server/pb_hooks/code_lookup.pb.js"))
}

// ─────────────────────────────────────────────────────────────────────────────
// The 403 substring contract — the fragile one
// ─────────────────────────────────────────────────────────────────────────────

/// The exact 403 message that means "this code is bound to a different device".
///
/// A deployed client reads any 403 whose message does NOT contain `suspended`
/// as this outcome, so the message is the contract for every other 403 the hub
/// might send.
#[test]
fn the_bound_to_another_device_403_keeps_its_wording() {
    assert!(
        activation_hook().contains(r#"e.json(403, {code:403, message:"Code bound to another device"})"#),
        "the hub no longer emits the literal 403 message \"Code bound to another \
         device\". Deployed clients classify a 403 by substring: any wording \
         without the word `suspended` means THIS outcome. Changing it without a \
         matching client silently mislabels every refusal. See `classify` in \
         locus/activation.rs."
    );
}

/// The exact 403 message that means "an operator suspended this code".
///
/// The word `suspended` is not decoration — it is the entire mechanism by which
/// a deployed client tells this apart from the message above.
#[test]
fn the_suspended_403_keeps_its_wording() {
    assert!(
        activation_hook().contains(r#"e.json(403, {code:403, message:"Code suspended"})"#),
        "the hub no longer emits the literal 403 message \"Code suspended\". The \
         client classifier is a case-insensitive search for the substring \
         `suspended`; removing that word makes the hub's suspension refusal read \
         as \"bound to another device\" on every deployed build."
    );
}

/// The substring the client actually keys on must still appear in the hub's
/// suspension wording.
///
/// This is a weaker, overlapping assertion than the exact-string one above, and
/// that is deliberate: this is the property the *client* depends on, stated as
/// the client states it. If the hub's message is ever reworded to something that
/// still contains `suspended`, this stays green while the exact-string test
/// fails loudly — which is the honest signal, because the reword is still a
/// change to a frozen contract.
#[test]
fn the_suspension_wording_still_contains_the_substring_the_client_matches() {
    let hook = activation_hook();
    // Find every 403 the hook emits and check the suspension one.
    let has_suspension_403 = hook
        .lines()
        .filter(|line| line.contains("e.json(403"))
        .any(|line| line.to_ascii_lowercase().contains("suspended"));

    assert!(
        has_suspension_403,
        "no 403 emitted by the activation hook contains the substring \
         `suspended`. The client's classifier (`classify` in \
         locus/activation.rs) uses exactly that substring to separate a \
         suspension from a device-binding refusal, and deployed clients cannot \
         be changed."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Status codes the client maps to specific outcomes
// ─────────────────────────────────────────────────────────────────────────────

/// Every status code the client maps to a *specific* outcome must still be the
/// code the hub actually sends for that condition.
///
/// This is the table `classify` is written against. A renumbering on either side
/// is invisible until a student is told the wrong thing, so it is pinned here.
#[test]
fn the_outcome_status_codes_are_unchanged() {
    let hook = activation_hook();

    // 404 -> NotFound, 410 -> Expired, 429 -> RateLimited, 409 -> DeviceAlreadyActivated
    for (code, what) in [
        (404, "Code not found"),
        (410, "Code expired"),
        (429, "Too many attempts"),
    ] {
        assert!(
            hook.contains(&format!(r#"e.json({code}, {{code:{code}, message:"{what}"}})"#))
                || hook.contains(&format!(r#"e.json({code},{{code:{code}, message:"{what}"}})"#)),
            "the hub no longer emits {code} \"{what}\" from /api/activate. \
             The client maps status {code} to a specific `ActivationOutcome` arm; \
             a renumbering tells the student the wrong thing about their code."
        );
    }
}

/// The "one code per device" refusal must be **409**, and must not be 403.
///
/// 403 is already overloaded for two conditions and disambiguated by substring
/// (above). A third meaning on 403 would make an old client report a binding
/// problem as a suspension. 409 is the whole point: it is a status the old
/// classifier does not map, so it falls through to `ServerError` and the message
/// is shown verbatim — which is why the message must also stand alone.
#[test]
fn the_one_code_per_device_refusal_is_409_not_403() {
    let hook = activation_hook();

    assert!(
        hook.contains("e.json(409,"),
        "the hub no longer refuses a second code on a bound device with 409. \
         This refusal must NOT reuse 403: the client disambiguates 403 by \
         substring, so a third 403 meaning would be misread as a suspension or a \
         device-binding error on every deployed build."
    );

    // The word `suspended` must not appear in the 409's message, because a
    // deployed client that has no 409 arm receives the message through its
    // generic ServerError path and renders it verbatim — and a message
    // containing `suspended` would be mislabelled if the status ever changed.
    let has_409 = hook.lines().find(|line| line.contains("e.json(409"));
    if let Some(line) = has_409 {
        assert!(
            !line.to_ascii_lowercase().contains("suspended"),
            "the 409 one-code-per-device message contains the word `suspended`, \
             which is the substring a 403 classifier keys on. The message is \
             shown verbatim to old clients through their ServerError path; \
             it must not be mistakable for a suspension. Line: {line}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The lookup hook's status vocabulary
// ─────────────────────────────────────────────────────────────────────────────

/// Whether the hook emits `status` anywhere, in either of the two forms it
/// actually uses.
///
/// The hook is a goja script, so the "contract" is a string value rather than a
/// typed field, and it is written two ways: inline in the `e.json(...)` literal
/// (`status:"ok"`) and by assignment to a response object
/// (`resp.status = "bound_other"`). Both mean the same thing on the wire, so the
/// guard must accept either form. An earlier version of this test matched only
/// the inline form and reported a false failure for the five statuses the hook
/// assigns — which is exactly the kind of over-specific guard that teaches
/// people to ignore a red test.
fn hook_emits_status(hook: &str, status: &str) -> bool {
    hook.contains(&format!(r#"status:"{status}""#))
        || hook.contains(&format!(r#"status = "{status}""#))
}

/// The lookup statuses the client deserializes must still be the ones the hub
/// emits.
///
/// `LookupStatus` is a serde enum with `#[serde(other)] => Unknown`, so a hub
/// that invents a new status degrades safely rather than failing — but a hub
/// that *renames* one silently turns "ready to activate" into "unknown", which
/// the screen renders as "could not check this code right now". That is a
/// confusing dead end for a student holding a perfectly good code.
#[test]
fn the_lookup_status_vocabulary_is_unchanged() {
    let hook = lookup_hook();

    for status in [
        "ok",
        "unbound",
        "bound_this_device",
        "bound_other",
        "suspended",
        "expired",
        "not_found",
    ] {
        assert!(
            hook_emits_status(&hook, status),
            "the lookup hook no longer emits status \"{status}\". The client's \
             `LookupStatus` enum deserializes these exact snake_case strings; a \
             rename degrades to `Unknown`, which the activation screen shows as \
             \"could not check this code right now\"."
        );
    }
}

/// An unknown status must degrade to `Unknown`, never to a success.
///
/// Asserted on the client's own enum definition rather than by parsing the hook:
/// this is the property that makes the vocabulary above safe to extend.
#[test]
fn the_lookup_enum_degrades_unknown_statuses_safely() {
    let source = read(&repo_root().join("client/src-tauri/src/locus/contract.rs"));

    assert!(
        source.contains("#[serde(other)]"),
        "`LookupStatus` in locus/contract.rs no longer has a `#[serde(other)]` \
         catch-all. Without it, a hub that adds a status fails to deserialize, \
         and the activation screen reports a transport error for a code that is \
         perfectly fine. Guessing \"ready\" is worse still — it tells a student \
         to activate a code that will be refused."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Guard against the drift this file was written to fix
// ─────────────────────────────────────────────────────────────────────────────

/// The documents that cite `activation_contract_test` must name a test that
/// exists.
///
/// This is the meta-guard. The bug that motivated this file was not a broken
/// contract — the contract was fine — it was that six documents claimed a guard
/// protected it and no guard existed. A future agent reading the same claim
/// should be able to trust it, which means the claim must be checkable.
#[test]
fn the_claimed_contract_test_exists() {
    let this_file = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/activation_contract.rs"),
    )
    .expect("this test file must be readable");

    // The name the docs use. If the file is renamed, the docs must move with it;
    // the point is that the *claim* resolves to something real.
    assert!(
        this_file.contains("activation/lookup refusal contract"),
        "this file no longer describes itself as the activation contract test, \
         so the six documents citing `activation_contract_test` point at \
         nothing. Update the docs in the same change or restore the description."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Device recognition — the code-free re-activation path
// ─────────────────────────────────────────────────────────────────────────────

/// The recognition response must NEVER contain the activation code.
///
/// This is the security property the whole design rests on. The activation code
/// is a bearer credential for the entitlement; if recognition returned it, then
/// anyone able to present a matching identity could read the code out and walk
/// away with it — an IDOR. The client does not need it (the tier config is what
/// builds a tunnel), so it must never be sent.
///
/// Asserted against the HOOK SOURCE rather than a sample response, because a
/// runtime test would only cover the inputs it happened to try. This scans every
/// `response.<field> =` assignment, so a field added on any path is caught.
#[test]
fn recognition_never_returns_the_activation_code() {
    let hook = read(&repo_root().join("server/pb_hooks/device_recognise.pb.js"));

    // Every field the response is allowed to set. The list is deliberately
    // explicit: adding to it should be a conscious act, because the one value
    // that must never appear here is the activation code.
    let allowed = [
        "status",
        "tier",
        "expires_at",
        "store",
        "message",
        "server_config",
        "udp_relay",
        "token",
        "token_expires_at",
    ];

    for line in hook.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("response.") {
            let field: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if field.is_empty() {
                continue;
            }
            assert!(
                allowed.contains(&field.as_str()),
                "the recognition response sets an unexpected field `{field}`. If \
                 this is the activation code, STOP: the code is a bearer credential \
                 and returning it here is the IDOR this design exists to avoid. If \
                 it is a new legitimate field, add it to this allow-list deliberately."
            );
        }
    }
}

/// A recognition miss must be indistinguishable from any other miss.
///
/// The uniform `unknown()` answer is what stops the endpoint being a
/// confirmation oracle: a prober must not be able to tell "no such device" from
/// "revoked" from "malformed" from "nothing entitled".
#[test]
fn recognition_answers_every_negative_identically() {
    let hook = read(&repo_root().join("server/pb_hooks/device_recognise.pb.js"));

    assert!(
        hook.contains("function unknown()") && hook.contains(r#"status:"unknown""#),
        "the recognition hook no longer funnels its negatives through one uniform \
         answer. Distinct failure shapes let a caller learn which device ids or \
         verifiers are real, which is exactly what the uniform response prevents."
    );

    // No negative path may leak a distinct status string.
    for leak in ["not_found", "revoked", "expired"] {
        assert!(
            !hook.contains(&format!(r#"status:"{leak}""#)),
            "the recognition hook answers with a distinct negative status \
             `{leak}`. Every negative must be `unknown`, so the endpoint cannot be \
             used to confirm a guessed credential."
        );
    }
}

/// The heartbeat must accept the token, or a recognised device can never check in.
///
/// Recognition exists so a reinstalling student skips the code prompt. But every
/// enforcement rule — suspension, expiry, config refresh — runs on the
/// heartbeat, and the heartbeat is code-keyed. If it did not also accept the
/// token, a recognised device would connect and then never find out it had been
/// suspended, which is worse than not recognising it at all.
#[test]
fn the_heartbeat_accepts_a_session_token() {
    let hook = read(&repo_root().join("server/pb_hooks/heartbeat.pb.js"));

    assert!(
        hook.contains("data.token"),
        "the heartbeat no longer reads a session token from the request body. A \
         token-authenticated device (every recognised one) would be unable to \
         check in, so it would never learn of a suspension or expiry."
    );
    assert!(
        hook.contains("device_identities"),
        "the heartbeat no longer resolves a token against device_identities, so \
         the token path is dead."
    );
    assert!(
        hook.contains("token_hash"),
        "the heartbeat no longer looks the token up by hash. The stored value is \
         a hash, so a plaintext lookup would never match and every recognised \
         device would be refused."
    );
}

/// The token must never be sent back to a client as part of a heartbeat log or
/// written into the attempts table.
///
/// It is a credential. The attempts log records a device id prefix and nothing
/// else; a token appearing there would put a usable credential in a table that
/// exists to be read by operators.
#[test]
fn the_token_is_not_written_into_the_attempts_log() {
    let hook = read(&repo_root().join("server/pb_hooks/device_recognise.pb.js"));

    for line in hook.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("att.set(") {
            assert!(
                !trimmed.contains("token"),
                "the attempts log records the session token. That table is read by \
                 operators; a token in it is a credential in a place nobody treats \
                 as secret. Store a redacted device id instead."
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Identity registration — the write whose absence broke auto-sign-in
// ─────────────────────────────────────────────────────────────────────────────

/// `/api/activate` must write a `device_identities` row, or recognition can
/// never find the device.
///
/// # The defect this pins
///
/// Until 2026-10-02 **nothing in the hook set created a `device_identities`
/// row**. `/api/device-recognise` and `/api/heartbeat` only ever read that
/// collection, so recognition looked up a row activation had never written and
/// answered the uniform `unknown` for every device. Two reported symptoms, one
/// cause: auto-sign-in never fired for anyone, and a reinstall — which
/// re-derives a different fingerprint — was told its own code was "already in
/// use on another device".
///
/// # Why a text check, and why THIS text
///
/// The hook is goja source that cannot be imported or called from Rust, so the
/// assertion is on its text — the same trade-off the rest of this file makes.
/// `registerIdentity(` is the call that performs the write; asserting the *call*
/// rather than the function's definition is deliberate, and it is the mistake
/// `check-consistency.sh` §15 made for `is_installer_payload`: matching the
/// symbol passes even when the call site has been removed.
#[test]
fn activation_registers_the_device_identity() {
    let hook = activation_hook();

    assert!(
        hook.contains("function registerIdentity("),
        "activation.pb.js no longer defines `registerIdentity`. Without an \
         identity write, `/api/device-recognise` has nothing to find and both \
         auto-sign-in and reinstall-survival are dead."
    );

    // The call sites, not the definition. A hook that defines the helper and
    // never calls it is exactly the shipped-broken shape this test exists for.
    let calls = hook.matches("registerIdentity(").count();
    assert!(
        calls >= 3,
        "activation.pb.js defines `registerIdentity` but calls it {calls} time(s) \
         (the definition is 1). It must be called on BOTH success paths — the \
         first-activation bind and the same-device re-activation — or a device \
         that repairs its code never becomes recognisable."
    );

    assert!(
        hook.contains(r#"row.set("verifier", verifier)"#),
        "the identity write does not set `verifier`, which is the field \
         `/api/device-recognise` looks the device up by. A row without it can \
         never be found."
    );
}

/// The client must SEND the verifier, or the hub has nothing to register.
///
/// This is the other half of the two-sided contract: the hub can only write an
/// identity it is given. `CodeRequest` gained the field; this pins that the
/// wire body actually carries it, and that an absent identity omits the keys
/// rather than sending empty strings (which would fail the hub's shape check
/// loudly instead of degrading).
#[test]
fn the_activation_request_carries_the_identity() {
    let contract = read(&repo_root().join("client/src-tauri/src/locus/contract.rs"));

    for field in ["verifier", "device_id", "store"] {
        assert!(
            contract.contains(&format!("pub {field}: &'a str")),
            "`CodeRequest` no longer carries `{field}`. The hub cannot register an \
             identity it is not sent, so auto-sign-in and reinstall-survival both \
             go dead again."
        );
    }

    assert!(
        contract.contains(r#"#[serde(skip_serializing_if = "str::is_empty")]"#),
        "the identity fields are no longer omitted when empty. An old client sends \
         no identity at all, and the hub's shape check must see absent fields, not \
         empty strings."
    );
}

/// The activation command must pass the resolved identity, not a default.
///
/// A `CodeRequest` that carries the fields but is constructed with
/// `IdentityWire::default()` would compile, pass the two checks above, and
/// register nobody — the same "the field exists but the value never arrives"
/// failure that made `recognised` dead state in `main.tsx`.
#[test]
fn the_activation_command_passes_the_resolved_identity() {
    let cmd = read(&repo_root().join("client/src-tauri/src/cmd/locus.rs"));

    assert!(
        cmd.contains("activation::activate_with_identity("),
        "`locus_activate` no longer calls `activate_with_identity`. Calling the \
         plain `activate` compiles and behaves exactly as before the fix — no \
         identity is registered and recognition stays dead."
    );
    assert!(
        cmd.contains("identity.verifier()"),
        "`locus_activate` builds an identity without the verifier. The hub's \
         registration is keyed on `sha256(secret)`, so an identity sent without \
         it cannot be stored."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// The binding key: what a code is bound to must be stable, and the same on
// both sides
// ─────────────────────────────────────────────────────────────────────────────

/// A code is bound to the device's **durable identity**, not the hardware
/// fingerprint.
///
/// This is the "codes become invalid for the very devices they were bound to"
/// bug, pinned. The hub refuses a code with `403 "Code bound to another device"`
/// whenever the value it is sent differs from the one it bound
/// (`activation.pb.js`). So the binding value MUST be stable across launches,
/// and the hardware fingerprint is not — `device::fingerprint()` re-derives on
/// every launch from MAC, disk serial and board UUID.
///
/// The guard is two-sided on purpose: it asserts that the client sends the
/// identity's id AND that the identity resolver reuses a stored identity
/// verbatim. Asserting either alone would pass with the other half reverted —
/// the client could send a stable value that the resolver still re-derives, or
/// resolve durably and then send the volatile fingerprint.
#[test]
fn a_code_binds_to_the_durable_identity_not_the_hardware_fingerprint() {
    let cmd = read(&repo_root().join("client/src-tauri/src/cmd/locus.rs"));
    let identity = read(&repo_root().join("client/src-tauri/src/locus/identity.rs"));
    let runtime = read(&repo_root().join("client/src-tauri/src/locus/runtime.rs"));

    // (1) The client half: the activation command binds to the identity's id.
    assert!(
        cmd.contains("identity.binding_id()"),
        "`locus_activate` no longer derives the binding value from the identity. \
         If it sends `device::fingerprint()` again, the value is re-derived from \
         hardware on every launch and the student's own code comes back 403 the \
         moment that hardware reports differently."
    );
    assert!(
        !cmd.contains("let fingerprint = device::fingerprint()"),
        "`locus_activate` (or a sibling command) computes the binding value from \
         `device::fingerprint()`. That is the re-derived hardware hash the whole \
         change removed — a code bound to it drifts with the hardware."
    );

    // The other binding sites must agree, or one of them re-introduces the drift.
    assert!(
        runtime.contains("identity.binding_id()"),
        "the heartbeat credential path still uses the hardware fingerprint for \
         the binding value, so a device can bind with the identity and then \
         re-present the fingerprint — which is the drift, moved rather than fixed."
    );

    // (2) The resolver half: a stored identity wins over changed hardware.
    assert!(
        identity.contains("pub fn binding_id(&self) -> String"),
        "`DeviceIdentity::binding_id` is gone. It is the single definition of \
         what a code binds to; without it each call site re-decides, which is how \
         the two identifiers diverged in the first place."
    );
    assert!(
        identity.contains("**A stored identity wins.**"),
        "the resolver's \"a stored identity wins\" rule is no longer documented. \
         That rule IS the durability guarantee — re-deriving on a launch where a \
         stored identity exists is the original bug."
    );
}

/// The binding value the client sends for a device must be the value the hub
/// recognises that device by.
///
/// A weaker, still-useful agreement: activation sends `fingerprint` (the binding
/// value) AND the `verifier` that `device_identities` is keyed on, in the same
/// request. The migration path on the hub matches them to rebind a code whose
/// fingerprint predates this change — so if the client stops sending both, that
/// path silently stops firing and every already-bound device 403s on update.
#[test]
fn the_activation_request_carries_both_the_binding_value_and_the_verifier() {
    let activation = read(&repo_root().join("client/src-tauri/src/locus/activation.rs"));
    let hook = activation_hook();

    assert!(
        activation.contains("fingerprint,") && activation.contains("verifier: &identity.verifier"),
        "`CodeRequest` must carry the binding `fingerprint` and the `verifier` in \
         the same call. The hub's binding-key migration proves \"same device\" with \
         the verifier when the fingerprint has changed; drop either and the \
         migration cannot fire."
    );

    // The hub half of the same agreement.
    //
    // Assert the CALL SITE, not merely that the helper is defined. An earlier
    // version of this guard checked `contains("migrateBindingIfSameDevice")`,
    // which a *definition* satisfies even when the call is removed — so the
    // guard passed with the migration inert, which is exactly the "a check that
    // cannot fail reads like a check that passed" trap. The call takes the
    // incoming fingerprint (`fp`), so match the full expression.
    assert!(
        hook.contains("if (migrateBindingIfSameDevice(rec, verifier, fp)) {"),
        "the hub no longer CALLS the binding-key migration. A device bound \
         before this change presents the new id on its first post-update \
         activation and would 403 with its own code — the fix would break every \
         already-bound device at once. (Defining the helper is not enough: it \
         must be called where the fingerprint mismatches.)"
    );
    assert!(
        hook.contains("migrateBindingIfSameDevice(rec, verifier, fp)")
            && hook.contains("return e.json(403, {code:403, message:\"Code bound to another device\"})"),
        "the migration is not positioned before the 403. It must be consulted \
         when the fingerprint does not match and only fall through to the refusal \
         when it proves nothing — otherwise the migration is dead code."
    );
    assert!(
        hook.contains("ident.getString(\"code\") !== codeRec.getString(\"code\")"),
        "the migration no longer checks that the identity ALREADY names this \
         code. Without that check the endpoint would let any registered device \
         graft any code onto itself."
    );
}
