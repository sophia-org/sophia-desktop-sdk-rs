//! Experimental lock-provider client foundation. The pinned contract is draft;
//! this crate does not yet supply a usable session. Providers only render.
pub use sophia_lock_protocol as protocol;
mod candidate;
pub use candidate::{Candidate, Prepared};
/// Provisional fixed-file startup for the experimental session.
#[doc(hidden)]
pub mod startup;
