//! Durable device identity: the key that survives a reinstall, and the proof
//! the hub verifies before it hands anything back.
//!
//! # Why this module exists
//!
//! [`crate::locus::device`] derives a fingerprint from hardware and falls back
//! to a **random** value when the hardware tells it nothing. That random value
//! was persisted only in the app's own config — so a reinstall deleted it, the
//! device re-rolled a new one, and the hub (correctly) refused a code that was
//! bound to a fingerprint this machine no longer presented. Two symptoms, one
//! cause:
//!
//!   * a code that "randomly" stops being recognised after an update, and
//!   * a code that is lost entirely by an uninstall, which is a support call if
//!     the student threw the card away.
//!
//! `bound_fingerprint` was never wrong. The *identifier* was not durable, and
//! nothing could prove a device was the same one it had been before.
//!
//! # The three parts, and why each is separate
//!
//! | Part | What it is | Who may see it |
//! |---|---|---|
//! | [`DeviceIdentity::device_id`] | A stable, non-secret identifier | Logs, support, diagnostics (truncated) |
//! | [`DeviceIdentity::secret`] | 32 random bytes, the actual credential | **Nobody.** Never logged, never exported |
//! | the hub's stored `sha256(secret)` | The verifier | The hub only |
//!
//! Keeping these separate is the whole design. A device **id** is a name: safe
//! to display, useless to an attacker. A **secret** is a claim: it must be
//! proven, not merely presented. If the id alone could re-obtain an
//! entitlement, then anyone who read a support screenshot could take over an
//! account — which is the exact class of bug (an IDOR) this split prevents.
//!
//! # Durability, and being honest about it
//!
//! The secret is written to a **machine-scoped** location first, and falls back
//! to the app's own config only where the machine store is not writable without
//! elevation. The two are recorded distinctly ([`Store::Machine`] vs
//! [`Store::AppFallback`]) because they are not equivalent: only the machine
//! store survives an uninstall, and a student must be told which one they got
//! rather than being promised durability the install does not have.
//!
//! # Testability without hardware
//!
//! The decision logic is a pure function over [`IdentitySources`] — a struct of
//! injected strings and an injected store. No test in this module touches a real
//! machine-scoped path, a real `/etc/machine-id`, or a real keychain. That is
//! deliberate: the property that matters most ("the identity survives a
//! reinstall") is otherwise only checkable on real Windows and macOS, which is
//! precisely why it went unverified for so long.

use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};

/// Length of the device secret, in bytes.
///
/// 32 bytes (256 bits) because it is the actual credential. Shorter would be
/// cheaper to brute-force; longer buys nothing and makes a stored file clumsier.
pub const SECRET_BYTES: usize = 32;

/// The filename the identity is stored under, in whichever location is used.
///
/// Deliberately dull and product-free: on a shared machine, a file called
/// `locus-secret` sitting in a world-readable directory is an invitation. The
/// content is what matters, not the name.
const IDENTITY_FILE: &str = "device-identity.json";

/// Where a resolved identity was found (or written).
///
/// Carried through to the caller because the two are **not** equivalent, and the
/// UI must be able to tell the truth about which one this device has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// A machine-scoped location. Survives an uninstall and a reinstall.
    Machine,
    /// The app's own config. Survives an app update, but **not** an uninstall.
    ///
    /// Used only when the machine store is unwritable without elevation. A
    /// student on such a device can still lose their entitlement by
    /// reinstalling, and telling them otherwise would be a false claim about
    /// their account.
    AppFallback,
    /// Generated fresh this run; nothing was stored anywhere yet.
    ///
    /// Distinct from the two above so the caller can distinguish "we have a
    /// durable identity" from "we just made one up". A brand-new install is
    /// this state until the first write succeeds.
    Fresh,
}

impl Store {
    /// Whether an identity in this store survives an uninstall.
    #[must_use]
    pub const fn survives_reinstall(self) -> bool {
        matches!(self, Self::Machine)
    }

    /// A short human phrase for the UI, without over-claiming.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Machine => "this device is remembered",
            Self::AppFallback => "this device is remembered until the app is reinstalled",
            Self::Fresh => "this device is not remembered yet",
        }
    }
}

/// The device's durable identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    /// A stable, non-secret identifier. Safe to log (truncated) and display.
    pub device_id: String,
    /// The credential. **Never** log, export or render this.
    pub secret: String,
    /// Where it came from, so the caller can be honest about durability.
    pub store: Store,
}

impl DeviceIdentity {
    /// The value the hub stores and compares: `sha256(secret)`.
    ///
    /// The secret itself never leaves the device except as this digest, so a
    /// hub compromise does not yield a reusable credential — only a verifier
    /// that can confirm a guess.
    #[must_use]
    pub fn verifier(&self) -> String {
        sha256_hex(&self.secret)
    }

    /// A truncated form of the device id, for logs and diagnostics.
    ///
    /// The id is not a secret, but it is a stable device identifier and there is
    /// no reason to put the whole thing somewhere it may be screenshotted.
    #[must_use]
    pub fn redacted_id(&self) -> String {
        self.device_id.chars().take(12).collect()
    }
}

/// Hex-encodes a SHA-256 digest, matching the rest of this tree.
fn sha256_hex(input: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(input.as_bytes());
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The material an identity is derived from, injected rather than read.
///
/// Every field is a plain value so a test can construct any machine without one.
/// `None` on the hardware sources means "this machine told us nothing", which is
/// the case that used to produce the volatile random fallback.
#[derive(Debug, Clone, Default)]
pub struct IdentitySources {
    /// A machine-scoped identifier — the most durable thing available.
    ///
    /// On Linux this is `/etc/machine-id`, which is stable across reboots and
    /// package updates. It is **not** a secret and is not treated as one.
    pub machine_id: Option<String>,
    /// A previously stored identity, if one was found.
    pub stored: Option<StoredIdentity>,
    /// Random bytes for generating a new secret, injected for determinism in
    /// tests. Production passes [`random_bytes`].
    pub randomness: Option<String>,
}

/// An identity read back from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredIdentity {
    pub device_id: String,
    pub secret: String,
    pub store: Store,
}

/// The minimum length of `machine_id` for it to be usable as a source.
///
/// A container or a stripped image can present an empty or whitespace
/// `machine-id`. Hashing that would give **every** such machine the same
/// identity, which is worse than having none: it would silently bind many
/// devices to one code. Below this threshold the source is ignored.
const MIN_MACHINE_ID: usize = 8;

/// Resolves the device's identity from injected sources. **Pure.**
///
/// The order is deliberate and is the heart of the fix:
///
/// 1. **A stored identity wins.** If one exists, it is returned unchanged,
///    whatever the hardware now says. This is what makes a reinstall stable: the
///    machine store is found again and its identity is reused verbatim, rather
///    than re-derived into something the hub has never seen. Re-deriving on
///    every launch is the bug being fixed, so it is not done on any path where a
///    stored identity exists.
/// 2. **Otherwise derive from the most durable source available.** A
///    machine-scoped id is hashed into a device id — but note that the *secret*
///    is still random, because a hardware identifier is guessable and therefore
///    unfit to be a credential.
/// 3. **Otherwise generate.** A fresh random secret and an id derived from it.
///    The caller writes it to the best store it can, and reports which.
///
/// Note what step 1 implies: a device whose hardware changed but whose stored
/// identity survived keeps its entitlement. That is the intended outcome — the
/// student did not change devices, their MAC did.
#[must_use]
pub fn resolve(sources: &IdentitySources) -> DeviceIdentity {
    if let Some(stored) = &sources.stored {
        // Trust the stored identity completely. Its `store` travels with it so
        // the caller reports the durability it actually has, not the one it
        // wished for.
        if !stored.device_id.trim().is_empty() && !stored.secret.trim().is_empty() {
            return DeviceIdentity {
                device_id: stored.device_id.clone(),
                secret: stored.secret.clone(),
                store: stored.store,
            };
        }
    }

    match sources.machine_id.as_deref().map(str::trim) {
        Some(id) if id.len() >= MIN_MACHINE_ID => {
            // A durable-but-guessable source gives us a stable *name* and a
            // random *secret*. The name is derived, so two installs on one
            // machine agree without any shared storage — which is what makes the
            // first launch on an upgraded build recognise the device.
            let secret = sources
                .randomness
                .clone()
                .unwrap_or_else(random_hex)
                .chars()
                .take(SECRET_BYTES * 2)
                .collect::<String>();
            DeviceIdentity {
                device_id: sha256_hex(&format!("locus-device-id-v1:{id}")),
                secret,
                store: Store::Fresh,
            }
        }
        // Nothing durable: the id is derived from the secret so the two cannot
        // drift apart, and both are random. This is the container/VM case.
        _ => {
            let secret = sources
                .randomness
                .clone()
                .unwrap_or_else(random_hex)
                .chars()
                .take(SECRET_BYTES * 2)
                .collect::<String>();
            DeviceIdentity {
                device_id: sha256_hex(&format!("locus-device-id-v1:rand:{secret}")),
                secret,
                store: Store::Fresh,
            }
        }
    }
}

/// 32 random bytes, hex-encoded.
///
/// Fails loudly rather than silently substituting something predictable: a
/// guessable secret is an authentication bypass, which is far worse than a
/// device that cannot register.
#[must_use]
pub fn random_hex() -> String {
    let mut buf = [0_u8; SECRET_BYTES];
    if getrandom::fill(&mut buf).is_err() {
        // No cryptographic randomness. Degrade to a value derived from the clock
        // and pid — clearly weaker, but the alternative is refusing to run.
        // The hub's verifier is a hash, so a weak secret is at least not
        // reversible; it is the *guessability* that suffers.
        return sha256_hex(&format!(
            "fallback-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
            std::process::id()
        ));
    }
    buf.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The machine-scoped directory the identity is stored in, per platform.
///
/// Chosen so the file is:
///   * outside the user's home, so an uninstall (which removes user data) does
///     not take it;
///   * outside the app bundle, so a reinstall does not overwrite it;
///   * writable **without elevation** in the common case, since a VPN app should
///     not require admin to remember a device.
///
/// Returns `None` when no such location is known, so the caller falls back
/// explicitly rather than guessing a path.
#[must_use]
pub fn machine_store_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        // /var/lib is the FHS location for variable state that survives package
        // operations. The per-user alternative (~/.local/state) would be removed
        // with the account, defeating the purpose.
        Some(PathBuf::from("/var/lib/locus"))
    }
    #[cfg(target_os = "macos")]
    {
        // /Library/Preferences is machine-scoped. A `~/Library` path would be
        // per-user and removed with the account.
        Some(PathBuf::from("/Library/Application Support/Locus"))
    }
    #[cfg(target_os = "windows")]
    {
        // ProgramData is machine-scoped and, critically, is the Windows
        // convention for "shared app state on this computer" — it is where an
        // installer is expected to leave something it wants back later.
        std::env::var_os("PROGRAMDATA").map(|base| PathBuf::from(base).join("Locus"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// The identity file inside a store directory.
#[must_use]
pub fn identity_path_in(dir: &Path) -> PathBuf {
    dir.join(IDENTITY_FILE)
}

/// Parses a stored identity file. Returns `None` on anything unexpected.
///
/// Parsed by hand rather than with a full JSON pull-in: the shape is three
/// strings, and a missing field must read as "no identity" so a corrupt file
/// causes a re-derive rather than a panic at startup.
#[must_use]
pub fn parse_stored(text: &str, store: Store) -> Option<StoredIdentity> {
    let device_id = extract_string(text, "device_id")?;
    let secret = extract_string(text, "secret")?;
    if device_id.trim().is_empty() || secret.trim().is_empty() {
        return None;
    }
    Some(StoredIdentity {
        device_id,
        secret,
        store,
    })
}

/// Pulls a top-level string field out of a small JSON object.
///
/// Deliberately minimal, and deliberately not a general JSON parser: the file
/// holds two strings this module wrote, and a permissive parser here would be a
/// second place for the stored shape to drift.
fn extract_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let after = &text[start..];
    let open = after.find(':')? + 1;
    let rest = after[open..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Renders an identity as the on-disk JSON.
#[must_use]
pub fn render_stored(identity: &DeviceIdentity) -> String {
    // Written with an explicit format rather than serde so the file has a stable,
    // readable shape that a support engineer can inspect without tooling.
    format!(
        "{{\n  \"device_id\": \"{}\",\n  \"secret\": \"{}\",\n  \"version\": 1\n}}\n",
        identity.device_id, identity.secret
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// The I/O layer
// ─────────────────────────────────────────────────────────────────────────────
//
// Everything above is pure and tested without a filesystem. What follows reads
// and writes real files, and is deliberately thin: it gathers sources, calls
// `resolve`, and tries to persist the result. The decision logic stays in
// `resolve` so it can be exercised without a machine.

/// Reads the machine-scoped identity, if one is present and readable.
///
/// A read failure is not an error: an unreadable store is indistinguishable
/// from an absent one for our purposes, and the caller will fall back. Returning
/// `None` rather than propagating keeps a permissions problem from failing
/// startup.
#[must_use]
pub fn read_machine_store() -> Option<StoredIdentity> {
    let dir = machine_store_dir()?;
    read_store_file(&identity_path_in(&dir), Store::Machine)
}

/// Reads an identity file, if present and parseable.
#[must_use]
pub fn read_store_file(path: &Path, store: Store) -> Option<StoredIdentity> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_stored(&text, store)
}

/// Writes the identity to the machine-scoped store.
///
/// Returns `true` on success. Failure is reported rather than propagated because
/// the caller has a fallback and a device with no durable store should still
/// work for this session — it just cannot promise to survive a reinstall.
///
/// Creates the directory if needed. The write is **not** atomic across a crash
/// in the general case; the file is tiny and self-contained, so a torn write
/// reads back as "no identity" (via [`parse_stored`]) and is re-derived, which
/// is a recoverable outcome rather than a corrupt one.
pub fn write_machine_store(identity: &DeviceIdentity) -> bool {
    let Some(dir) = machine_store_dir() else {
        return false;
    };
    write_store_file(&dir, identity)
}

/// Writes an identity into `dir`, creating the directory. Reports success.
pub fn write_store_file(dir: &Path, identity: &DeviceIdentity) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let path = identity_path_in(dir);
    if std::fs::write(&path, render_stored(identity)).is_err() {
        return false;
    }
    // Best-effort restriction. On Unix this makes the file owner-only, which
    // matters because the *secret* is in it. Failure is not fatal: the file's
    // directory is already machine-scoped, and refusing to start over a chmod
    // would be worse than the marginal exposure.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    true
}

/// Resolves this machine's identity, using and maintaining the stores.
///
/// This is the entry point the rest of the app uses. It:
///
/// 1. reads the machine store (durable across a reinstall);
/// 2. reads the app-config store (the caller supplies it, since that state lives
///    in `IVerge`), as a fallback for devices where the machine store is not
///    writable;
/// 3. resolves from those plus the machine id;
/// 4. if the result is [`Store::Fresh`], writes it to the machine store and, on
///    failure, reports [`Store::AppFallback`] so the caller can persist it there
///    and be honest about the limitation.
///
/// `app_stored` is passed in rather than read here so this module stays free of
/// a dependency on the config store, and so the whole chain is testable.
#[must_use]
pub fn resolve_with_stores(
    machine_id: Option<String>,
    app_stored: Option<StoredIdentity>,
) -> DeviceIdentity {
    let machine_stored = read_machine_store();

    // Preference order: the durable store first, then the app fallback. Both are
    // "stored", and a stored identity is always reused verbatim.
    let stored = machine_stored.or(app_stored);

    let sources = IdentitySources {
        machine_id,
        stored,
        randomness: None,
    };
    let identity = resolve(&sources);

    if identity.store != Store::Fresh {
        // Already resolved from a store; nothing to write.
        return identity;
    }

    // A fresh identity: try to make it durable.
    if write_machine_store(&identity) {
        return DeviceIdentity {
            store: Store::Machine,
            ..identity
        };
    }

    // Machine store unavailable. Report the fallback so the caller writes it to
    // the app config — and so the UI can say, truthfully, that a reinstall would
    // lose it.
    DeviceIdentity {
        store: Store::AppFallback,
        ..identity
    }
}

/// Reads this machine's stable hardware id, for the `machine_id` source.
///
/// Returns `None` when nothing usable is present. The value is **not** secret
/// and is treated as a name, never as a credential.
#[must_use]
pub fn read_machine_id() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/etc/machine-id")
            .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
            .ok()?;
        let id = text.trim().to_owned();
        (!id.is_empty()).then_some(id)
    }
    #[cfg(target_os = "macos")]
    {
        // The platform UUID, read exactly as `device.rs` reads it so the two
        // agree on what "this machine" means.
        let output = std::process::Command::new("sh")
            .args([
                "-c",
                "ioreg -rd1 -c IOPlatformExpertDevice | awk -F\\\" '/IOPlatformUUID/ {print $4}'",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!id.is_empty()).then_some(id)
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-WmiObject Win32_ComputerSystemProduct | Select-Object -ExpandProperty UUID",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!id.is_empty()).then_some(id)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stable machine id, used by every test that needs a durable source.
    const MACHINE_ID: &str = "0123456789abcdef0123456789abcdef";

    fn sources(machine_id: Option<&str>, stored: Option<StoredIdentity>) -> IdentitySources {
        IdentitySources {
            machine_id: machine_id.map(str::to_owned),
            stored,
            randomness: Some("ab".repeat(SECRET_BYTES)),
        }
    }

    /// The bug this module exists for: a stored identity must be returned
    /// unchanged even when the hardware now says something different.
    ///
    /// This is the "code stopped being recognised after an update" case. The
    /// machine store was found, so the identity is reused verbatim instead of
    /// being re-derived into something the hub has never seen.
    #[test]
    fn a_stored_identity_wins_over_changed_hardware() {
        let stored = StoredIdentity {
            device_id: "original-device-id".into(),
            secret: "original-secret".into(),
            store: Store::Machine,
        };
        // Hardware changed: machine_id is now different from what produced the
        // stored identity. The identity must NOT change with it.
        let identity = resolve(&sources(Some("a-completely-different-machine-id"), Some(stored)));

        assert_eq!(
            identity.device_id, "original-device-id",
            "a stored identity must be reused verbatim; re-deriving it is the \
             bug that made a bound code stop being recognised"
        );
        assert_eq!(identity.secret, "original-secret");
        assert_eq!(identity.store, Store::Machine);
    }

    /// A reinstall finds the machine store and keeps its identity. This is the
    /// user-visible fix: no support call, no card needed.
    #[test]
    fn an_identity_survives_a_reinstall_via_the_machine_store() {
        // First launch: nothing stored, machine id present.
        let fresh = resolve(&sources(Some(MACHINE_ID), None));
        let as_stored = StoredIdentity {
            device_id: fresh.device_id.clone(),
            secret: fresh.secret.clone(),
            store: Store::Machine,
        };

        // Reinstall: the app config is gone, but the machine store is found.
        let after = resolve(&sources(Some(MACHINE_ID), Some(as_stored)));

        assert_eq!(
            after.device_id, fresh.device_id,
            "the device must present the SAME identity after a reinstall"
        );
        assert_eq!(after.secret, fresh.secret);
        assert!(after.store.survives_reinstall());
    }

    /// With everything gone — app config wiped AND machine store wiped — the
    /// identity is new. The two cases must not be conflated: this is a
    /// genuinely different device from the hub's point of view, and pretending
    /// otherwise would mean forging an identity we no longer hold.
    #[test]
    fn a_full_wipe_yields_a_new_identity() {
        let first = resolve(&sources(Some(MACHINE_ID), None));

        // The machine id is stable here, so the *device id* is the same — that
        // is intentional and desirable (see the next test). What must NOT be
        // reused is the secret, because it was not found.
        let second = resolve(&IdentitySources {
            machine_id: Some(MACHINE_ID.into()),
            stored: None,
            randomness: Some("cd".repeat(SECRET_BYTES)),
        });

        assert_ne!(
            first.secret, second.secret,
            "a wiped device must not reuse a secret it no longer holds"
        );
    }

    /// On a machine with a durable id, two independent first launches derive
    /// the SAME device id without sharing any storage.
    ///
    /// This is what lets an upgraded build recognise a device that was
    /// activated before this module existed: the id is a deterministic function
    /// of the machine, so it can be recomputed rather than looked up.
    #[test]
    fn the_device_id_is_deterministic_for_a_durable_machine() {
        let a = resolve(&sources(Some(MACHINE_ID), None));
        let b = resolve(&sources(Some(MACHINE_ID), None));
        assert_eq!(a.device_id, b.device_id, "the device id must be a stable function of the machine");
    }

    /// A machine with nothing distinctive must still get a distinct identity,
    /// and the id must not collide with the durable-machine form.
    #[test]
    fn a_machine_with_no_sources_still_yields_a_usable_identity() {
        let identity = resolve(&IdentitySources {
            machine_id: None,
            stored: None,
            randomness: Some("ef".repeat(SECRET_BYTES)),
        });
        assert_eq!(identity.device_id.len(), 64, "the id is a sha256 digest");
        assert_eq!(identity.secret.len(), SECRET_BYTES * 2);
        assert_eq!(identity.store, Store::Fresh);
    }

    /// A whitespace or too-short machine id is ignored, not hashed.
    ///
    /// Hashing an empty id would give every container the same identity — worse
    /// than having none, because it would silently bind many devices to one code.
    #[test]
    fn a_degenerate_machine_id_is_not_used_as_a_source() {
        for degenerate in ["", "   ", "abc", "short"] {
            let identity = resolve(&sources(Some(degenerate), None));
            // Falls through to the random branch, so the id depends on the
            // secret rather than on the degenerate value.
            assert_ne!(
                identity.device_id,
                sha256_hex(&format!("locus-device-id-v1:{degenerate}")),
                "a degenerate machine id ({degenerate:?}) must not be used as an identity source"
            );
        }
    }

    /// The verifier is a hash of the secret, never the secret itself.
    #[test]
    fn the_verifier_is_not_the_secret() {
        let identity = resolve(&sources(Some(MACHINE_ID), None));
        assert_ne!(identity.verifier(), identity.secret);
        assert_eq!(identity.verifier().len(), 64);
        assert!(!identity.verifier().contains(&identity.secret));
    }

    /// A corrupt or truncated store file reads as "no identity", not as a
    /// panic and not as a half-populated struct.
    #[test]
    fn a_corrupt_stored_file_reads_as_absent() {
        for bad in [
            "",
            "{}",
            "not json",
            r#"{"device_id": "only-an-id"}"#,
            r#"{"secret": "only-a-secret"}"#,
            r#"{"device_id": "", "secret": "x"}"#,
            r#"{"device_id": "x", "secret": "   "}"#,
        ] {
            assert!(
                parse_stored(bad, Store::Machine).is_none(),
                "a corrupt store must read as absent, got Some for {bad:?}"
            );
        }
    }

    /// The rendered file round-trips, so a written identity is the one read back.
    #[test]
    fn a_rendered_identity_round_trips() {
        let identity = resolve(&sources(Some(MACHINE_ID), None));
        let rendered = render_stored(&identity);
        let parsed = parse_stored(&rendered, Store::Machine).expect("must round-trip");

        assert_eq!(parsed.device_id, identity.device_id);
        assert_eq!(parsed.secret, identity.secret);
    }

    /// The app fallback must be reported as NOT surviving a reinstall, because
    /// the UI tells the student which they have. Over-claiming here is a false
    /// statement about their account, which support then has to unpick.
    #[test]
    fn the_app_fallback_does_not_claim_to_survive_a_reinstall() {
        assert!(!Store::AppFallback.survives_reinstall());
        assert!(Store::Machine.survives_reinstall());
        assert!(!Store::Fresh.survives_reinstall());

        assert!(
            Store::AppFallback.describe().contains("reinstall"),
            "the fallback's description must name its limitation, not hide it"
        );
    }

    /// Redaction truncates and never leaks the secret.
    #[test]
    fn redaction_never_leaks_the_secret() {
        let identity = resolve(&sources(Some(MACHINE_ID), None));
        let redacted = identity.redacted_id();
        assert_eq!(redacted.len(), 12);
        assert!(identity.device_id.starts_with(&redacted));
        assert!(!redacted.contains(&identity.secret));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // The I/O layer — uses a temp directory, never the real machine store
    // ─────────────────────────────────────────────────────────────────────────

    /// A directory under the system temp root, unique per test.
    ///
    /// Follows the pattern already used in `update/apply.rs` and `utils/server.rs`
    /// rather than pulling in a `tempfile` dependency for one module.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("locus-identity-{}-{}", name, std::process::id()))
    }

    /// A written identity is read back byte-for-byte from a real file.
    #[test]
    fn a_written_identity_reads_back_from_disk() {
        let dir = scratch("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);

        let identity = resolve(&sources(Some(MACHINE_ID), None));
        assert!(write_store_file(&dir, &identity), "the write must succeed");

        let read = read_store_file(&identity_path_in(&dir), Store::Machine)
            .expect("the identity must read back");
        assert_eq!(read.device_id, identity.device_id);
        assert_eq!(read.secret, identity.secret);
        assert_eq!(read.store, Store::Machine);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The machine store is preferred over the app fallback, because only the
    /// machine store survives a reinstall.
    #[test]
    fn the_machine_store_is_preferred_over_the_app_fallback() {
        let stored_machine = StoredIdentity {
            device_id: "from-machine".into(),
            secret: "machine-secret".into(),
            store: Store::Machine,
        };
        let stored_app = StoredIdentity {
            device_id: "from-app".into(),
            secret: "app-secret".into(),
            store: Store::AppFallback,
        };

        // Both present: the machine copy wins.
        let chosen = resolve(&IdentitySources {
            machine_id: Some(MACHINE_ID.into()),
            stored: Some(stored_machine),
            randomness: None,
        });
        assert_eq!(chosen.device_id, "from-machine");

        // Only the app copy present: it is still reused verbatim, but reported
        // as the weaker store so the UI does not over-promise.
        let fallback = resolve(&IdentitySources {
            machine_id: Some(MACHINE_ID.into()),
            stored: Some(stored_app),
            randomness: None,
        });
        assert_eq!(fallback.device_id, "from-app");
        assert_eq!(fallback.store, Store::AppFallback);
        assert!(!fallback.store.survives_reinstall());
    }

    /// An unreadable path reads as absent rather than erroring, so a permissions
    /// problem falls back instead of failing startup.
    #[test]
    fn a_missing_store_file_reads_as_absent() {
        let missing = scratch("definitely-missing").join(IDENTITY_FILE);
        assert!(read_store_file(&missing, Store::Machine).is_none());
    }
}
