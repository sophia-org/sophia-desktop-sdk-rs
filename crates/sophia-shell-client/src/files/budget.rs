//! Memory budgets for fetched snapshot objects.
//!
//! RAW: an object's encoded bytes grow only by exact reservation (no
//! doubling), never past the feed's cap (`FEEDS`), and a failed reservation
//! is an error, not an abort.
//!
//! DECODED: the codecs allocate exactly what the declared counts ask for,
//! and every count and text length is bounded before allocation
//! (`table_count`, `take_text_padded`, `rows` with exact capacity), so a
//! decoded object's heap is at most the budget below. After decoding, the
//! client measures the value's actual capacities against that budget and
//! refuses it if it exceeds it, so no construction change can silently
//! breach the bound. Peak per fetch is the raw cap plus the decoded budget;
//! the inbox keeps at most one undelivered decoded object per feed.
//!
//! The `BTreeMap` per-entry charge is conservative: a leaf node holds up to
//! eleven `(u16, String)` entries (26 bytes each here) plus length and parent
//! fields in about 300 bytes, and is never less than half full except at
//! the root, so under 60 bytes per entry; 96 is charged.

use std::mem::size_of;

use sophia_shell_protocol::{
    SOPHIA_SHELL_APPLICATION_KEYWORDS_MAX_BYTES, SOPHIA_SHELL_APPLICATION_LABEL_MAX_BYTES,
    SOPHIA_SHELL_CATALOG_IDENTITY_MAX_BYTES, SOPHIA_SHELL_MAX_APPLICATIONS,
    SOPHIA_SHELL_MAX_INDICATOR_LABEL_BYTES, SOPHIA_SHELL_MAX_INDICATORS,
    SOPHIA_SHELL_MAX_OUTPUT_STATUS, ShellApplicationDescriptor, ShellIndicator,
    ShellIndicatorSnapshot, ShellOutputStatus, ShellPersistentCatalog,
};

const MAP_ENTRY_OVERHEAD: usize = 96;

/// The most heap a decoded persistent catalog may hold: every entry at its
/// longest label and keywords, and every identity at its longest.
pub(super) const CATALOG_BUDGET: usize = size_of::<ShellPersistentCatalog>()
    + SOPHIA_SHELL_MAX_APPLICATIONS
        * (size_of::<ShellApplicationDescriptor>()
            + SOPHIA_SHELL_APPLICATION_LABEL_MAX_BYTES
            + SOPHIA_SHELL_APPLICATION_KEYWORDS_MAX_BYTES)
    + SOPHIA_SHELL_MAX_APPLICATIONS
        * (MAP_ENTRY_OVERHEAD + SOPHIA_SHELL_CATALOG_IDENTITY_MAX_BYTES);

/// The most heap a decoded indicator snapshot may hold.
pub(super) const INDICATORS_BUDGET: usize = size_of::<ShellIndicatorSnapshot>()
    + SOPHIA_SHELL_MAX_OUTPUT_STATUS
        * (size_of::<ShellOutputStatus>() + SOPHIA_SHELL_MAX_INDICATOR_LABEL_BYTES)
    + SOPHIA_SHELL_MAX_INDICATORS
        * (size_of::<ShellIndicator>() + SOPHIA_SHELL_MAX_INDICATOR_LABEL_BYTES);

/// A decoded catalog's heap, from its actual capacities.
pub(super) fn catalog_bytes(catalog: &ShellPersistentCatalog) -> usize {
    let entries = &catalog.catalog.entries;
    size_of::<ShellPersistentCatalog>()
        + entries.capacity() * size_of::<ShellApplicationDescriptor>()
        + entries
            .iter()
            .map(|entry| entry.label.capacity() + entry.keywords.capacity())
            .sum::<usize>()
        + catalog
            .identities
            .values()
            .map(|identity| MAP_ENTRY_OVERHEAD + identity.capacity())
            .sum::<usize>()
}

/// A decoded indicator snapshot's heap, from its actual capacities.
pub(super) fn indicators_bytes(snapshot: &ShellIndicatorSnapshot) -> usize {
    size_of::<ShellIndicatorSnapshot>()
        + snapshot.statuses.capacity() * size_of::<ShellOutputStatus>()
        + snapshot
            .statuses
            .iter()
            .map(|status| status.layout.capacity())
            .sum::<usize>()
        + snapshot.indicators.capacity() * size_of::<ShellIndicator>()
        + snapshot
            .indicators
            .iter()
            .map(|indicator| indicator.label.capacity())
            .sum::<usize>()
}

// The arithmetic above, checked at compile time: a type growing
// unexpectedly fails the build here.
const _: () = assert!(CATALOG_BUDGET < 4 * 1024 * 1024);
const _: () = assert!(INDICATORS_BUDGET < 64 * 1024);
