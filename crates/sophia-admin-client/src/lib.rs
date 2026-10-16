//! Developing administrative session on a versioned pipeline. Raw-socket
//! connection and production qualification remain incomplete.
pub mod command;
pub mod session;
pub mod snapshot;
pub mod startup;
pub use sophia_admin_protocol as protocol;
