//! The Locus update path.
//!
//! Replaces the retired client's self-replacing binary and the upstream Tauri
//! updater's static manifest with a **hub-mediated, entitlement-aware,
//! per-platform** system that already exists on the server side.
//!
//! # The four pieces
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`version`] | Is the advertised version actionable? Never string comparison. |
//! | [`signal`] | Decode what the hub advertised, for *this* platform. |
//! | [`apply`] | Download, verify SHA-256 (fail closed), stage privately. |
//! | [`install`] | Hand the verified bytes to the platform installer. |
//!
//! # What is deliberately not here
//!
//! - **A second config/apply path.** Config handling stays in
//!   `core::manager::config`.
//! - **Binary swapping.** The installer owns installation. The old client's
//!   hand-rolled swap destroyed an installation (FIXES #22), and an installed
//!   application has no need of it.
//! - **Rollout arithmetic.** The hub gates which clients *see* an update. The
//!   client does not compute the rollout; it either receives an offer or it does
//!   not. Duplicating that logic would mean two sources of truth for who gets
//!   what.

pub mod apply;
pub mod install;
pub mod signal;
pub mod version;

pub use apply::{DownloadedUpdate, UpdatePhase};
pub use signal::{Platform, SignalRejection, UpdateOffer, decode, decode_public_manifest};
pub use version::{compare, is_newer, rejection_reason};

/// Why an advertised update did **not** become a prompt.
///
/// # Why this is a type and not three `debug!` lines
///
/// It used to be three `logging!(debug, …)` calls. The default log level is
/// `Info`, so on a normal install every one of these was **invisible** — the
/// offer was silently dropped and nothing anywhere said why. A macOS student on
/// 3.2.24 reported exactly that ("the update wasn't offered") and there was no
/// evidence to diagnose it with: not in the log, not on screen.
///
/// An update that is advertised and then ignored is indistinguishable from a
/// stable release with no update, which is the failure mode this whole path
/// exists to avoid. So the reason is a value the caller must handle, not a log
/// line it may never emit — and it is surfaced on the Account page rather than
/// only in a log a student will never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoOfferReason {
    /// The hub advertised nothing. **Normal** — no release is published, or the
    /// active row is off. Not a fault, and must not be shown as one.
    NotAdvertised,
    /// Automatic checking is off in settings, so the offer was deliberately not
    /// recorded.
    ///
    /// Carries its own variant because it is the ONE reason a student can fix
    /// themselves, and the reason most likely to be stuck on: the field is
    /// inherited from upstream Clash Verge Rev, where the Account-page row was
    /// once mis-wired to auto-launch, so a stale `false` can persist across
    /// upgrades with nothing on screen saying so.
    AutomaticChecksOff { version: String },
    /// Not newer than what is running — a stale or rolled-back `update_config`.
    NotNewer { reason: String },
    /// The signal named a version but nothing to download for this build.
    NoArtifactForPlatform { version: String, platform: &'static str },
    /// This build target is not one we publish updates for.
    UnsupportedPlatform,
}

impl NoOfferReason {
    /// Whether this reason is a fault worth telling a student about.
    ///
    /// `NotAdvertised` is the common, healthy case — showing anything for it
    /// would put a permanent "no updates" notice on every up-to-date install.
    /// Everything else is an actionable state.
    #[must_use]
    pub const fn is_actionable(&self) -> bool {
        !matches!(self, Self::NotAdvertised)
    }

    /// A one-line explanation for a log.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::NotAdvertised => "the hub is advertising no update".to_owned(),
            Self::AutomaticChecksOff { version } => format!(
                "{version} is available, but automatic update checking is turned off in Settings"
            ),
            Self::NotNewer { reason } => reason.clone(),
            Self::NoArtifactForPlatform { version, platform } => format!(
                "{version} is advertised but has no build for {platform} — the release is incomplete"
            ),
            Self::UnsupportedPlatform => {
                "this build target does not receive updates".to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NoOfferReason;

    /// Only a real obstacle is actionable.
    ///
    /// `NotAdvertised` is the healthy, common case — an up-to-date install, or a
    /// period with no release published. Reporting it would put a permanent
    /// notice on every working device, which is how a student learns to ignore
    /// the one that matters.
    #[test]
    fn only_real_obstacles_are_actionable() {
        assert!(!NoOfferReason::NotAdvertised.is_actionable());
        assert!(
            NoOfferReason::AutomaticChecksOff {
                version: "3.2.26".into()
            }
            .is_actionable(),
            "a stuck setting is the case a student can fix, so it must surface"
        );
        assert!(
            NoOfferReason::NoArtifactForPlatform {
                version: "3.2.26".into(),
                platform: "macos_arm",
            }
            .is_actionable()
        );
    }

    /// Every reason must produce a sentence a human can act on or forward.
    ///
    /// The whole point of the type is that nothing is silently dropped, so an
    /// empty or contentless description is a regression: it would make the
    /// Account-page notice say nothing.
    #[test]
    fn every_reason_explains_itself() {
        let reasons = [
            NoOfferReason::NotAdvertised,
            NoOfferReason::AutomaticChecksOff {
                version: "3.2.26".into(),
            },
            NoOfferReason::NotNewer {
                reason: "the hub advertised 3.2.20, which is not newer than the running 3.2.26".into(),
            },
            NoOfferReason::NoArtifactForPlatform {
                version: "3.2.26".into(),
                platform: "macos_arm",
            },
            NoOfferReason::UnsupportedPlatform,
        ];
        for reason in reasons {
            let text = reason.describe();
            assert!(!text.is_empty(), "{reason:?} produced no description");
            assert!(
                text.len() > 15,
                "{reason:?} produced a useless description: {text:?}"
            );
        }
    }

    /// The setting-related reason must name the setting, not the symptom.
    ///
    /// This is the case that stranded a macOS student: "no update offered" with
    /// no pointer at the cause. The sentence has to say what to change.
    #[test]
    fn the_setting_reason_points_at_the_setting() {
        let text = NoOfferReason::AutomaticChecksOff {
            version: "3.2.26".into(),
        }
        .describe();
        assert!(
            text.to_lowercase().contains("automatic"),
            "must name the setting so a student can find it: {text:?}"
        );
        assert!(
            text.contains("3.2.26"),
            "must name the version it is withholding: {text:?}"
        );
    }
}
