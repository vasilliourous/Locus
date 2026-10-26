//! The hub's TLS root stores, and the one rule for choosing between them.
//!
//! # Why this is a module of its own
//!
//! The *decision* ("try the OS store; on a certificate failure only, retry with
//! the roots compiled into the binary") is tested here with no network, no OS and
//! no app handle. What remains in [`super`] is the part that needs a live hub.
//!
//! The concrete failure this encodes is the Windows one in `FIXES.md`
//! (2026-10-18): `rustls-platform-verifier` reads only the Windows certificate
//! store, the hub's chain tops out at `ISRG Root X2`, and a machine that has not
//! taken root-certificate updates reports the gap as
//! `InvalidCertificate(Expired)` — which accuses a certificate that is valid for
//! months.
//!
//! # Why Apple platforms get the bundled roots FIRST
//!
//! On macOS the platform store is the keychain, and the same shape of gap exists
//! there: our chain has `YE1` (Let's Encrypt) under `ISRG Root X2`, and an older
//! macOS keychain may not carry that ECDSA root at all. `webpki` does not chase
//! the AIA extension, so there is no path to an anchor and the OS reports it with
//! the same misleading word.
//!
//! The hub has exactly one certificate chain, and it is issued by a public CA whose
//! roots ship in `webpki-roots`. So for a **public hub whose chain we can name**
//! there is nothing the keychain knows that the bundled list does not, and the
//! bundled list has the advantage of being current: it is updated with every
//! release instead of whenever the machine was last patched. On Apple platforms
//! the bundled roots are therefore tried first and the keychain is the fallback.
//!
//! The distinction matters because a *corporate* TLS-inspecting proxy installs its
//! root into the OS store only — which is why the OS store stays the first choice
//! everywhere it can actually be older than our binary, and why it remains the
//! fallback on Apple platforms rather than being removed.

/// Which root store verified — or failed to verify — a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootStore {
    /// `rustls-platform-verifier`: the OS store (Windows store, macOS keychain,
    /// Linux bundle). Honours operator-installed roots and corporate proxies that
    /// a compiled-in list cannot know about.
    Platform,
    /// `webpki-roots`: Mozilla's list compiled into the binary.
    Bundled,
}

impl RootStore {
    /// The first store to try on this platform.
    ///
    /// See the module docs: Apple platforms start with the bundled roots because
    /// an old keychain is a measured failure mode here, while Windows and Linux
    /// start with the OS store because a school-owned machine is the likelier
    /// place for an operator-installed root (a TLS-inspecting proxy).
    #[must_use]
    pub const fn primary() -> Self {
        Self::primary_for(cfg!(target_os = "macos"))
    }

    /// The same rule, as a pure function of "is this Apple?".
    ///
    /// Split out because [`Self::primary`] reads `cfg!`, which is evaluated for
    /// the HOST compiling the test. This crate is built and tested on Linux CI,
    /// so a test that only asserted `primary()` behaved would pass on a Linux
    /// runner no matter what the macOS branch said — a guard that cannot fail on
    /// the machine that runs it. (Observed: flipping the branch to `Self::Platform`
    /// unconditionally left every test green.) Taking the platform as an argument
    /// makes the Apple rule assertable everywhere, which is the only way a
    /// platform-specific decision gets tested at all.
    #[must_use]
    pub const fn primary_for(apple: bool) -> Self {
        if apple { Self::Bundled } else { Self::Platform }
    }

    /// The store to try when [`Self::primary`] failed with a certificate error.
    #[must_use]
    pub const fn fallback(self) -> Self {
        match self {
            Self::Platform => Self::Bundled,
            Self::Bundled => Self::Platform,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Platform => "the operating system's certificate store",
            Self::Bundled => "the certificate roots bundled with Locus",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback must be the OTHER store on every platform.
    ///
    /// A `fallback` that returned `self` would make the retry a second identical
    /// attempt: it would fail the same way and the log would claim two stores
    /// were tried.
    #[test]
    fn the_fallback_is_the_other_store() {
        assert_eq!(RootStore::Platform.fallback(), RootStore::Bundled);
        assert_eq!(RootStore::Bundled.fallback(), RootStore::Platform);
        assert_ne!(RootStore::primary(), RootStore::Platform.fallback())
    }

    /// Apple platforms must start with the bundled roots, on EVERY host.
    ///
    /// This is the Intel-Mac defect: an old keychain has no `ISRG Root X2`, and
    /// the OS reports that as an expired certificate. Exercised through
    /// [`RootStore::primary_for`] so it runs on the Linux runner too — the
    /// `cfg!`-reading version of this test was green on a deliberately broken
    /// branch, which is the §3 "a guard that cannot fail" shape.
    #[test]
    fn apple_platforms_try_the_bundled_roots_first() {
        assert_eq!(
            RootStore::primary_for(true),
            RootStore::Bundled,
            "macOS must not lead with the keychain: an old one has no ISRG Root X2, \
             and the failure is reported as an expired certificate"
        );
        assert_eq!(
            RootStore::primary_for(false),
            RootStore::Platform,
            "Windows and Linux must lead with the OS store, so a corporate proxy root \
             installed there is honoured"
        );
        // And the real value on this host must match the rule, not something else.
        assert_eq!(RootStore::primary(), RootStore::primary_for(cfg!(target_os = "macos")));
    }
}
