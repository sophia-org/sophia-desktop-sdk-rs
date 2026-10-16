//! WM client implementation in progress. Typed candidates own immutable bytes;
//! file startup and ordered journal assembly are bounded. The submission
//! driver, session facade and custody integration remain in progress.
//! Startup and journal modules are provisional driver interfaces.
mod candidate;
pub use candidate::{Candidate, EncodedCandidate};
pub use sophia_wm_protocol as protocol;
/// Provisional ordered journal implementation for the session driver.
#[doc(hidden)]
pub mod journal;
/// Startup implementation used by the developing session driver. Not a
/// negotiated WM session or a stable application entrypoint yet.
#[doc(hidden)]
pub mod startup;
