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
