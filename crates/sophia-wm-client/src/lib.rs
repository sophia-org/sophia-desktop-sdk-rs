//! WM client implementation in progress. Typed candidates own immutable bytes;
//! file startup is bounded, but negotiation, event delivery and custody are
//! not implemented yet. Startup remains a provisional driver interface.
mod candidate;
pub use candidate::{Candidate, EncodedCandidate};
pub use sophia_wm_protocol as protocol;
/// Startup implementation used by the developing session driver. Not a
/// negotiated WM session or a stable application entrypoint yet.
#[doc(hidden)]
pub mod startup;
