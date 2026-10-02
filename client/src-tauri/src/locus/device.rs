//! Hardware fingerprint: a hash of whatever machine identifiers this OS exposes.
//!
//! Ports `legacy/wails-client/internal/activation/fingerprint_*.go`.
//!
//! # This is NOT the value an activation code is bound to
//!
//! It used to be, and that was the bug. This fingerprint is **re-derived on every
//! launch** from MAC address, disk serial and board UUID, so a NIC enumerating in
//! a different order, a disk serial becoming unreadable, or the `combine` chain
//! below degrading to a weaker rung all produce a *different* digest. Binding a
//! code to it meant a student's own laptop presented a different device after an
//! update, and the hub refused the code with `403 "Code bound to another
//! device"`.
//!
//! A code is bound to a device's **durable identity** — the persisted
//! `device_id` from [`crate::locus::identity`], which is written to a
//! machine-scoped store on first launch and reused verbatim thereafter. See
//! [`crate::locus::identity::DeviceIdentity::binding_id`] for the full argument.
//!
//! # What this value is still for
//!
//! * **Seeding a first-run identity** where the OS exposes no `machine-id`
//!   (see [`crate::locus::identity::resolve`]).
//! * **Diagnostics** — a one-shot "what does this machine report" probe.
//!
//! It is deliberately no longer on any activation or heartbeat path. The hash is
//! still cached for the process lifetime so that a *seeding* read is consistent
//! within a run, but it is neither persisted nor sent anywhere.
//!
//! Three properties were said to matter here, and they pull against each other:
//!
//! * **Stable** across app restarts, updates and reinstalls — which this value
//!   is *not*, and why it was retired as a binding key.
//! * **Distinct per machine**, or one code would work on the whole class.
//! * **Not trivially shareable**, but also **not PII** — diagnostics deliberately
//!   truncate it, and it must never be logged in full.

use sha2::{Digest as _, Sha256};
use std::sync::OnceLock;

/// Minimum length of the material that goes into a fingerprint.
///
/// Below this the sources are obviously empty (a VM with no disk serial, a
/// container with no MAC) and hashing them would produce the *same* fingerprint
/// for every such machine — worse than useless, because it silently binds many
/// devices to one code.
const MIN_ENTROPY: usize = 8;

/// The fingerprint for this machine, computed once and reused.
pub fn fingerprint() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(compute).clone()
}

/// Hex-encodes a SHA-256 digest, matching the digest formatting used elsewhere
/// in this tree.
fn sha256_hex(input: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(input.as_bytes());
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Combines whatever hardware identifiers are available into a fingerprint,
/// degrading through weaker combinations, and finally to a random value.
///
/// Returns `None` when nothing distinctive could be gathered, so the caller can
/// fall back explicitly rather than hashing an empty string (which would be
/// identical on every such machine).
fn combine(candidates: &[&str]) -> Option<String> {
    for combination in candidates {
        if combination.len() >= MIN_ENTROPY {
            return Some(sha256_hex(combination));
        }
    }
    None
}

/// Generates a random identifier of last resort.
///
/// Used only when the OS gave us nothing distinctive. It is persisted by the
/// caller, so it is stable for the install — but it will not survive a
/// reinstall, and a device in this state needs an operator unbind if it ever
/// loses its stored state.
fn random_fingerprint() -> String {
    let mut buf = [0_u8; 32];
    if getrandom::fill(&mut buf).is_err() {
        // Cryptographic randomness unavailable. This is a sandbox-grade
        // situation; a time-derived value keeps the app working rather than
        // failing activation outright.
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

fn compute() -> String {
    match platform_sources() {
        Some(fingerprint) => fingerprint,
        None => random_fingerprint(),
    }
}

/// Rejects a fingerprint that is too short to be one.
///
/// A real fingerprint is a 64-character SHA-256 hex digest. Anything shorter is
/// a truncated value or a bug — and sending it would bind the code to a device
/// identity that can never be reproduced.
#[must_use]
pub const fn is_valid(fingerprint: &str) -> bool {
    fingerprint.len() >= 16
}

/// A truncated fingerprint for logs and diagnostics.
///
/// The fingerprint is not a secret, but it is a stable device identifier and
/// there is no reason to put the whole thing in a support report or a log line.
#[must_use]
pub fn redact(fingerprint: &str) -> String {
    fingerprint.chars().take(12).collect()
}

#[cfg(windows)]
fn platform_sources() -> Option<String> {
    use std::process::Command;

    /// Runs a command with its console window hidden.
    ///
    /// Without this every launch flashes a PowerShell window on screen — the
    /// exact symptom the old client was fixed for, and one that makes a VPN
    /// look like malware to a student.
    fn run_hidden(program: &str, args: &[&str]) -> Option<String> {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let output = Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if text.is_empty() { None } else { Some(text) }
    }

    let mac = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Get-NetAdapter | Where-Object {$_.PhysicalMediaType -ne 0 -and $_.Status -eq 'Up'} \
             | Select-Object -First 1 -ExpandProperty MacAddress",
        ],
    );

    let disk = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Get-WmiObject Win32_DiskDrive | Select-Object -First 1 -ExpandProperty SerialNumber",
        ],
    );

    let board = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Get-WmiObject Win32_ComputerSystemProduct | Select-Object -ExpandProperty UUID",
        ],
    );

    let hostname = std::env::var("COMPUTERNAME").ok();

    let mac = mac.unwrap_or_default();
    let disk = disk.unwrap_or_default();
    let board = board.unwrap_or_default();
    let host = hostname.unwrap_or_default();

    combine(&[
        &format!("{mac}{disk}{board}"),
        &format!("{mac}{board}"),
        &format!("{mac}{host}"),
    ])
}

#[cfg(target_os = "linux")]
fn platform_sources() -> Option<String> {
    use std::fs;

    /// Reads the first line of a file, if it is readable and non-empty.
    fn read_first(path: &str) -> Option<String> {
        let text = fs::read_to_string(path).ok()?;
        let first = text.lines().next()?.trim().to_owned();
        if first.is_empty() { None } else { Some(first) }
    }

    // machine-id is the right primary source on Linux: it is stable across
    // reboots and package updates, and unlike a MAC address it does not change
    // when the student switches between WiFi and ethernet.
    let machine_id = read_first("/etc/machine-id")
        .or_else(|| read_first("/var/lib/dbus/machine-id"))
        .unwrap_or_default();

    // The first non-loopback, non-virtual interface's MAC.
    let mac = fs::read_dir("/sys/class/net")
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    // Skip interfaces whose address is not a real NIC.
                    if name == "lo" || name.starts_with("veth") || name.starts_with("docker") {
                        return None;
                    }
                    fs::read_to_string(entry.path().join("address"))
                        .ok()
                        .map(|address| address.trim().to_owned())
                })
                .find(|address| !address.is_empty() && address != "00:00:00:00:00:00")
        })
        .unwrap_or_default();

    let hostname = fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_owned())
        .unwrap_or_default();

    combine(&[
        &format!("{machine_id}{mac}"),
        &format!("{machine_id}{hostname}"),
        &machine_id,
    ])
}

#[cfg(target_os = "macos")]
fn platform_sources() -> Option<String> {
    use std::process::Command;

    /// The platform UUID and the primary interface's MAC.
    ///
    /// `ioreg` is used rather than shelling out to `system_profiler`, which is
    /// an order of magnitude slower and would delay first paint.
    fn run(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if text.is_empty() { None } else { Some(text) }
    }

    let hw_uuid = run(
        "sh",
        &[
            "-c",
            "ioreg -rd1 -c IOPlatformExpertDevice | awk -F\\\" '/IOPlatformUUID/ {print $4}'",
        ],
    );

    let mac = run("sh", &["-c", "ifconfig en0 2>/dev/null | awk '/ether/ {print $2}'"]);

    let hw_uuid = hw_uuid.unwrap_or_default();
    let mac = mac.unwrap_or_default();

    combine(&[&format!("{hw_uuid}{mac}"), &hw_uuid])
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn platform_sources() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fingerprint must not change between calls. A per-call value would
    /// bind a device at activation and then fail its own heartbeat.
    #[test]
    fn fingerprint_is_stable_within_a_process() {
        let first = fingerprint();
        let second = fingerprint();
        assert_eq!(first, second, "the fingerprint must be cached, not recomputed");
    }

    /// A real fingerprint is a SHA-256 hex digest, which is what the hub stores
    /// and what the 64-character values in the hub's database look like.
    #[test]
    fn fingerprint_is_a_sha256_hex_digest() {
        let value = fingerprint();
        assert_eq!(
            value.len(),
            64,
            "expected a 64-char SHA-256 hex digest, got {value:?} ({})",
            value.len()
        );
        assert!(
            value.chars().all(|c| c.is_ascii_hexdigit()),
            "a fingerprint must be lowercase hex, got {value:?}"
        );
    }

    /// This is the check the hub performs, so the client must apply the same
    /// one locally rather than sending something the hub will reject.
    #[test]
    fn validity_matches_what_the_hub_accepts() {
        assert!(is_valid(&fingerprint()));
        assert!(!is_valid(""), "an empty fingerprint must be rejected");
        assert!(!is_valid("short"), "a truncated fingerprint must be rejected");
    }

    /// Two machines must not collide on the same fingerprint — the fallback for
    /// "no hardware identifiers" must not be a constant.
    #[test]
    fn the_random_fallback_is_not_constant() {
        let a = random_fingerprint();
        let b = random_fingerprint();
        assert_ne!(a, b, "the fallback must be unique per call, not a fixed value");
        assert_eq!(a.len(), 64);
    }

    /// Diagnostics and log lines carry a prefix, never the whole identifier.
    #[test]
    fn redaction_keeps_only_a_prefix() {
        let value = fingerprint();
        let redacted = redact(&value);
        assert_eq!(redacted.len(), 12);
        assert!(value.starts_with(&redacted));
    }
}
