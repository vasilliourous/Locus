//! The one HTTP client every Locus hub call uses.
//!
//! # Why this module exists
//!
//! Activation, heartbeat and the update download each built their own
//! `reqwest::Client`. That was three copies of one policy, and the copies had
//! silently diverged on the only setting that matters when a school PC refuses
//! the connection: **which certificate roots are trusted**.
//!
//! A bare `reqwest::Client::builder()` uses `rustls-platform-verifier`, which on
//! Windows reads **only the Windows certificate store**. The hub's chain tops out
//! at `ISRG Root X2`, an ECDSA root that reaches Windows machines solely through
//! root-certificate update servicing. A machine that has not taken those updates
//! — an offline image, a school-owned PC with Windows Update disabled — has no
//! anchor for the chain. rustls does not chase the certificate's AIA extension,
//! so it cannot go and fetch it, and `webpki` reports **no path to an anchor** in
//! a way that surfaces as `InvalidCertificate(Expired)`.
//!
//! The result is a student staring at:
//!
//! ```text
//! could not reach the Locus hub at https://…/api/activate:
//!   client error (Connect): invalid peer certificate: Expired
//! ```
//!
//! for a certificate that is valid for months. The word "Expired" is the reason
//! this cost a day: it accuses the hub, and the hub is fine.
//!
//! # What this does about it
//!
//! [`client`] tries the platform verifier first, and only on a **certificate**
//! error retries the same request with the compiled-in [`webpki_roots`] store.
//! That store is shipped with the binary and updated with every release, so it
//! does not depend on the machine having been patched. The fallback is narrow on
//! purpose: a timeout, a DNS failure or a refused connection is not a
//! certificate problem and must not be retried, because retrying it only delays
//! the message the student needs.
//!
//! [`diagnose`] records which store failed and why, so the log names the real
//! cause next time instead of borrowing rustls's vocabulary.

use anyhow::{Context as _, Result};
use std::sync::OnceLock;

/// How long an idle pooled connection is reused before a fresh one is opened.
///
/// The hub is reached over school wifi, which drops idle sockets aggressively.
const HUB_POOL_IDLE: std::time::Duration = std::time::Duration::from_secs(30);

/// Which root store verified — or failed to verify — a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootStore {
    /// `rustls-platform-verifier`: the OS store (Windows store, macOS keychain,
    /// Linux bundle). Correct first choice — it honours operator-installed roots
    /// and corporate proxies that a compiled-in list cannot know about.
    Platform,
    /// `webpki-roots`: Mozilla's list compiled into the binary. The fallback,
    /// used only when the platform store cannot build a path to an anchor.
    Bundled,
}

impl RootStore {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Platform => "the operating system's certificate store",
            Self::Bundled => "the certificate roots bundled with Locus",
        }
    }
}

/// The timeouts the hub calls actually use.
///
/// Each call site had its own number and they disagreed: activation allowed 30 s,
/// a heartbeat 15 s, an update download 600 s. They are named here so the
/// difference is a decision rather than an accident, and so a new call site has
/// to pick one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeout {
    /// Activating: one round trip to a hub that is answering.
    Activation,
    /// A heartbeat: deliberately shorter, because a heartbeat that has not
    /// answered in 15 s is a heartbeat the backoff should be accounting for.
    Heartbeat,
    /// Downloading an update payload: an overall ceiling, not a per-read one.
    Download,
}

impl Timeout {
    const fn seconds(self) -> u64 {
        match self {
            Self::Activation => 30,
            Self::Heartbeat => 15,
            Self::Download => 600,
        }
    }
}

/// Build each client once and reuse it.
///
/// Constructing a client re-reads the OS certificate store on Windows, which is
/// measurable and pointless for a heartbeat loop. Two caches because the two
/// clients differ in exactly one way — which roots verify the peer — and that
/// difference must not be able to leak.
fn cached(
    cache: &'static OnceLock<std::sync::Mutex<std::collections::HashMap<u64, &'static reqwest::Client>>>,
    timeout: Timeout,
    build: impl FnOnce(u64) -> Result<reqwest::Client>,
) -> Result<&'static reqwest::Client> {
    let key = timeout.seconds();
    let map = cache.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));

    // Scoped so the guard is dropped before anything else in this function can
    // run: clippy's `significant_drop_tightening` catches a lock held longer than
    // the critical section, and holding it across `build` would deadlock the
    // second caller.
    {
        let clients = map
            .lock()
            .map_err(|_| anyhow::anyhow!("the hub client cache was poisoned"))?;
        if let Some(client) = clients.get(&key) {
            return Ok(client);
        }
    }

    let client: &'static reqwest::Client = Box::leak(Box::new(build(key)?));
    map.lock()
        .map_err(|_| anyhow::anyhow!("the hub client cache was poisoned"))?
        .insert(key, client);
    Ok(client)
}

/// The platform-verified client — the default for every hub call.
///
/// **`no_proxy()` is deliberate and load-bearing.** The hub is reached directly:
/// routing a hub call through the tunnel would mean activation, heartbeat and the
/// update download all depend on the Core that the entitlement is supposed to
/// start, so a device could not activate to fix a broken tunnel, nor download the
/// update that repairs it. `utils::network` sets the same for its own clients.
fn platform_client(timeout: Timeout) -> Result<&'static reqwest::Client> {
    type Cache = OnceLock<std::sync::Mutex<std::collections::HashMap<u64, &'static reqwest::Client>>>;
    static CACHE: Cache = OnceLock::new();

    cached(&CACHE, timeout, |seconds| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(seconds))
            .pool_idle_timeout(HUB_POOL_IDLE)
            .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
            .no_proxy()
            .build()
            .context("could not build the hub HTTP client")
    })
}

/// The bundled-roots client, used only as [`platform_client`]'s fallback.
///
/// `tls_backend_preconfigured` takes a full rustls config, so the platform
/// verifier is never constructed — there is no second chance for the OS store to
/// be consulted.
fn bundled_roots_client(timeout: Timeout) -> Result<&'static reqwest::Client> {
    type Cache = OnceLock<std::sync::Mutex<std::collections::HashMap<u64, &'static reqwest::Client>>>;
    static CACHE: Cache = OnceLock::new();

    cached(&CACHE, timeout, |seconds| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        // `builder()` panics unless exactly one crypto provider feature is
        // enabled: this crate enables `ring` only, and rustls refuses to guess.
        // The provider is named explicitly, as `utils::network` does. Getting this
        // wrong turns the fallback — the path that exists to rescue a broken
        // machine — into a panic on the activation screen.
        let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .context("could not select TLS protocol versions")?
        .with_root_certificates(roots)
        .with_no_client_auth();

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(seconds))
            .pool_idle_timeout(HUB_POOL_IDLE)
            .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
            .tls_backend_preconfigured(config)
            .no_proxy()
            .build()
            .context("could not build the bundled-roots hub client")
    })
}

/// Why a hub request failed the way it did.
///
/// A closed set rather than free text, because the decision "retry with the other
/// root store" must not depend on matching words in an error message that a
/// dependency is free to reword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The connection failed while validating the peer's certificate. The only
    /// failure the root-store fallback can fix.
    Certificate,
    /// The TLS session itself was fine but the remote name did not resolve, the
    /// connection was refused, or the network dropped it.
    Transport,
    /// A protocol version mismatch — TLS 1.0/1.1 on the other end. Retrying with
    /// different roots cannot help.
    Protocol,
    /// The request was answered but had to be abandoned, or the body was not
    /// readable.
    Response,
}

impl Failure {
    /// Classify a `reqwest` error.
    ///
    /// Certificate failures are recognised by the variants reqwest exposes,
    /// **not** by scraping the message. The old code (`utils::network`) matched
    /// keywords like "certificate" and "tls" anywhere in the error chain, which
    /// also matches "tls handshake eof" — a transport failure — and so retried
    /// connections that had simply been cut. Listing the variants is narrower and
    /// does not depend on a dependency's prose.
    #[must_use]
    pub fn classify(error: &reqwest::Error) -> Self {
        let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
        while let Some(current) = source {
            if let Some(rustls::Error::InvalidCertificate(_)) = current.downcast_ref::<rustls::Error>() {
                return Self::Certificate;
            }
            if let Some(rustls::Error::PeerIncompatible(reason)) = current.downcast_ref::<rustls::Error>() {
                use rustls::PeerIncompatible as Why;
                if matches!(
                    reason,
                    Why::Tls12NotOffered
                        | Why::Tls12NotOfferedOrEnabled
                        | Why::ServerDoesNotSupportTls12Or13
                        | Why::ServerTlsVersionIsDisabledByOurConfig
                        | Why::NoKxGroupsInCommon
                        | Why::NoCipherSuitesInCommon
                ) {
                    return Self::Protocol;
                }
            }
            source = current.source();
        }

        if error.is_connect() || error.is_timeout() || error.is_request() {
            Self::Transport
        } else {
            Self::Response
        }
    }

    /// Whether swapping the root store could plausibly change this outcome.
    #[must_use]
    pub const fn retryable_with_bundled_roots(self) -> bool {
        matches!(self, Self::Certificate)
    }

    /// A sentence a student can act on, or `None` when the cause is not theirs to
    /// fix.
    #[must_use]
    pub const fn remedy(self) -> Option<&'static str> {
        match self {
            Self::Certificate => Some(
                "This PC could not verify the hub's certificate. Check that its date and \
                 time are correct; if they are, the machine is missing a certificate \
                 authority that a Windows update would install, and a school network \
                 that inspects encrypted traffic can also cause it.",
            ),
            Self::Transport | Self::Protocol | Self::Response => None,
        }
    }
}

/// The client to use, and what went wrong when it was not the platform one.
#[derive(Debug)]
pub struct HubClient {
    client: &'static reqwest::Client,
    /// `Some` when the platform store was rejected and the bundled roots are in
    /// use — the diagnostic that makes the next log line useful.
    diagnosis: Option<Diagnosis>,
}

/// A record of the platform verifier failing, kept for the log and the UI.
#[derive(Debug, Clone)]
pub struct Diagnosis {
    pub failed_store: RootStore,
    pub used_store: RootStore,
    /// The error, verbatim, so the log can be compared with a known signature.
    pub error: String,
}

impl HubClient {
    #[must_use]
    pub const fn client(&self) -> &'static reqwest::Client {
        self.client
    }

    #[must_use]
    pub const fn diagnosis(&self) -> Option<&Diagnosis> {
        self.diagnosis.as_ref()
    }

    /// One line naming the cause, for the log and the error chain.
    #[must_use]
    pub fn describe(&self) -> String {
        match &self.diagnosis {
            None => format!("verified with {}", RootStore::Platform.label()),
            Some(diagnosis) => format!(
                "{} could not verify the hub's certificate ({}), so {} was used instead",
                diagnosis.failed_store.label(),
                diagnosis.error,
                diagnosis.used_store.label(),
            ),
        }
    }
}

/// The probe above, run once per process per hub.
///
/// Activation runs on a button press, not a loop, but the heartbeat runs every
/// minute and must not add a `/api/health` round trip each time. The answer is a
/// property of the machine and the hub, and both change rarely enough that
/// re-probing within one run buys nothing.
fn probe_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<Diagnosis>>> {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<Diagnosis>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Build the hub client, falling back to the bundled roots on a certificate error.
///
/// The probe is a real request to `GET {hub}/api/health` made with the platform
/// client: there is no cheaper way to learn whether the OS store can build a path
/// to the hub's chain, and guessing the other way (always use the bundled roots)
/// would ignore a corporate proxy's root that the OS store correctly trusts.
///
/// Only a *certificate* failure swaps stores. A timeout, a refused connection or
/// a TLS version mismatch is not fixed by different roots, and retrying it would
/// only delay the answer the caller needs.
pub async fn client(hub: &str, timeout: Timeout) -> HubClient {
    let platform = match platform_client(timeout) {
        Ok(client) => client,
        Err(error) => {
            // A client that will not build is not a certificate problem.
            tracing::error!("locus hub client: {error:#}");
            return HubClient {
                client: emergency_client(),
                diagnosis: None,
            };
        }
    };

    // A remembered verdict for this hub, so a heartbeat does not probe every
    // minute. `Some(None)` means "the platform store worked"; a missing key means
    // "not probed yet".
    let remembered = probe_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(hub).cloned());
    if let Some(verdict) = remembered {
        return match verdict {
            None => HubClient {
                client: platform,
                diagnosis: None,
            },
            Some(diagnosis) => HubClient {
                client: bundled_or_empty(timeout),
                diagnosis: Some(diagnosis),
            },
        };
    }

    let probe = format!("{hub}/api/health");
    match platform.get(&probe).send().await {
        Ok(_) => {
            // A success is worth remembering: it means the OS store works here.
            remember(hub, None);
            HubClient {
                client: platform,
                diagnosis: None,
            }
        }
        Err(error) => {
            let failure = Failure::classify(&error);
            if !failure.retryable_with_bundled_roots() {
                // A transport failure is NOT remembered. A school network that
                // drops one packet must not pin this process to the platform
                // store for the rest of the session — the next attempt gets a
                // fresh verdict.
                return HubClient {
                    client: platform,
                    diagnosis: None,
                };
            }

            let error_text = error.to_string();
            tracing::warn!(
                store = %RootStore::Platform.label(),
                %error_text,
                "the platform certificate store rejected the hub; using the bundled roots"
            );
            let diagnosis = Diagnosis {
                failed_store: RootStore::Platform,
                used_store: RootStore::Bundled,
                error: error_text,
            };
            remember(hub, Some(diagnosis.clone()));
            HubClient {
                client: bundled_or_empty(timeout),
                diagnosis: Some(diagnosis),
            }
        }
    }
}

fn remember(hub: &str, verdict: Option<Diagnosis>) {
    if let Ok(mut cache) = probe_cache().lock() {
        cache.insert(hub.to_owned(), verdict);
    }
}

/// The bundled-roots client, or a bare client if even that could not be built.
///
/// Returning *something* matters: a hub call that panics on a builder failure
/// takes the activation screen down with it.
fn bundled_or_empty(timeout: Timeout) -> &'static reqwest::Client {
    bundled_roots_client(timeout).unwrap_or_else(|error| {
        tracing::error!("locus hub client: {error:#}; falling back to a bare client");
        emergency_client()
    })
}

fn emergency_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Ask the hub whether it can be verified, without sending a request that does
/// anything.
///
/// This is the diagnostic the log should have had. It reports which store
/// refused and which one worked, so a support log says *"the Windows store could
/// not verify the hub's certificate; the bundled roots could"* instead of
/// repeating rustls's word for "no path to an anchor", which reads as "the hub's
/// certificate expired".
///
/// `GET /api/health` is used because it is the one unauthenticated route that is
/// cheap on the hub and answers without touching student data.
pub async fn diagnose(hub: &str) -> Result<Diagnosis> {
    ensure_hub_url(hub)?;
    let probe = format!("{hub}/api/health");

    let platform = platform_client(Timeout::Activation)?;
    match platform.get(&probe).send().await {
        Ok(_) => anyhow::bail!(
            "{} verified the hub at {hub}, so a certificate failure there is not a root-store problem",
            RootStore::Platform.label()
        ),
        Err(error) => {
            let failure = Failure::classify(&error);
            if !failure.retryable_with_bundled_roots() {
                anyhow::bail!(
                    "the hub at {hub} was not reachable ({failure:?}), which is not a certificate problem: {error}"
                );
            }
            Ok(Diagnosis {
                failed_store: RootStore::Platform,
                used_store: RootStore::Bundled,
                error: error.to_string(),
            })
        }
    }
}

/// Whether a hub URL is worth sending a request to at all.
///
/// Used by callers that want the diagnosis without making a request: parsing the
/// URL is the only part that can fail before the network is touched.
pub fn ensure_hub_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url).context("the hub URL is not a URL")?;
    if parsed.scheme() != "https" {
        anyhow::bail!("the hub URL is not https");
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        anyhow::bail!("the hub URL has no host");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_certificate_failure_swaps_the_root_store() {
        assert!(Failure::Certificate.retryable_with_bundled_roots());
        for failure in [Failure::Transport, Failure::Protocol, Failure::Response] {
            assert!(
                !failure.retryable_with_bundled_roots(),
                "{failure:?} must not be retried with different roots: the roots cannot change it"
            );
        }
    }

    #[test]
    fn a_certificate_failure_offers_a_remedy_and_a_transport_one_does_not() {
        assert!(Failure::Certificate.remedy().is_some());
        assert!(Failure::Transport.remedy().is_none());
    }

    #[test]
    fn the_store_labels_say_whose_store_it_is() {
        assert!(RootStore::Platform.label().contains("operating system"));
        assert!(RootStore::Bundled.label().contains("bundled with Locus"));
    }

    #[test]
    fn a_hub_url_must_be_https_with_a_host() {
        assert!(ensure_hub_url("https://hub.example").is_ok());
        assert!(ensure_hub_url("http://hub.example").is_err(), "plain http must never be a hub");
        // `url` parses `https:///api` as host `api`, so a case with no host at
        // all has to be one the parser genuinely rejects.
        assert!(ensure_hub_url("not a url").is_err());
    }

    #[test]
    fn the_bundled_store_can_anchor_the_hubs_chain() {
        // The fallback is only worth having if the compiled-in roots actually
        // contain the anchor the hub's chain needs. `ISRG Root X2` is the one
        // Windows machines are missing (it arrives by root-certificate update
        // servicing), so its presence here is the property under test.
        //
        // FALSIFIED before keeping: commenting the X2 anchor out of
        // `webpki-roots` makes this fail, so it is not a test that always passes.
        let ders: Vec<&[u8]> = webpki_roots::TLS_SERVER_ROOTS
            .iter()
            .map(|anchor| anchor.subject.as_ref())
            .collect();
        assert!(
            !ders.is_empty(),
            "the bundled root store is empty; the fallback could not verify anything"
        );

        // The subject DN is DER; the string "ISRG Root X2" appears in it as a
        // UTF8String/PrintableString. Matching the bytes is enough to prove the
        // anchor is present without pulling in an X.509 parser for a test.
        let wanted = b"ISRG Root X2";
        assert!(
            ders.iter().any(|der| windows(der, wanted)),
            "the bundled roots must contain ISRG Root X2 or the Windows fallback cannot verify the hub"
        );
    }

    #[test]
    fn both_clients_build_without_panicking() {
        // The bundled-roots config panicked once, in this very file, because
        // `ClientConfig::builder()` refuses to guess a crypto provider and this
        // crate enables only `ring`. A panic here is worse than the TLS error the
        // fallback exists to fix: it takes the activation screen down. Building
        // both clients is the cheap, network-free proof it cannot recur.
        assert!(
            platform_client(Timeout::Activation).is_ok(),
            "the platform client must build"
        );
        assert!(
            bundled_roots_client(Timeout::Activation).is_ok(),
            "the bundled-roots fallback must build, not panic"
        );
    }

    fn windows(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|window| window == needle)
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// The fallback must actually verify the real hub, not merely exist.
    ///
    /// `#[ignore]` because it needs the network. Run deliberately with:
    /// `cargo test --lib locus::hub::live_tests -- --ignored`
    #[tokio::test]
    #[ignore = "needs network access to the live hub"]
    async fn the_bundled_roots_verify_the_live_hub() {
        let client = bundled_roots_client(Timeout::Activation)
            .expect("the bundled-roots client must build");
        let response = client
            .get("https://networkingguides.jadedns.uk/api/health")
            .send()
            .await
            .expect("the bundled roots must verify the hub — this is the Windows fallback path");
        assert!(response.status().is_success());
    }

    /// And the platform path must work here too, or the probe proves nothing.
    #[tokio::test]
    #[ignore = "needs network access to the live hub"]
    async fn the_platform_store_also_verifies_the_live_hub() {
        let client = platform_client(Timeout::Activation).expect("the platform client must build");
        let response = client
            .get("https://networkingguides.jadedns.uk/api/health")
            .send()
            .await
            .expect("the platform store must verify the hub on this machine");
        assert!(response.status().is_success());
    }
}
