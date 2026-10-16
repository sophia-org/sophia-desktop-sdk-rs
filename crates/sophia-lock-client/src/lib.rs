//! Experimental lock-provider client foundation. The pinned contract is draft;
//! this crate does not yet supply a usable session. Providers only render.
pub use sophia_lock_protocol as protocol;
mod candidate;
pub use candidate::{Candidate, Prepared};
/// Provisional ordered journal; session integration remains in development.
#[doc(hidden)]
pub mod journal;
/// Provisional exact publication fetch, including supersession.
#[doc(hidden)]
pub mod object;
/// Provisional fixed-file startup for the experimental session.
#[doc(hidden)]
pub mod startup;
/// Provisional submission lane; public admission policy belongs to Session.
#[doc(hidden)]
pub mod submission;
