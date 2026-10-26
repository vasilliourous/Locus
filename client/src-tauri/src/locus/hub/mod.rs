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
//! [`client`] tries one root store and, only on a **certificate** error, retries
//! the same request with the other. Neither store can be trusted to be the right
//! one on every machine, so which is first is a platform decision rather than a
//! global constant — [`roots::RootStore::primary`] answers it, and the reasoning
//! for each platform is in that module.
//!
//! The fallback is narrow on purpose: a timeout, a DNS failure or a refused
//! connection is not a certificate problem and must not be retried, because
//! retrying it only delays the message the student needs.
//!
//! [`diagnose`] records which store failed and why, so the log names the real
//! cause next time instead of borrowing rustls's vocabulary.
//!
//! # The two halves
//!
//! | Module | What it holds | Why it is separate |
//! |---|---|---|
//! | [`roots`] | [`roots::RootStore`] and the order to try the stores in | Pure: the *decision* is tested with no network, no OS and no app handle |
//! | this one (`client.rs`) | the clients themselves, the probe, the cache, [`Diagnosis`] | Needs a live hub |

mod client;
mod roots;

pub use client::{Diagnosis, Failure, HubClient, Timeout, client, diagnose, ensure_hub_url};
pub use roots::RootStore;
