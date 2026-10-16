# WM API parity

Revision 1, 2026-10-16. Contract: the pinned API-1 WM files, schema and golden
records in `spec/`. C reference: `fbfcdd8`, `src/sophia_wm_session.h`;
Rust base: `622d9c0`, with the obligations and startup-wake changes in this
series. This maps implemented capabilities, not production qualification.

| C entry points | Rust counterpart | Ownership and evidence |
| --- | --- | --- |
| `state_bytes`, `storage_bytes` | Rust-owned allocations and `PipelineLimits` | C caller storage and borrowed fd remain distinct from Rust-owned memory/socket. Neither SDK needs the other language. |
| `open_fd` | `Connecting::direct` | One deadline across nonblocking version, file startup and negotiation. Supplied admission only; no authentication fallback. `tests/connecting.rs`, `tests/session.rs`. |
| `open_attached` | `Connecting::attached` | Exact quiescent sole root; no reset/reattach. `tests/startup.rs` tests valid handoff and rejects wrong roots, unfinished work and inadequate capacity. |
| `state`, `epoch`, `capabilities`, `limits` | Same-named `Session` getters | Ready means capability negotiation, not profile activation. Early-startup introspection through `Connecting` still needs review; it currently exposes readiness, fd, interests and deadlines. |
| `poll_fd`, `poll_events`, `timeout` | `fd`, `io_interest`, `wake_deadline` | Borrowed descriptor is monitoring-only. Recompute after API calls; immediate work precedes sleep. Connecting includes negotiation retry wakes. |
| `dispatch` | `dispatch(now, byte_budget)` | Caller monotonic milliseconds; shared per-call I/O budget. Rust allows zero-byte buffered work, whereas C refuses zero budget. Neither performs socket I/O at zero budget. |
| `obligations` | `Session::obligations` | Consumed/acked/submitted watermarks, deadlines, retries, event/snapshot readiness and ACK barrier. Terminal sessions retain watermarks and clear pending flags. |
| `remote_error` | `remote_error` | Wire errno retained separately from local/protocol failures. Full ESTALE/revocation peer coverage remains pending. |
| `event`, `consume` | `event`, `consume` | Borrowed stable head; controls remain internal; cumulative ACK never skips the head. `tests/journal.rs` and socket session tests. |
| `submit` | `submit(&Candidate, deadline)` | Seven application candidate kinds; copied immutable bytes, one candidate, local ticket. `tests/candidate.rs`, `tests/submission.rs`. |
| `outcome` | `custody` | Last 64 results; Submitted is custody, not commit. Refusal, unsent and uncertain disconnect remain distinct. C Unavailable maps to Rust None. |
| `next_transaction` | `next_transaction` | Independent monotonic domain IDs; externally assigned IDs must exceed the Submitted watermark. Refusal does not advance it. |
| `snapshot`, `snapshot_result`, `snapshot_release` | `request_snapshot`, `snapshot_result`, `release_snapshot` | One Cycle-bound fetch/result, cap/length/EOF/identity validation and clunk before exposure. Socket tests use a pinned golden row and literal file framing. |
| `close` | `close` / Drop | No replay or reconnect. Rust closes its owned descriptor; C leaves its borrowed descriptor to the caller. Custody remains queryable after explicit close. |

Candidate kinds are ProfilePrepared, ProfileActive, ProfileRolledBack,
Configuration, Dirty, Projection and SessionOperation. Event kinds are
ProfilePrepare, ProfileActivate, ProfileRollback, ConfigurationOutcome, Cycle,
ProjectionOutcome, SessionOperationOutcome and PresentationReceipt. Codec
coverage includes each; integrated socket tests do not yet exercise every
profile/outcome/Cycle-cause combination.

## Remaining qualification

The production export counterpart is Sophia's
`crates/sophia-session/tests/support/policy_file_c_sdk.rs` and its independent
C peer. It covers profile handoff, configuration, a multi-read snapshot and
policy exchange through the production adapter/worker. Add a native Rust peer
against the same scenario; do not infer interoperability from shared codecs.
This fixture supplies admission and outcomes, so it proves neither protected
launch authentication nor Engine commit/presentation.

Also finish early-startup introspection, public error/ownership documentation,
revocation and terminal-error peer scenarios, and the operation-to-export
evidence map. Publication, verified pins and foreign-language binding checks
remain t333 gates. These requirements cannot be waived by marking both SDKs
unsupported.
