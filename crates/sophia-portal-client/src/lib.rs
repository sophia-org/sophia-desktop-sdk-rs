//! Developing recipient client components; callers supply authenticated transport.
pub mod read_window;
pub mod snapshot;
pub mod startup;
pub use sophia_portal_protocol as protocol;
