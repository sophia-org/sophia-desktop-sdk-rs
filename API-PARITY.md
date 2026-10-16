# C and Rust SDK API parity

Matrix revision 10, 2026-10-16. Sophia t333 owns the complete parity gate.
This matrix maps shell event-loop and content-admission operations; it is not a complete role
inventory. Contract inputs are the pinned API-1 shell files and WM custody
rules in `spec/`, checked by `spec/SHA256SUMS`. C comparison source:
`sophia-desktop-sdk-c` `a90e235` (unreleased 0.10.0-dev; see below).

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
| Atomic admission | `sophia_ss_submit` | `enqueue_content_batch` and existing single/group methods | All encoded units enter the bounded queue or none. Rust batches accept independent content records and whole candidates, each with its own transaction and ticket. Local admission is not server custody. |
| Reserve before choosing records | `sophia_ss_reserve` | `reserve_capacity(slots, bytes)` | One live token; dispatch and admissions using unreserved capacity can continue. Rust reserves from its bulk allowance, preserving control credits. |
| Commit delayed content | `sophia_ss_commit` | `commit_reserved_content_batch` and single/group methods | Both retain credits on invalid input/size and issue tickets only on successful admission. Rust still encodes/allocates at commit. |
| Prepare known records | Reserve then commit | `reserve_content_batch` and single/group methods | Rust's additional prepared handle exclusively borrows the connection; its commit needs no allocation, I/O or capacity check. |
| Cancel | `sophia_ss_cancel` | `cancel_reservation(&token)`; prepared handle `cancel`/Drop | Delayed-content tokens require explicit cancellation in both SDKs. Rust's prepared handle alone cancels on Drop. No admission or tickets. |
| Token identity | Session address, connection epoch and serial (0.10.0-dev) | Private shared identity checked against the connection | Wrong-connection and stale tokens cannot consume the live reservation. Do not move/reinitialize C session storage while a reservation is live. |
| Custody query | `sophia_ss_outcome` | `custody(Ticket)` | Local/admitted, in-flight, Submitted, refused errno, dropped-unsent and unknown map to the corresponding Rust values. Submitted is not semantic acceptance. |
| Old/unknown ticket | UNAVAILABLE for evicted, ARGUMENT for unissued | `custody_result` returns `Unavailable` or `Unissued` | Existing `custody` retains its `Option` API. Both keep the latest 256 tickets; unavailability is not an uncertain remote outcome. |

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
helpers and batch coverage beyond content records remain open under t333.

`ContentBatchItem` describes one content record or whole candidate. A batch has
1–64 operations, subject to remaining byte/slot credits; each item produces one
ticket and its own server submission or upload write. Mixed batches use bulk
credits; only all-ACK batches use control capacity. Staged resource starts take
distinct free slots, and failure or cancellation commits none of their claims.
An atomic local admission is not a server-wide transaction: later refusal or
disconnect can settle each ticket differently.

Rust batch tests cover transaction/order preservation, invalid middle records,
count/byte/slot limits, delayed reservation preservation, control headroom,
resource-claim rollback and mixed custody after disconnect. C's
`admission_atomicity` already checks invalid middle records, byte/slot bounds,
queued versus in-flight custody and terminal classification; its full suite
passes at `a90e235`. This does not yet map every descriptor, catalog or native
input operation into a common batch API.

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

## Custody lookup and native lifecycle prerequisites

Rust `custody_result` distinguishes an evicted ticket from zero or a number
never issued by that connection. Retained `Custody::Unknown` remains a separate
outcome. Queries perform no I/O and work after disconnect. Tickets belong to
one connection; numeric tickets do not detect accidental use on another one.
Rust ledger and scripted disconnect checks match C's `old_and_unissued_tickets`
retention-boundary test and existing uncertain-custody controls.

## Native launcher transport

The Rust client now supports the pinned revision-7 launcher profile: exact
capability bits 5, 7, 8 and 11, with Limits. Other roles cannot use its native
record API or deliver native events. The public entry points map as follows:

| Operation | Native C | Native Rust |
| --- | --- | --- |
| Allocation, input ACK, activation | `sophia_ss_submit` with native records | `enqueue_native_launcher_tracked` |
| Whole native candidate | `sophia_ss_submit` with NativeCandidate | `enqueue_native_candidate_tracked` |
| Observe native/shared events and catalog | `sophia_ss_event`, consume; `sophia_ns_next` adds lifecycle policy | `take_native_launcher_observation`, transport only |
| Keyboard activation plus ACK | `sophia_ss_submit`; helper `sophia_ns_input_reserve/commit` | `reserve_native_input_response`, or `enqueue_native_input_response` |

Rust's prepared response validates matching event identities and distinct
transactions, then holds both records before a UI edit. Activation precedes
ACK. Commit admits both or neither locally, with independent custody tickets;
Drop cancels. It does not decide whether focus permits activation, infer an
input disposition, or enforce the input ACK deadline. The existing content
reservation handle is reused; it exclusively borrows the connection.

Use the native observation stream exclusively when ordering matters: native,
content and catalog receive methods otherwise remove items from the same inbox.
The catalog carries application entries without persistent identities. Existing
snapshot coalescing and bounded fetching still apply. Journal acknowledgements
confirm delivery, not application input acknowledgement or action completion.

Seven Rust scripted-peer checks cover exact negotiation, all native events,
catalog ordering, allocation/whole-candidate custody, cancellation, invalid
response identity, queue saturation, malformed input, foreign epochs and
separate custody after disconnect. C `f8ca212` passes its full suite, including
`sophia_shell_files_roles_test` and `native_session_test`'s `test_input` and
`test_activation`. C helper tests use a session stand-in; these paired checks
are not production-export or physical qualification.


## Native presentation and keyboard lifecycle

Rust now supplies `NativeLauncherLifecycle` for the presentation/input portion
of C's `sophia_ns`. `observe` applies ordered decoded observations; `input`
retains an input until its response commits. Opening IDs must rise, Prepared
never enables targets, focus must match the shown candidate and current revision,
and a later exact focus supersedes the previous lease without a fabricated
lease-order rule. Activation admission disarms the opening; close and disconnect
remove input authority. A late Presented for a closed opening cannot restore it.

`enqueue_native_candidate_with_lifecycle` admits the exact candidate and its
metadata together; failed admission registers nothing. `prepare_native_input`
checks eligibility and reserves the response before an edit. Drop cancels and
retains the input; commit records its tickets and consumes it without allocation
or I/O. `service` reconciles custody, counts refused ACKs once and disarms a final
connection. No automatic retry of application effects or reconnect occurs.

The input deadline is issuance microseconds divided by 1000 plus the timeout,
saturating at the clock representation limit, matching C. It is an early host
CLOCK_MONOTONIC scheduling deadline, not proof of timely server service.
`NativeLauncherObservation::Published` exposes catalog/output announcements
before fetching completes; old local scenes are disarmed at announcement, not
merely when the replacement object is installed. Exhaustive matches must add
this variant. Consume one ordered stream; feed a snapshot after accepting it
for scene construction. The reducer does not render or install UI state.

Nine Rust lifecycle controls and the existing transport checks pass, as do
workspace tests, strict Clippy and C's full suite at `62b62f6`. C counterparts
are `native_session_test`'s focus, input, activation, catalog-invalidation,
close and terminal controls. That C helper suite uses a session stand-in;
production-export acceptance is still pending.

## Native resources and pacing

Rust now tracks allocation and resource ownership and frame demands/permits:

| Operation | Native C | Native Rust |
| --- | --- | --- |
| Acquire/resize/release | `sophia_ns_allocate`, `sophia_ns_release` | `allocate_native`, `release_native_allocation` |
| Upload | `sophia_ns_upload_begin/end/cancel`, `sophia_ns_resource_cancel`, `sophia_ss_upload_chunk` | `begin_native_resource`, `upload_native_chunk`, `end_native_resource`, `cancel_native_resource` |
| Retire/release | `sophia_ns_retire`, resource events | `retire_native_resource`, `observe` |
| Demand/cancel | `sophia_ns_demand`, `sophia_ns_demand_cancel` | `demand_native_frame`, `cancel_native_demand` |
| Construct a scene | `sophia_ns_present` | `present_native_scene` |
| Reconcile time and custody | `sophia_ns_service` | `service_at` |

Both retain at most 16 resource identities and one upload. Rust copies each
caller chunk and returns a Stored custody ticket; C borrows its buffer until
upload progress finishes. Caller writes may split the declared layout chunks.
Rust validates layout against Limits and requires Stored before another write
or End. Failed payload is not replayed; cancel the resource. Both SDKs offer explicit-resource cancellation after a refused End. C's
`sophia_ns_resource_cancel` also delegates current-upload cancellation to the
existing transport wrapper. Neither helper releases the resource identity until
its terminal event. Independent tests cover refused End, cancellation and
identity retention; C also checks queue saturation and stale generations.

A rejected resize preserves the grant. Close removes local allocation use but
waits for server invalidation before replacing an orphaned grant. Resources
retain their own retirement/release lifetime. Submitted retirement is not release;
a refused retirement restores Accepted state. The Rust scene helper uses current
allocation/scale and accepted resources, fills context generations, checks byte
bounds and consumes a permit only on successful local admission. The lower-level
candidate API still registers metadata without resource/pacing policy.

Permit time is advisory demand enqueue plus TTL, with a caller margin. Local
expiry disarms use but keeps the demand outstanding until authoritative end;
neither SDK can prove timely arrival from that bound. Tests cover candidate
refusal before and after permit expiry. C's new regression failed on `c678319`
before repair: expiry first caused an indefinite wait for the same end. The fix
preserves that terminal observation and leaves newer demands untouched.

The Rust file wire also leaked upload slots after definitively refused Begins;
a six-refusal regression failed before repair. It now releases the pending
claim and drops queued payload for vanished bindings without blocking later
records. Full Rust tests, strict Clippy and C `make check` pass; 25 Rust native
checks include paired allocation, resource, pacing and refusal cases. Export
qualification, native capture and performance claims remain separate.

Pointer action/ACK handling and the integrated native facade remain open, as do
delayed native reservations, other non-content batches and configurable queues.
These gaps still block full t333 parity and production adoption.
