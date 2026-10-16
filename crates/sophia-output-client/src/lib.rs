//! Developing output session client with fixed-file startup and an immutable
//! submission lane. Journal handling and topology fetching remain in progress.
pub use sophia_output_protocol as protocol;
/// Provisional fixed-file startup for the developing output driver.
#[doc(hidden)]
pub mod startup;
/// Provisional immutable submission lane for the developing output driver.
#[doc(hidden)]
pub mod submission;
