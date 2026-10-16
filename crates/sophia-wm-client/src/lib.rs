//! WM client implementation in progress. Typed candidates own immutable bytes;
//! file startup, negotiation, ordered events and custody use the 9P pipeline.
//! Cycle-bound snapshot fetching and complete release qualification remain
//! pending. Startup, journal and submission modules are provisional interfaces.
mod candidate;
pub use candidate::{Candidate, EncodedCandidate};
pub use sophia_wm_protocol as protocol;
/// Provisional ordered journal implementation for the session driver.
#[doc(hidden)]
pub mod journal;
mod session;
/// Startup implementation used by the developing session driver. Not a
/// negotiated WM session or a stable application entrypoint yet.
#[doc(hidden)]
pub mod startup;
/// Provisional submission state machine for the session driver.
#[doc(hidden)]
pub mod submission;
pub use session::{Session, SessionError, SessionState};
