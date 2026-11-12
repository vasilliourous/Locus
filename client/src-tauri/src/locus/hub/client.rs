//! The hub clients: the two root stores, the probe that picks between them, and
//! the diagnosis that explains the choice.
//!
//! Read [`super`](crate::locus::hub) first — it holds the reason this module
//! exists. The short version: a bare `reqwest` client trusts only the OS store,
//! which on an unpatched machine has no path to the hub's `ISRG Root X2` anchor
//! and reports that as `InvalidCertificate(Expired)`.

use super::roots::RootStore;
use anyhow::{Context as _, Result};
use std::sync::OnceLock;

/// How long an idle pooled connection is reused before a fresh one is opened.
///
/// The hub is reached over school wifi, which drops idle sockets aggressively.
const HUB_POOL_IDLE: std::time::Duration = std::time::Duration::from_secs(30);

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

    /// The same ceiling, for a caller that must build its OWN client.
    ///
    /// The update download is the one case: it needs redirects disabled, and a
    /// `reqwest::Client` cannot be cloned into a client with different redirect
    /// policy. Exposing the number rather than duplicating it means a change to
    /// the download ceiling reaches both clients.
    #[must_use]
    pub const fn seconds_public(self) -> std::time::Duration {
        std::time::Duration::from_secs(self.seconds())
    }
}

/// Whether a hub client follows redirects.
///
/// A first-class part of the client policy rather than a per-call tweak, because
/// the two answers are a decision about this network, not a style choice:
///
///   * [`Self::Follow`] — the default for API calls. The hub's alias block `308`s
///     to the canonical name, and an API call must keep working when it lands
///     there.
///   * [`Self::Refuse`] — for an **artifact** download. On a network that filters
///     the hub by name, the redirect target is usually the blocked name: following
///     it turns a refusal that can be explained into a timeout that cannot. The
///     download also checks the host it was offered against
///     [`crate::locus::update::signal::OfferHosts`] first, and following a redirect
///     would quietly move the request to a host that check never saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Redirects {
    Follow,
    Refuse,
}

impl Redirects {
    fn policy(self) -> reqwest::redirect::Policy {
        match self {
            Self::Follow => reqwest::redirect::Policy::limited(5),
            Self::Refuse => reqwest::redirect::Policy::none(),
        }
    }
}

/// Build each client once and reuse it.
///
/// Constructing a client re-reads the OS certificate store on Windows, which is
/// measurable and pointless for a heartbeat loop. Cached per (timeout, redirect
/// policy, roots) — every axis on which these clients differ.
fn cached_policy(
    cache: &'static OnceLock<std::sync::Mutex<std::collections::HashMap<(u64, Redirects), &'static reqwest::Client>>>,
    timeout: Timeout,
    redirects: Redirects,
    build: impl FnOnce(u64) -> Result<reqwest::Client>,
) -> Result<&'static reqwest::Client> {
    let key = (timeout.seconds(), redirects);
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

    let client: &'static reqwest::Client = Box::leak(Box::new(build(key.0)?));
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
fn platform_client(timeout: Timeout, redirects: Redirects) -> Result<&'static reqwest::Client> {
    type Cache = OnceLock<std::sync::Mutex<std::collections::HashMap<(u64, Redirects), &'static reqwest::Client>>>;
    static CACHE: Cache = OnceLock::new();
    cached_policy(&CACHE, timeout, redirects, |seconds| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(seconds))
            .pool_idle_timeout(HUB_POOL_IDLE)
            .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
            .redirect(redirects.policy())
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
fn bundled_roots_client(timeout: Timeout, redirects: Redirects) -> Result<&'static reqwest::Client> {
    type Cache = OnceLock<std::sync::Mutex<std::collections::HashMap<(u64, Redirects), &'static reqwest::Client>>>;
    static CACHE: Cache = OnceLock::new();

    cached_policy(&CACHE, timeout, redirects, |seconds| {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        // `builder()` panics unless exactly one crypto provider feature is
        // enabled: this crate enables `ring` only, and rustls refuses to guess.
        // The provider is named explicitly, as `utils::network` does. Getting this
        // wrong turns the fallback — the path that exists to rescue a broken
        // machine — into a panic on the activation screen.
        let config =
            rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .context("could not select TLS protocol versions")?
                .with_root_certificates(roots)
                .with_no_client_auth();

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(seconds))
            .pool_idle_timeout(HUB_POOL_IDLE)
            .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
            .tls_backend_preconfigured(config)
            .redirect(redirects.policy())
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
    /// The peer **accepted the TCP connection and then cut it** — during the TLS
    /// handshake, or after the request had been written.
    ///
    /// This is separate from [`Self::Transport`] for one reason: it is the
    /// signature of a middlebox that read the destination (the TLS ClientHello
    /// carries the SNI in the clear) and killed the flow. One of the two failures
    /// the activation tunnel can fix — see [`Self::retryable_over_tunnel`] for
    /// the closed set.
    ///
    /// "After the request had been written" is part of the definition since
    /// 2026-11-09. The check used to be gated on `error.is_connect()`, which is
    /// true only of a connection that never came up, so an appliance that let
    /// the request out and *then* reset it was classified [`Self::Transport`] and
    /// never reached the tunnel. The kinds that matter — `ConnectionReset`,
    /// `BrokenPipe`, `UnexpectedEof`, `InvalidData` — are listed once, in
    /// `is_reset_kind`.
    ///
    /// Deliberately **not** the same as:
    ///   * a DNS failure — nothing was dialled, so nothing was reset;
    ///   * a certificate error — the hub was reached; different roots fix it.
    ///
    /// Classified from the `io::ErrorKind`, not from the message, for the reason
    /// given on [`Self::classify`].
    ResetDuringHandshake,
    /// The hub's **name did not resolve**.
    ///
    /// Split out of [`Self::Transport`] because it is the one transport failure
    /// with a specific, actionable cause: the network's DNS refused to answer, so
    /// nothing was dialled and nothing was blocked — which is exactly what a
    /// school filter that resolves nothing it has not categorised looks like. It
    /// used to arrive as a bare "error sending request for url (…)" with no remedy,
    /// and a student on a school network was left with a sentence naming no cause.
    ///
    /// Still **not** retried over the activation tunnel, for a reason that is
    /// about the resolver rather than the block: the tunnel's own engine must
    /// resolve `SS_HUB` before it can dial anything, and a resolver that answers
    /// nothing answers nothing for it too. A school filter that returns a
    /// *portal or bogus address* instead does not produce this variant — it
    /// produces a reset or a refusal, and both of those do trigger the tunnel.
    Dns,
    /// The name resolved and the host **refused** the connection — nothing was
    /// listening on that port.
    ///
    /// Distinct from a reset: a refusal means the packet reached a machine, so the
    /// network is not blocking this destination *by packet filter*. And distinct
    /// from DNS: the name is fine.
    ///
    /// # Why this now triggers the activation tunnel
    ///
    /// It used to be excluded, on the reasoning that "a different transport to the
    /// same address will be refused too". **That is only true when the same party
    /// answers on both.** Measured on 2026-11-06: on N4L a request for a blocked
    /// hostname is answered by the filtering appliance — the TLS ClientHello is
    /// proxied and torn down, which the client sees as a reset *or* as a refusal,
    /// depending on how far the appliance got. The school block is a
    /// **name-reputation** decision, so the second dial (the bootstrap port, which
    /// presents no name) is a different question and must be asked.
    ///
    /// The cost of being wrong is one refused SOCKS dial for a hub that is
    /// genuinely down, reported as "the activation tunnel was tried as well and
    /// also failed" — which is exactly what happened. The cost of *not* trying is
    /// a student who cannot enrol at all.
    Refused,
    /// The TLS session itself was fine but the network dropped the connection, the
    /// host was unreachable, or the request timed out.
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
        // A reset is checked FIRST and separately, because it is an `io::Error`
        // buried in the chain that would otherwise be swallowed by the
        // `is_connect()` catch-all below — and because confusing it with a
        // refusal or a DNS failure would either trigger a fallback that cannot
        // help or leave a censored student with no route at all.
        //
        // The reset check is NOT gated on `is_connect()`, and that removed gate
        // is the defect this guards. `is_connect()` is true only for a
        // connection that never came up; an appliance that proxies the
        // ClientHello and then tears the flow down produces a request-phase
        // error instead (`UnexpectedEof`/`InvalidData` in the chain), which
        // classified as `Transport` — not retryable over the tunnel — so the one
        // transport built for a censored network was never attempted on the
        // network it was built for. A reset means a flow was cut, whether it was
        // being opened or already open; whether the connection had completed is
        // not the question.
        if reset_in_chain(error) {
            return Self::ResetDuringHandshake;
        }

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

        // DNS and refusal are separated from the catch-all below, and only after
        // the reset and certificate checks have had their say. Ordering is the
        // whole mechanism: a resolver failure and a drop both arrive as
        // `is_connect()`, so testing for `Transport` first would make these two
        // unreachable — a variant that can never be returned reads exactly like
        // one that is classified correctly.
        if error.is_connect() {
            // Which resolver error the OS chose is not something a student can act
            // on, so only the fact of a naming failure is kept.
            if io_kinds_in_chain(error).any(is_name_resolution) {
                return Self::Dns;
            }
            if io_kinds_in_chain(error).any(|kind| kind == std::io::ErrorKind::ConnectionRefused) {
                return Self::Refused;
            }
        }

        if error.is_connect() || error.is_timeout() || error.is_request() {
            Self::Transport
        } else {
            Self::Response
        }
    }

    /// Whether swapping the root store could plausibly change this outcome.
    ///
    /// Named for the store swap rather than for a direction: which store is tried
    /// first is a platform decision (`RootStore::primary`), and the retry is the
    /// same rule either way.
    #[must_use]
    pub const fn retryable_with_other_roots(self) -> bool {
        matches!(self, Self::Certificate)
    }

    /// Whether a **different transport** could plausibly change this outcome.
    ///
    /// The activation tunnel dials the same hub on a different port speaking a
    /// protocol whose traffic carries no readable destination, so it is a
    /// candidate fix only when the failure was the network refusing to carry a
    /// **named** connection. That is three variants:
    ///
    ///   * `ResetDuringHandshake` — the middlebox read the name and cut the flow,
    ///     before or after the request was written. This is the case the tunnel
    ///     exists for.
    ///   * `Refused` — **in the set since 2026-11-06.** The old reasoning was "a
    ///     machine answered and said no; another port to the same machine would
    ///     be asked differently, but nothing observed suggests it would help". On
    ///     the network this exists for, the machine that answers is the *filter*,
    ///     not the hub: it proxies the ClientHello and tears the flow down, which
    ///     reaches this process as a reset or as a refusal depending on how far it
    ///     got. Both are one decision — the destination's **name** is blocked — and
    ///     the bootstrap port presents no name.
    ///   * `Dns` — **in the set since 2026-11-09.** It used to be excluded, on the
    ///     reasoning that the tunnel's engine dials the hub by name too, so a
    ///     resolver that answers nothing answers nothing for the fallback either.
    ///     That reasoning was correct *about the document being generated*: the
    ///     bootstrap outbound's `server` was the hub hostname. It is no longer the
    ///     whole story — the engine now carries an address candidate
    ///     (`contract::ACTIVATION_TUNNEL_ADDR`, `activation_tunnel::Target`) that
    ///     needs no resolver at all, which is what makes a DNS-filtered network
    ///     reachable for the first time. Excluding `Dns` while shipping an address
    ///     candidate would leave the one failure the candidate exists for
    ///     untried.
    ///
    /// NOT in the set:
    ///
    ///   * `Certificate` — the hub WAS reached; different roots fix it, and the
    ///     tunnel would present the same certificate.
    ///   * `Protocol` — the hub was reached and refused a TLS version.
    ///   * `Transport` — the connection was dropped or timed out with no signature
    ///     of a name-based block. `TimedOut` in particular is indistinguishable
    ///     from a hub that is merely slow, and retrying it on a heartbeat loop
    ///     would spawn a 60 MB engine per beat.
    ///   * `Response` — an answer arrived; the problem is above the transport.
    ///
    /// A hub that is genuinely down now costs one extra refused SOCKS dial,
    /// reported in the same sentence as the original failure. That is the cheaper
    /// of the two mistakes by a wide margin.
    #[must_use]
    pub const fn retryable_over_tunnel(self) -> bool {
        matches!(self, Self::ResetDuringHandshake | Self::Refused | Self::Dns)
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
            Self::ResetDuringHandshake => Some(
                "The network cut the connection to the hub. This usually means a school \
                 or work filter is blocking it. Locus will try the activation tunnel \
                 instead when one is available.",
            ),
            // The sentence a school-network student needed and did not get. It says
            // what actually happened — nothing was dialled, because the name did not
            // resolve — instead of "could not reach the hub", which sounds like the
            // hub is down.
            //
            // It promised nothing about the tunnel until 2026-11-09, and that was
            // honest then: the bootstrap outbound dialled the hub hostname, so the
            // fallback needed the same lookup this failure says is filtering. With
            // an address candidate the sentence can name the retry — and the hotspot
            // advice stays, because the address is a compiled constant and an
            // address can go stale in a way a name does not.
            Self::Dns => Some(
                "This computer could not look up the hub's address, so the connection was \
                 never attempted. That is usually this network filtering DNS — common on \
                 school and work networks. Locus will try the activation tunnel instead \
                 when one is available; if that does not work either, a phone hotspot \
                 will tell you whether this computer is the cause.",
            ),
            Self::Refused => Some(
                "The hub's address was found, but nothing there accepted the connection. \
                 That is either a temporary outage at the hub or this network quietly \
                 redirecting the address — common on school and work networks. Locus \
                 will try the activation tunnel instead when one is available; if that \
                 fails too, try a phone hotspot to tell the two apart.",
            ),
            Self::Transport | Self::Protocol | Self::Response => None,
        }
    }
}

/// Every `io::ErrorKind` in the chain, one pass.
///
/// The chain, not the top error, because hyper wraps the socket error before it
/// reaches `reqwest`: the kind that names the cause sits under the wrapper. The
/// alternative — matching words in the messages — is the thing
/// [`Failure::classify`] exists to avoid.
fn io_kinds_in_chain(error: &reqwest::Error) -> impl Iterator<Item = std::io::ErrorKind> + '_ {
    let mut cursor: Option<&(dyn std::error::Error + 'static)> = Some(error);
    std::iter::from_fn(move || {
        while let Some(current) = cursor {
            cursor = current.source();
            if let Some(io_error) = current.downcast_ref::<std::io::Error>() {
                return Some(io_error.kind());
            }
        }
        None
    })
}

/// Whether this kind means "the name could not be resolved".
///
/// `NotFound` is what `getaddrinfo`'s `EAI_NONAME`/`EAI_NODATA` map to on Linux
/// and macOS (`Name or service not known`, `nodename nor servname provided`) and
/// what Windows' `WSAHOST_NOT_FOUND` maps to. `HostUnreachable` is included
/// because some resolvers report a filtered name that way rather than as an
/// absence — and because on a school network the distinction between "this name
/// does not exist" and "this name is not reachable for you" is one the student
/// cannot see and should not have to.
const fn is_name_resolution(kind: std::io::ErrorKind) -> bool {
    matches!(kind, std::io::ErrorKind::NotFound | std::io::ErrorKind::HostUnreachable)
}

/// Whether an `io::Error` in this chain is a **reset**, as opposed to a refusal,
/// a DNS failure or a timeout.
///
/// `ConnectionReset` is the kind the OS reports for a RST (`ECONNRESET`, os error
/// 104 on Linux). It is matched on the **kind**, not the message, for the reason
/// [`Failure::classify`] gives.
///
/// One thing this deliberately accepts: on the Linux test path the reset surfaces
/// as `io::ErrorKind::Other` with no raw OS error, because hyper wraps it before
/// the kind is preserved. That is why the message is checked as a fallback — but
/// only *after* `is_connect()` has already narrowed the failure to the connection
/// phase, so the false-positive the old `utils::network` code risked (matching
/// "tls" in "tls handshake eof" and retrying a cut connection forever) does not
/// recur here: a reset is a *reason to try another transport once*, never a reason
/// to retry the same one in a loop.
/// Whether this `io::ErrorKind` is the network cutting a flow that was live.
///
/// The set is wider than "connection reset" on purpose, and the extra members
/// are not guesses — they are what a filtering appliance's teardown looks like
/// one layer up, and each was missing from the original two.
///
///   * `ConnectionReset` — the RST itself.
///   * `BrokenPipe`      — the same RST observed while writing.
///   * `UnexpectedEof`   — **the common one.** A TLS connection closed inside
///     the handshake reports a clean EOF at the record layer, and hyper
///     surfaces it as `UnexpectedEof`. This is what an appliance that proxies
///     the ClientHello and then drops the flow produces.
///   * `InvalidData`     — the record layer receiving something that is not a
///     TLS record, which is what an in-line appliance's rewritten bytes look
///     like.
///
/// Deliberately NOT here: `NotFound`/`HostUnreachable` (a naming failure with
/// its own class), `ConnectionRefused` (already retryable as its own class), and
/// `TimedOut` (indistinguishable from a hub that is merely slow; retrying it
/// would spawn an engine per beat on a heartbeat loop).
const fn is_reset_kind(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::InvalidData
    )
}

fn reset_in_chain(error: &reqwest::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = source {
        if let Some(io_error) = current.downcast_ref::<std::io::Error>()
            && is_reset_kind(io_error.kind())
        {
            return true;
        }
        let text = current.to_string();
        if text.contains("Connection reset by peer") || text.contains("connection reset") {
            return true;
        }
        source = current.source();
    }
    false
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
            None => format!("verified with {}", RootStore::primary().label()),
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
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<Diagnosis>>>> = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// The client for `store`, or the emergency client if it cannot be built.
fn client_for(store: RootStore, timeout: Timeout, redirects: Redirects) -> Result<&'static reqwest::Client> {
    match store {
        RootStore::Platform => platform_client(timeout, redirects),
        RootStore::Bundled => bundled_roots_client(timeout, redirects),
    }
}

/// Build the hub client, trying the other root store on a certificate error.
///
/// The probe is a real request to `GET {hub}/api/health` made with the primary
/// store's client: there is no cheaper way to learn whether that store can build
/// a path to the hub's chain, and assuming an answer would either ignore a
/// corporate proxy's root that the OS store correctly trusts (always bundled) or
/// repeat the old-keychain failure on every launch (always platform).
///
/// Which store is primary depends on the platform — see [`super::roots`] and
/// [`RootStore::primary`]. Only a *certificate* failure swaps stores. A timeout,
/// a refused connection or a TLS version mismatch is not fixed by different roots,
/// and retrying it would only delay the answer the caller needs.
pub async fn client(hub: &str, timeout: Timeout) -> HubClient {
    client_with(hub, timeout, Redirects::Follow).await
}

/// [`client`] with an explicit redirect policy, for the artifact download.
///
/// Same store selection, same probe, same cache — only the redirect answer differs
/// (see [`Redirects`]). This exists so the download does not hand-build a client:
/// that is precisely what the consistency checker forbids, because a bare client
/// trusts only the OS store and the download is the call that repairs a machine
/// whose OS store cannot verify the hub.
pub async fn client_refusing_redirects(hub: &str, timeout: Timeout) -> HubClient {
    client_with(hub, timeout, Redirects::Refuse).await
}

/// A client that sends its requests through `proxy`, or the ordinary client when
/// it is `None`.
///
/// # Why this exists
///
/// Every other client here sets `.no_proxy()` ([`platform_client`],
/// [`bundled_roots_client`]), deliberately: the hub is normally reached directly,
/// because routing activation through the tunnel would make activation depend on
/// the Core that activation is supposed to start. On a network that blocks the hub
/// by name that reasoning inverts — direct cannot work at all — so the maintenance
/// tunnel supplies a local SOCKS5 to dial instead.
///
/// # Why it is a sibling and not a parameter on the existing builders
///
/// The root-store selection and the probe cache are what must NOT be duplicated
/// (see the module docs). Proxy support is orthogonal to them: the same selected
/// client is rebuilt with a proxy attached. Keeping it adjacent means the store
/// logic is untouched and the classifier is untouched.
///
/// `None` returns exactly what [`client`] returns, so a caller can pass its
/// routing decision straight through without branching.
pub async fn client_with_proxy(hub: &str, timeout: Timeout, proxy: Option<reqwest::Proxy>) -> HubClient {
    let Some(proxy) = proxy else {
        return client(hub, timeout).await;
    };

    // The store selection is NOT repeated here: it is reused by taking the
    // ordinary client's verdict and rebuilding only the transport. A second
    // probe would be a second policy, which is the divergence this module was
    // built to remove.
    let selected = client(hub, timeout).await;
    match proxied_client(timeout, &proxy) {
        Ok(client) => HubClient {
            client,
            diagnosis: selected.diagnosis().cloned(),
        },
        Err(error) => {
            tracing::error!("locus hub client (proxied): {error:#}");
            selected
        }
    }
}

/// Builds (once per proxy URL) a client that dials through `proxy`.
///
/// # Why it does not set `no_proxy`
///
/// `no_proxy()` clears the proxy list, which would discard the very proxy this
/// function exists to install. Every other builder in this module sets it; this
/// one must not, and that difference is the whole function.
///
/// # Why the cache is keyed on the proxy URL
///
/// The maintenance engine's loopback port changes every time it is restarted
/// (a free port is picked per start, `free_loopback_port`), so a cache keyed only
/// on the timeout would hand back a client pointing at a dead port.
fn proxied_client(timeout: Timeout, proxy: &reqwest::Proxy) -> Result<&'static reqwest::Client> {
    type Cache = OnceLock<std::sync::Mutex<std::collections::HashMap<String, &'static reqwest::Client>>>;
    static CACHE: Cache = OnceLock::new();

    // `Proxy` exposes no accessor for its URL, so the key is the debug form: it
    // contains the proxy URL and nothing that changes per call.
    let key = format!("{}|{:?}", timeout.seconds(), proxy);
    if let Some(client) = CACHE
        .get_or_init(Default::default)
        .lock()
        .ok()
        .and_then(|c| c.get(&key).copied())
    {
        return Ok(client);
    }

    let client: &'static reqwest::Client = Box::leak(Box::new(
        reqwest::Client::builder()
            .timeout(timeout.seconds_public())
            .pool_idle_timeout(HUB_POOL_IDLE)
            .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
            .redirect(Redirects::Follow.policy())
            .proxy(proxy.clone())
            .build()
            .context("could not build the proxied hub HTTP client")?,
    ));
    if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
        cache.insert(key, client);
    }
    Ok(client)
}

async fn client_with(hub: &str, timeout: Timeout, redirects: Redirects) -> HubClient {
    let primary = RootStore::primary();
    let primary_client = match client_for(primary, timeout, redirects) {
        Ok(client) => client,
        Err(error) => {
            // A client that will not build is not a certificate problem.
            tracing::error!("locus hub client ({primary:?}): {error:#}");
            return HubClient {
                client: emergency_client(),
                diagnosis: None,
            };
        }
    };

    // A remembered verdict for this hub, so a heartbeat does not probe every
    // minute. `Some(None)` means "the primary store worked"; a missing key means
    // "not probed yet".
    let remembered = probe_cache().lock().ok().and_then(|cache| cache.get(hub).cloned());
    match remembered {
        Some(None) => HubClient {
            client: primary_client,
            diagnosis: None,
        },
        Some(Some(diagnosis)) => HubClient {
            client: bundled_or_empty(diagnosis.used_store, timeout, redirects),
            diagnosis: Some(diagnosis),
        },
        // Nothing remembered for THIS name. A verdict belongs to the name it was
        // formed against, so a device moved to an alias starts here — which is
        // exactly what must happen on the network this fallback exists for.
        None => probe_and_choose(hub, primary, primary_client, timeout, redirects).await,
    }
}

/// Runs the store probe for a hub nobody has asked about yet in this process.
///
/// Split out of [`client_with`] because the decision tree there was over the
/// complexity budget, and because this half is the part with a rule in it:
///
/// **A probe that fails for a reason other than certificates must not be reported
/// as success.** It used to ignore the response status entirely, so a `404` — a
/// filter answering in the hub's place, or a hub whose route is absent — read as
/// "this store works", and the *next* call then failed with a message about
/// whatever the proxy returned. Only 2xx is success; a redirect or a 5xx from an
/// appliance is an answer from something that is not this hub.
async fn probe_and_choose(
    hub: &str,
    primary: RootStore,
    primary_client: &'static reqwest::Client,
    timeout: Timeout,
    redirects: Redirects,
) -> HubClient {
    let probe = format!("{hub}/api/health");
    let outcome = primary_client.get(&probe).send().await;

    match outcome {
        Ok(response) if response.status().is_success() => {
            // A success is worth remembering: it means this store works here.
            remember(hub, None);
            HubClient {
                client: primary_client,
                diagnosis: None,
            }
        }
        Ok(response) => {
            // Reachable, and answered by something that is not the hub's health
            // route. Reported here rather than left for the caller to discover as
            // an unexplained 4xx.
            tracing::warn!(
                target: "locus::hub",
                status = %response.status(),
                "the hub probe for {hub} was answered by something other than the hub's health \
                 route — a filter or a misrouted request is the usual cause"
            );
            HubClient {
                client: primary_client,
                diagnosis: None,
            }
        }
        Err(error) => {
            let failure = Failure::classify(&error);
            if !failure.retryable_with_other_roots() {
                // A transport failure is NOT remembered. A school network that
                // drops one packet must not pin this process to one store for
                // the rest of the session — the next attempt gets a fresh verdict.
                return HubClient {
                    client: primary_client,
                    diagnosis: None,
                };
            }

            let error_text = error.to_string();
            let secondary = primary.fallback();
            tracing::warn!(
                failed = %primary.label(),
                %error_text,
                "{} could not verify the hub's certificate; using {} instead",
                primary.label(),
                secondary.label(),
            );
            let diagnosis = Diagnosis {
                failed_store: primary,
                used_store: secondary,
                error: error_text,
            };
            remember(hub, Some(diagnosis.clone()));
            HubClient {
                client: bundled_or_empty(secondary, timeout, redirects),
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
fn bundled_or_empty(store: RootStore, timeout: Timeout, redirects: Redirects) -> &'static reqwest::Client {
    client_for(store, timeout, redirects).unwrap_or_else(|error| {
        tracing::error!("locus hub client ({store:?}): {error:#}; falling back to a bare client");
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

    let primary = RootStore::primary();
    let client = client_for(primary, Timeout::Activation, Redirects::Follow)?;
    match client.get(&probe).send().await {
        Ok(_) => anyhow::bail!(
            "{} verified the hub at {hub}, so a certificate failure there is not a root-store problem",
            primary.label()
        ),
        Err(error) => {
            let failure = Failure::classify(&error);
            if !failure.retryable_with_other_roots() {
                anyhow::bail!(
                    "the hub at {hub} was not reachable ({failure:?}), which is not a certificate problem: {error}"
                );
            }
            Ok(Diagnosis {
                failed_store: primary,
                used_store: primary.fallback(),
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
        assert!(Failure::Certificate.retryable_with_other_roots());
        for failure in [
            Failure::Dns,
            Failure::Refused,
            Failure::Transport,
            Failure::Protocol,
            Failure::Response,
        ] {
            assert!(
                !failure.retryable_with_other_roots(),
                "{failure:?} must not be retried with different roots: the roots cannot change it"
            );
        }
    }

    #[test]
    fn a_certificate_failure_offers_a_remedy_and_a_transport_one_does_not() {
        assert!(Failure::Certificate.remedy().is_some());
        assert!(Failure::Transport.remedy().is_none());
    }

    /// A name that does not resolve is its own outcome, and it has a remedy.
    ///
    /// This is the defect the school-network report named. `Dns` used to be
    /// folded into `Transport`, whose remedy is `None` — so the student saw
    /// "could not reach the Locus hub at …: error sending request for url (…)"
    /// and nothing else, with no way to learn that the name had not resolved.
    ///
    /// **Updated 2026-11-09.** This test used to assert the opposite of its last
    /// two checks: that the sentence must NOT mention the activation tunnel and
    /// that `Dns` must NOT be retryable over it. Both were right for a tunnel
    /// whose outbound dialled the hub hostname, and both became wrong when the
    /// outbound gained a compiled address candidate
    /// (`activation_tunnel::bootstrap_targets`). The assertions are inverted here
    /// rather than removed, because the *property* they protect — the sentence and
    /// the retry must agree — is the one that is easy to break by changing only
    /// one of them. The pair is also asserted directly in
    /// `a_name_that_does_not_resolve_must_reach_the_tunnel_now_that_it_carries_an_address`.
    ///
    /// FALSIFIED before keeping: removing the `Dns` arm from `remedy()` makes the
    /// first assertion fail, and removing the `is_name_resolution` check from
    /// `classify` makes the last one fail.
    #[test]
    fn a_name_that_does_not_resolve_is_named_and_has_a_remedy() {
        let remedy = Failure::Dns.remedy().expect("a DNS failure must have a remedy");
        let lowered = remedy.to_lowercase();
        assert!(
            lowered.contains("look up") || lowered.contains("dns"),
            "must say the name did not resolve, got {remedy:?}"
        );
        assert!(
            lowered.contains("network"),
            "must name the network as the usual cause, got {remedy:?}"
        );
        assert!(
            lowered.contains("activation tunnel"),
            "the sentence and the retry must agree: \
             `retryable_over_tunnel` says this failure reaches the tunnel, so the \
             sentence has to name it. Got {remedy:?}"
        );
        assert!(
            Failure::Dns.retryable_over_tunnel(),
            "the address candidate needs no resolver, so a naming failure is exactly the \
             case the tunnel can rescue"
        );
    }

    /// A refusal is distinct from a drop, and from DNS.
    #[test]
    fn a_refusal_is_its_own_outcome_with_its_own_sentence() {
        let remedy = Failure::Refused.remedy().expect("a refusal must have a remedy");
        assert!(
            remedy.to_lowercase().contains("nothing there accepted"),
            "must say the address was found and refused, got {remedy:?}"
        );
        assert_ne!(
            Failure::Refused.remedy(),
            Failure::Dns.remedy(),
            "the two causes need different sentences: one is the network, one is the hub"
        );
    }

    /// **The 2026-11-06 field report, as a rule.**
    ///
    /// A school network blocked the hub by name-reputation, and the client's one
    /// transport for that case was attempted only on a reset. The appliance can
    /// equally answer by refusing (it terminated the flow before the hub did), and
    /// on that day it did — so the bootstrap tunnel, which was reachable, was never
    /// dialled and the student could not enrol.
    ///
    /// FALSIFIED before keeping: reverting `retryable_over_tunnel` to
    /// `matches!(self, Self::ResetDuringHandshake)` fails on the first assertion,
    /// and so does removing `Refused` from the set while leaving the sentence in
    /// place. The sentence is asserted too, because a remediation path nobody is
    /// told about is the same bug wearing a different hat.
    #[test]
    fn a_refusal_must_reach_the_activation_tunnel() {
        assert!(
            Failure::Refused.retryable_over_tunnel(),
            "a filter that tears the flow down reaches the client as a reset OR a \
             refusal depending on how far it got; excluding either strands the student"
        );
        assert!(
            Failure::ResetDuringHandshake.retryable_over_tunnel(),
            "the reset case must stay — it is the one that was measured"
        );
        for not_retryable in [
            Failure::Certificate,
            Failure::Protocol,
            Failure::Transport,
            Failure::Response,
        ] {
            assert!(
                !not_retryable.retryable_over_tunnel(),
                "{not_retryable:?} must not spawn an engine: a different transport cannot \
                 change it, and on a heartbeat loop that would be one spawn per beat"
            );
        }

        let remedy = Failure::Refused
            .remedy()
            .expect("a refusal a filter caused must have a remedy")
            .to_lowercase();
        assert!(
            remedy.contains("activation tunnel"),
            "the refusal sentence must name the retry, or the student is told nothing \
             happened when one did: {remedy:?}"
        );
    }

    /// **A DNS failure must reach the tunnel, because the tunnel can now dial an
    /// address.**
    ///
    /// WHY THIS EXISTS. `Dns` was deliberately excluded from
    /// `retryable_over_tunnel` with a reason that was true when it was written:
    /// the bootstrap outbound's `server` was the hub hostname, so a resolver that
    /// answered nothing answered nothing for the fallback too. That changed on
    /// 2026-11-09 — `activation_tunnel::bootstrap_targets` now adds a compiled
    /// address candidate — and the exclusion became the one thing standing between
    /// a DNS-filtered student and the only transport that can carry enrolment.
    ///
    /// This test is the pair: the variant must be retryable, and the sentence must
    /// say so. A retry nobody is told about, or a sentence promising a retry that
    /// is not attempted, are the two ways this goes wrong.
    ///
    /// FALSIFIED before keeping: restoring `matches!(self,
    /// ResetDuringHandshake | Refused)` fails the first assertion.
    #[test]
    fn a_name_that_does_not_resolve_must_reach_the_tunnel_now_that_it_carries_an_address() {
        assert!(
            Failure::Dns.retryable_over_tunnel(),
            "a DNS-filtered network is the case the compiled address candidate exists \
             for; not retrying it means the candidate is never dialled"
        );
        let remedy = Failure::Dns
            .remedy()
            .expect("a naming failure must carry a remedy")
            .to_lowercase();
        assert!(
            remedy.contains("activation tunnel"),
            "the sentence must name the retry now that it exists: {remedy:?}"
        );
        assert!(
            remedy.contains("hotspot"),
            "the address is a compiled constant and can go stale; the hotspot advice is \
             what lets a student tell a stale address from a filtering network: {remedy:?}"
        );
    }

    /// The classification is walked from the chain, so these build real ones.
    ///
    /// `io::ErrorKind` is what the code reads, so the test injects kinds — not
    /// message strings — exactly as `is_name_resolution` and the reset check do.
    /// A test that matched text here would pass while the shipped classifier read
    /// kinds, which is the failure mode this repository has paid for before.
    #[test]
    fn io_kinds_in_the_chain_decide_between_dns_and_refusal() {
        use std::io::ErrorKind;

        for kind in [ErrorKind::NotFound, ErrorKind::HostUnreachable] {
            assert!(is_name_resolution(kind), "{kind:?} must be read as a naming failure");
        }
        for kind in [
            ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset,
            ErrorKind::TimedOut,
            ErrorKind::Other,
        ] {
            assert!(
                !is_name_resolution(kind),
                "{kind:?} must NOT be read as a naming failure: it would tell a student \
                 their DNS is filtering when the hub refused or the link dropped"
            );
        }
    }

    /// A reset that arrives AFTER the request was written must reach the tunnel.
    ///
    /// WHY THIS EXISTS. `reset_in_chain` is only consulted when reqwest reports
    /// `is_connect()`, which is true of a connection that failed to establish and
    /// **false** of a request that was refused or reset once it was sent. A
    /// filtering appliance that proxies the ClientHello and then tears the flow
    /// down produces exactly the second shape, and it reaches this process as an
    /// `UnexpectedEof`, a `BrokenPipe` or an `InvalidData` in the source chain —
    /// which classified as `Transport`, was NOT retryable over the tunnel, and so
    /// the one transport built for a censored network was never attempted.
    ///
    /// Measured on 2026-11-09 against the deployed hub: `curl -v` to the
    /// bootstrap port connected, sent the request off completely, and then
    /// reported `Recv failure: Connection reset by peer`. That is the shape this
    /// test pins.
    #[test]
    fn a_reset_after_the_request_was_sent_still_reaches_the_tunnel() {
        use std::io::ErrorKind;

        for kind in [
            ErrorKind::ConnectionReset,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
            ErrorKind::InvalidData,
        ] {
            assert!(
                is_reset_kind(kind),
                "{kind:?} in the chain is an appliance cutting the flow after it read the \
                 name; not retrying it leaves a student with no route at all"
            );
        }

        // The distance matters. These mean something else — a name that did not
        // resolve, a machine that answered, a link that hung up — and retrying
        // any of them spawns a 60 MB engine per attempt on the network where it
        // cannot help.
        for kind in [
            ErrorKind::NotFound,
            ErrorKind::HostUnreachable,
            ErrorKind::ConnectionRefused,
            ErrorKind::TimedOut,
        ] {
            assert!(
                !is_reset_kind(kind),
                "{kind:?} must not be read as a reset: it has its own classification and \
                 its own remedy"
            );
        }
    }

    #[test]
    fn the_store_labels_say_whose_store_it_is() {
        assert!(RootStore::Platform.label().contains("operating system"));
        assert!(RootStore::Bundled.label().contains("bundled with Locus"));
    }

    #[test]
    fn a_hub_url_must_be_https_with_a_host() {
        assert!(ensure_hub_url("https://hub.example").is_ok());
        assert!(
            ensure_hub_url("http://hub.example").is_err(),
            "plain http must never be a hub"
        );
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
            platform_client(Timeout::Activation, Redirects::Follow).is_ok(),
            "the platform client must build"
        );
        assert!(
            bundled_roots_client(Timeout::Activation, Redirects::Follow).is_ok(),
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
        let client =
            bundled_roots_client(Timeout::Activation, Redirects::Follow).expect("the bundled-roots client must build");
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
        let client = platform_client(Timeout::Activation, Redirects::Follow).expect("the platform client must build");
        let response = client
            .get("https://networkingguides.jadedns.uk/api/health")
            .send()
            .await
            .expect("the platform store must verify the hub on this machine");
        assert!(response.status().is_success());
    }
}
