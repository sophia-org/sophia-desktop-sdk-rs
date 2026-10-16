# C and Rust SDK API parity

Matrix revision 4, 2026-10-16. Sophia t333 owns the complete parity gate.
This matrix maps shell event-loop and content-admission operations; it is not a complete role
inventory. Contract inputs are the pinned API-1 shell files and WM custody
rules in `spec/`, checked by `spec/SHA256SUMS`. C comparison source:
`sophia-desktop-sdk-c` baseline `cfa6561`, plus this change series' reservation
identity fix (unreleased 0.10.0-dev; see below).

| Operation | Native C | Native Rust | Contract or remaining difference |
| --- | --- | --- | --- |
| Monitor the socket | `sophia_ss_poll_fd` | `ShellConnection: AsFd` | C retains caller ownership; Rust lends its owned descriptor. Never bypass SDK I/O. |
| Read/write interests | `sophia_ss_poll_events` | `ShellConnection::io_interest` | Read while live; write only for queued transport bytes. Recompute after progress. |
| Local work | Submit queues transport output; drain `sophia_ss_event`, consume and acknowledge obligations | `io_interest().immediate`; drive `poll_io` and drain typed receive methods | Do not sleep with undrained work. Rust also flags newly admitted work and exhausted drive rounds. |
| Nonblocking service | `sophia_ss_dispatch` | `ShellConnection::dispatch(now, byte_budget)` | Both bound the whole call to the supplied bytes and 32 syscall attempts per direction. Zero budget permits local processing without socket I/O. Rust's convenience `poll_io` retains its separate fixed limits. |
| Retry timer | `sophia_ss_timeout(now_ms)` | `wake_deadline()` with `dispatch(now, byte_budget)` | Both accept caller time and reject backward supplied times before changing state. Rust uses `Instant`; C uses monotonic milliseconds. |
| Retry identity | Same staged transaction, epoch and submission ID | Same staged transaction, epoch and submission ID | Neither retries in the refusal pass or replays across epochs. Both start at 4 ms and double to 256 ms; progress can permit an earlier retry. |
| Terminal connection | Poll interests become zero; inspect session state/error | `io_interest()` returns a terminal error | Stop monitoring; drain already received events and inspect custody. No automatic reconnect or replay. |
| Convenience wait | Caller composes poll, timer and dispatch | `wait_for_io` | Rust retains its existing convenience method. |

Rust checks: `external_loop_interest_tracks_queued_bytes_and_buffered_replies`,
`external_loop_drains_local_events_and_does_not_watch_idle_writability`,
`external_loop_stops_monitoring_a_closed_connection`, malformed-reply poison
controls, and the existing readiness/deadline tests. C counterparts:
`poll_reports_queued_output` in `shell_session_events_test.c` and
`eagain_waits_for_progress` in `shell_session_custody_test.c`. Both suites use
independent scripted peers. These checks do not establish production-export
qualification or physical desktop acceptance.

Dispatch controls add a Rust test that advances caller time through the full
4–256 ms schedule without sleeping, preserves staged transaction identity and
rejects backward time without progress. One-byte shell dispatches still reach
Submitted custody. Generic transport tests verify exact partial writes and
reads and that reusing a budget does not replenish it. C's matching partial
request control is `service_byte_budget_bounds_partial_requests`; its existing
`clock_rules` checks backward time and timestamp saturation.

Clock representations differ at their upper limit: C saturates deadlines at
`UINT64_MAX`; Rust rejects a supplied `Instant` that cannot represent the maximum
retry interval. Neither wraps time. With Rust's explicit driver, use `take_*`
receives; `poll_*` and `wait_for_io` drive the default clock and budget. Simulated
time must not be mixed with those convenience calls. These are documented API
differences, not evidence that every shell operation already has a counterpart.

## Content admission and reservations

| Operation | Native C | Native Rust | Contract or remaining difference |
| --- | --- | --- | --- |
| Atomic admission | `sophia_ss_submit` | `enqueue_content_tracked`, `enqueue_content_group_tracked` | All encoded units enter the bounded queue or none. Local admission is not server custody. Rust groups currently represent a whole candidate, not an arbitrary list of records. |
| Reserve before choosing records | `sophia_ss_reserve` | `reserve_capacity(slots, bytes)` | One live token; dispatch and admissions using unreserved capacity can continue. Rust reserves from its bulk allowance, preserving control credits. |
| Commit delayed content | `sophia_ss_commit` | `commit_reserved_content`, `commit_reserved_content_group` | Both retain credits on invalid input/size and issue tickets only on successful admission. Rust still encodes/allocates at commit; its group is a whole candidate, not an arbitrary batch. |
| Prepare known records | Reserve then commit | `reserve_content`, `reserve_content_group` | Rust's additional prepared handle exclusively borrows the connection; its commit needs no allocation, I/O or capacity check. |
| Cancel | `sophia_ss_cancel` | `cancel_reservation(&token)`; prepared handle `cancel`/Drop | Delayed-content tokens require explicit cancellation in both SDKs. Rust's prepared handle alone cancels on Drop. No admission or tickets. |
| Token identity | Session address, connection epoch and serial (0.10.0-dev) | Private shared identity checked against the connection | Wrong-connection and stale tokens cannot consume the live reservation. Do not move/reinitialize C session storage while a reservation is live. |
| Custody query | `sophia_ss_outcome` | `custody(Ticket)` | Local/admitted, in-flight, Submitted, refused errno, dropped-unsent and unknown map to the corresponding Rust values. Submitted is not semantic acceptance. |
| Old/unknown ticket | UNAVAILABLE for evicted, ARGUMENT for unissued | `None` for either | Rust does not currently distinguish those cases. Both retain the latest 256 tickets. |

Rust scripted controls cover cancellation/drop without tickets or I/O, commit
to Queued and then Submitted, invalid groups, full queues, upload-slot reuse,
and a peer disappearing while the reservation is held. A compile-fail example
checks exclusive borrowing; an outbox test checks capacity acquisition before
commit and preservation of payload storage. C's `reservation` control covers
cancel, capacity isolation, invalid commits and closure while reserved.

Rust preparation uses the existing encoder and local checks. Runtime grant,
upload and semantic decisions still belong to the session. For lifecycle-aware
candidate registration, use the existing `enqueue_candidate_tracked`; a plain
content reservation does not update `ContentLifecycle`. Native lifecycle
helpers and arbitrary multi-record groups remain open under t333.

The delayed-content Rust token now survives connection service. Tests cover
reserved byte/slot isolation, control credits, unused-credit release, invalid
records, wrong-connection/stale tokens, cancellation and terminal release.
Allocation failure can still refuse a delayed-content commit while preserving
credits; use a prepared handle when the local side effect needs an infallible
commit. Rust takes nonzero bounded byte/slot counts; its fixed bulk/control
partition differs from C's configurable queue limits.

C regression `reservation_is_bound_to_its_session` failed on `cfa6561`: two
live sessions with the same reservation serial accepted one another's token.
The companion fix checks owner address and epoch as well as serial. This expands
the public reservation struct; 0.10.0-dev is unreleased and C consumers must
recompile before adoption. It changes no wire record or server authority.

Remaining inventory: every shell revision, operation, event, terminal outcome
and native lifecycle helper; generic transport, launch and
namespace ownership; WM/output clients missing in Rust; experimental lock client
missing in Rust; administration missing in both. Broker and portal coverage must
follow their accepted public contracts. No missing capability is supplied by an
IPC fallback. A codec alone does not establish client parity.
