# C and Rust SDK API parity

Matrix revision 1, 2026-10-16. Sophia t333 owns the complete parity gate.
This first slice maps shell event-loop operations; it is not a complete role
inventory. Contract inputs are the pinned API-1 shell files and WM custody
rules in `spec/`, checked by `spec/SHA256SUMS`. C comparison source:
`sophia-desktop-sdk-c` `65ea947` (library bytes unchanged from `ba09265d`).

| Operation | Native C | Native Rust | Contract or remaining difference |
| --- | --- | --- | --- |
| Monitor the socket | `sophia_ss_poll_fd` | `ShellConnection: AsFd` | C retains caller ownership; Rust lends its owned descriptor. Never bypass SDK I/O. |
| Read/write interests | `sophia_ss_poll_events` | `ShellConnection::io_interest` | Read while live; write only for queued transport bytes. Recompute after progress. |
| Local work | Submit queues transport output; drain `sophia_ss_event`, consume and acknowledge obligations | `io_interest().immediate`; drive `poll_io` and drain typed receive methods | Do not sleep with undrained work. Rust also flags newly admitted work and exhausted drive rounds. |
| Nonblocking service | `sophia_ss_dispatch` | `ShellConnection::poll_io` | C takes a byte budget, with 32 syscalls per direction. Rust has at most 32 rounds of 256 KiB/64 syscalls per direction. Configurable Rust budgets remain a gap. |
| Retry timer | `sophia_ss_timeout(now_ms)` | `wake_deadline()` | C uses caller-supplied monotonic milliseconds; Rust uses `Instant` internally. Clock control remains a gap. |
| Retry identity | Same staged transaction, epoch and submission ID | Same staged transaction, epoch and submission ID | Neither retries in the refusal pass or replays across epochs. C backoff is 4–256 ms; Rust is 2–64 ms. Resolve timing policy before claiming full behavioral parity. |
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

Remaining inventory: every shell revision, operation, event, terminal outcome,
admission/reservation and native lifecycle helper; generic transport, launch and
namespace ownership; WM/output clients missing in Rust; experimental lock client
missing in Rust; administration missing in both. Broker and portal coverage must
follow their accepted public contracts. No missing capability is supplied by an
IPC fallback. A codec alone does not establish client parity.
