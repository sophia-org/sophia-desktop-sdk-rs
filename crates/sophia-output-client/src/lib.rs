//! Developing output session client. Fixed-file startup is implemented;
//! negotiation, custody and automatic topology fetching remain in progress.
pub use sophia_output_protocol as protocol;
/// Provisional fixed-file startup for the developing output driver.
#[doc(hidden)]
pub mod startup;
