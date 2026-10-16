//! WM client implementation in progress. Typed candidates own immutable bytes;
//! transport, negotiation, event delivery and custody are not implemented yet.
mod candidate;
pub use candidate::{Candidate, EncodedCandidate};
pub use sophia_wm_protocol as protocol;
