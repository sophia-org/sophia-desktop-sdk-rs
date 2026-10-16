//! Developing output session client with fixed-file startup and an immutable
//! submission lane and ordered journal. Topology fetching and session
//! integration remain in progress.
pub use sophia_output_protocol as protocol;
/// Provisional ordered journal for the developing output driver.
#[doc(hidden)]
pub mod journal;
/// Provisional fixed-file startup for the developing output driver.
#[doc(hidden)]
pub mod startup;
/// Provisional immutable submission lane for the developing output driver.
#[doc(hidden)]
pub mod submission;
