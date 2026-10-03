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

/// Source with `//` line comments removed.
///
/// The absence assertions below must not be satisfied (or defeated) by a
/// tombstone comment that *names* the removed symbol to explain why it is gone.
/// Stripping comments first means a check fails only when real code carries the
/// name — which is what "removed" means.
fn read_code(path: &Path) -> String {
    read(path)
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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
// The suspension 403 substring contract — still the fragile one
// ─────────────────────────────────────────────────────────────────────────────

/// The exact 403 message that means "an operator suspended this code".
///
/// The word `suspended` is not decoration — it is the entire mechanism by which
/// a deployed client tells a suspension apart from every other 403. The
/// device-binding 403 that used to share this status is gone (codes are no
/// longer tied to a device), so a suspension is now the ONLY 403 the hub sends
/// here — but the substring test in the client is unchanged and still deployed,
/// so the wording is still frozen.
#[test]
fn the_suspended_403_keeps_its_wording() {
    assert!(
        activation_hook().contains(r#"e.json(403, {code:403, message:"Code suspended"})"#),
        "the hub no longer emits the literal 403 message \"Code suspended\". The \
         client classifier is a case-insensitive search for the substring \
         `suspended`; removing that word makes the hub's suspension refusal read \
         as a generic refusal on every deployed build."
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
    let has_suspension_403 = hook
        .lines()
        .filter(|line| line.contains("e.json(403"))
        .any(|line| line.to_ascii_lowercase().contains("suspended"));

    assert!(
        has_suspension_403,
        "no 403 emitted by the activation hook contains the substring \
         `suspended`. The client's classifier (`classify` in \
         locus/activation.rs) uses exactly that substring to separate a \
         suspension from any other refusal, and deployed clients cannot be \
         changed."
    );
}

/// No hook may emit the retired device-binding refusal.
///
/// `Code bound to another device` was a 403 the client disambiguated by
/// substring. With codes no longer device-bound, no hook may emit it: a
/// reappearance would mean the retired model had crept back into the activation
/// path, and it would read as a suspension or a generic refusal on a deployed
/// client.
#[test]
fn the_retired_binding_refusal_is_gone() {
    let hook = activation_hook();
    assert!(
        !hook.contains("Code bound to another device"),
        "the activation hook still emits the retired \"Code bound to another \
         device\" refusal. Codes are single-use and not tied to a device; this \
         message has no condition left to describe."
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

    // 404 -> NotFound, 410 -> Expired, 429 -> RateLimited
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

/// A second activation of the same code is NOT a refusal — it restores access.
///
/// This is the contract that replaced device binding. A student reinstalling, or
/// moving to a new machine, re-enters the same code; the hub must answer it as a
/// success ("Already activated", with the tier config), not as an error. If this
/// ever became a refusal, every reinstall would be a support call — which is the
/// exact failure the client-side persistence of the code exists to prevent.
#[test]
fn a_repeat_activation_restores_rather_than_refuses() {
    let hook = activation_hook();

    assert!(
        hook.contains(r#"message:"Already activated""#),
        "the hub no longer answers a repeat activation with \"Already activated\". \
         Re-activating the same code is the recovery path for a reinstall or a new \
         machine, and it must succeed."
    );
    // And it must carry the tier config, so a reinstalling student's Connect has
    // something to dial.
    assert!(
        hook.contains("alreadyRedeemed"),
        "the hub no longer distinguishes a first redemption from a repeat one. \
         Without that split the two paths cannot report different messages, and a \
         returning student would be told 'Activation successful' for a code they \
         already had."
    );
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
        "suspended",
        "expired",
        "not_found",
    ] {
        assert!(
            hook_emits_status(&hook, status),
            "the lookup hook no longer emits status \"{status}\". The client's \
             `LookupStatus` enum deserializes these exact snake_case strings; a \
             rename degrades to `Unknown` on deployed builds."
        );
    }
}

/// The status the hub sends for an already-redeemed code must be one the
/// DEPLOYED clients already treat as ready.
///
/// # This is the defect that shipped, pinned
///
/// The hub began sending a NEW name (`already_used`) for a redeemed code. A
/// deployed client's `LookupStatus` is an allow-list with a `#[serde(other)]`
/// catch-all, so the unknown name became `Unknown`, and the deployed classifier
/// read `Unknown` as NOT ready — the client refused to call `/api/activate` at
/// all. A student who reinstalled could not re-enter their code, and the only
/// remedy was an operator releasing it. Found live 2026-10-03.
///
/// So this asserts the AGREEMENT between the two sides rather than each alone:
/// whatever the hook emits for a redeemed code must be `bound_this_device`,
/// because that is the one frozen name every deployed build maps to
/// `ready: true`. A new name here is a dead end for clients that cannot update
/// their way out of it.
#[test]
fn a_redeemed_code_reports_a_status_deployed_clients_accept() {
    let hook = lookup_hook();

    assert!(
        hook.contains(r#"resp.status = "bound_this_device""#),
        "the lookup hook no longer reports a redeemed code as `bound_this_device`. \
         That frozen name is the only status a DEPLOYED client maps to \
         `ready: true`; any other name reaches it as `Unknown` and the client \
         will refuse to activate — a student stuck at the code prompt with a \
         valid code, unable even to update."
    );

    assert!(
        !hook.contains(r#""already_used""#),
        "the lookup hook still emits `already_used`, which deployed clients read \
         as `Unknown` (= not ready) and which blocks activation. See the note in \
         code_lookup.pb.js: the meaning is right, the WIRE NAME is what broke."
    );
}

/// The client must PROCEED on an unknown lookup status, never block.
///
/// The other half of the same agreement. Even with the hub's name pinned above,
/// a future hub will eventually send something this build does not know, and the
/// lookup must not be the thing that strands a student: `/api/activate` is the
/// authority and answers the same question.
#[test]
fn an_unknown_lookup_status_does_not_block_activation() {
    let activation = read_code(&repo_root().join("client/src-tauri/src/locus/activation.rs"));

    // Find the Unknown arm and assert it is `true` (proceed), not `false`.
    let unknown_arm = activation
        .lines()
        .find(|line| line.contains("LookupStatus::Unknown =>"))
        .unwrap_or_else(|| panic!("no `LookupStatus::Unknown` arm found in classify_lookup"));

    assert!(
        unknown_arm.contains("(true,"),
        "`LookupStatus::Unknown` no longer marks the code as ready to try. An \
         unrecognised status must fall through to `/api/activate` — blocking on \
         it is how a client meeting a newer hub becomes permanently stuck at the \
         code prompt. Arm was: {unknown_arm}"
    );
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
// Device recognition — RETIRED (removed 2026-10)
// ─────────────────────────────────────────────────────────────────────────────

// This section used to pin the `/api/device-recognise` contract: that the
// endpoint never returned the activation code, that it answered every negative
// identically, that the heartbeat accepted a session token, and that a code
// bound to the durable device identity. Every one of those properties described
// a mechanism that no longer exists.
//
// Device recognition was removed from both sides. The client no longer presents
// a device identity, the hub no longer stores one, and there is no session
// token: a client authenticates with its activation code, which it persists to a
// machine-scoped store so a reinstall does not lose it (see
// `client/src-tauri/src/locus/credential.rs`).
//
// What replaces the old guarantees is asserted below: the endpoint is gone, and
// the retired machinery cannot still be live.

/// The recognition response must never have existed as a live code path.
///
/// The endpoint is kept as a tombstone — it answers a uniform `unknown` for
/// deployed 3.2.x clients, so they fall through to the code prompt instead of
/// logging a 404. What it must NOT do is any work: no database read, no token,
/// no code in any field.
#[test]
fn the_recognition_endpoint_is_a_tombstone() {
    let hook = read(&repo_root().join("server/pb_hooks/device_recognise.pb.js"));

    assert!(
        hook.contains(r#"status: "unknown""#) || hook.contains(r#"status:"unknown""#),
        "the recognition tombstone no longer answers `unknown`. A deployed client \
         expects that exact status on this route; removing it turns a harmless \
         no-op into a parse failure it will log as a transport error."
    );
    // It must do no lookups at all. Any of these strings appearing means the
    // retired machinery is live again.
    for forbidden in ["device_identities", "token_hash", "findFirstRecordByFilter", "verifier"] {
        assert!(
            !hook.contains(forbidden),
            "the recognition tombstone still references `{forbidden}`. It must do \
             no work at all — no identity lookup, no token, no verifier. Its only \
             job is to answer `unknown` to clients that predate the removal."
        );
    }
}

/// The hub must have no live device-identity or token machinery.
///
/// This is the two-sided removal contract: the client half is asserted by the
/// absence of `locus_recognise` (checked below), and this is the hub half. If a
/// hook still reads or writes either collection, the feature is half-removed and
/// the two sides can disagree.
#[test]
fn the_hub_has_no_live_device_identity_machinery() {
    let heartbeat = read(&repo_root().join("server/pb_hooks/heartbeat.pb.js"));
    let activation = activation_hook();

    assert!(
        !heartbeat.contains("device_identities") && !heartbeat.contains("token_hash"),
        "the heartbeat hook still resolves a session token against \
         `device_identities`. Nothing mints a token any more, so this code path \
         can only be dead — and dead code in the authentication path is where a \
         future change reintroduces the feature by accident."
    );
    assert!(
        !heartbeat.contains("data.token"),
        "the heartbeat hook still reads a `token` from the request body. The \
         client sends none; the field is a frozen wire name kept only so a \
         deployed client's request is not rejected."
    );
    assert!(
        !activation.contains("registerIdentity") && !activation.contains("device_identities"),
        "the activation hook still registers a device identity. Recognition is \
         gone, so this write has no reader and the collection it targets is \
         retired."
    );
}

/// The client must have no recognition command or symbol left.
///
/// The client half of the removal, asserted as the absence of the symbols. A
/// half-removal — the command stays registered but is never called — would read
/// as clean while leaving a dead IPC surface a later change could revive.
#[test]
fn the_client_has_no_recognition_surface() {
    let cmd = read_code(&repo_root().join("client/src-tauri/src/cmd/locus.rs"));
    let lib = read_code(&repo_root().join("client/src-tauri/src/lib.rs"));

    for forbidden in ["locus_recognise", "RecognitionResult", "resolve_identity"] {
        assert!(
            !cmd.contains(forbidden),
            "`{forbidden}` is still present in cmd/locus.rs. Device recognition \
             was removed from the client; a leftover symbol is a dead surface."
        );
    }
    assert!(
        !lib.contains("locus_recognise"),
        "`locus_recognise` is still registered in the Tauri handler list. The \
         command no longer exists, so this would fail to build — but the point \
         is that the frontend can still invoke a name the hub no longer serves."
    );
}

/// The activation request must NOT carry an identity, and must still carry the
/// frozen empty fields a deployed hub expects.
#[test]
fn the_activation_request_omits_the_identity() {
    let contract = read_code(&repo_root().join("client/src-tauri/src/locus/contract.rs"));

    // The recognised types are gone.
    for forbidden in ["RecogniseRequest", "RecogniseResponse", "pub enum Recognised"] {
        assert!(
            !contract.contains(forbidden),
            "`{forbidden}` is still defined in contract.rs. The recognition wire \
             types were removed with the feature."
        );
    }
}

/// A code is single-use, and its release path is the only thing that clears the
/// stamp.
///
/// This is the hub-side half of the single-use rule. `activated_at` is the whole
/// record of redemption; the console and the admin endpoint release a code by
/// clearing it. A hook outside those two paths clearing it would make a code a
/// student is using look unused — and it could then be sold twice.
#[test]
fn only_the_release_paths_clear_the_single_use_stamp() {
    // The consistency script enforces this across the whole hook directory; this
    // asserts the two release paths actually exist, so the rule has somewhere
    // legitimate to be enforced.
    let unbind = read(&repo_root().join("server/pb_hooks/admin_unbind.pb.js"));
    let console = read(&repo_root().join("server/pb_hooks/admin_console.pb.js"));

    assert!(
        unbind.contains(r#"set("activated_at", null)"#),
        "the admin release endpoint no longer clears `activated_at`, so a code \
         can never be released for a different student."
    );
    assert!(
        console.contains(r#"set("activated_at", null)"#),
        "the console's codes.unbind no longer clears `activated_at`, so an \
         operator cannot release a code from the console."
    );
}

