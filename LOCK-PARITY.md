# Experimental lock SDK parity

Revision 3, 2026-10-16. The pinned lock file contract remains draft.
C reference: `cce3196`, `src/sophia_lock_client.h` and `src/lock_files/client*.c`.
Rust `sophia-lock-client` has preparation, provisional startup and submission;
it is not yet a usable lock session.

| Capability | Rust status / remaining work |
| --- | --- |
| Candidate preparation | All seven candidate kinds; epoch/submission envelope, typed body validation, fixed 128-byte owned storage. Three tests cover literal bytes, caller mutation, full-size presentation and invalid shapes/offers. |
| Direct/attached startup | Provisional `Opening` on a versioned pipeline. Checks exact API epoch, matching Limits, four fixed streams and admitted-root handoff. Nonblocking version facade/session integration remains. |
| Negotiation | Local offer checks match C: Present required, known bits, chords require Chords, valid revision range. Server decides unsupported revision and semantic chord refusal. Welcome/refusal handling remains. |
| Submission and custody | Provisional bounded lane: one immutable submission, explicit EAGAIN retry, cumulative Submitted ACK barrier and latest-result inspection. Clunk/walk after settlement; reopen after ACK. Session integration remains. |
| Journal | Missing. Retained ordered events, internal controls, character-free Entry and granted Chord validation. |
| Lock object | Missing. Exact publication/Qid/generation through EOF; consume superseded announcements internally. This differs from output topology fetching. |
| Upload | Missing. Begin/admission, slot open, borrowed or owned chunk lifetime, end/cancel/retire, resource events and recovery. |
| Upload window | Missing. C supports 1–8 writes with five control slots reserved; drain outstanding writes before cancellation. Pipelined short writes cancel, single-write mode can continue. |
| Event loop | Missing. C service has no internal clock; retry is explicit. Match capabilities without adding polling or hidden clock overhead. |
| Protected/production gates | Missing for the new Rust session; retain existing codec and raw-peer controls separately. |

Prepared Rust candidates own no heap storage and borrow no caller memory. Body
encoding currently uses one temporary Vec; writing the fixed envelope directly
avoids a second allocation. Preparation is not allocation-free, and no native
CPU improvement is claimed. C already encodes into fixed caller-owned storage.
Resource and allocation validity remain server decisions; encoding grants no
custody, presentation, credential access or unlock authority.

The C client accepts Candidate, FrameDemand and ResourceRetire publicly; its
upload helper owns Begin/End/Cancel, and startup owns Negotiate. The developing
Rust session must preserve those lifecycle boundaries rather than exposing
unrestricted raw admission through the preparation enum. C’s independent codec
and client suites pass; Rust’s three preparation tests and strict Clippy pass.

Seven Rust socket startup tests cover fragmented reads, exact decimal API epoch
(no zero, leading zero, overflow or suffix), Limits epoch matching, EOF/clunk,
control iounits, quiescent admitted roots, clock regression and unsent expiry.
Constructors queue without I/O. Startup keeps the transaction fid open read/write
and retains its iounit; submit and ACK are write-only. Rust currently requires
eight requests, sixteen fids, msize 4096–65536 and two-frame input capacity;
C accepts six requests for its default single-write mode. This conservative
Rust bound is explicit and must be revisited with upload-window integration.
Rust adds a caller-supplied startup deadline; C leaves that timer to its caller.
Four Rust submission tests cover both custody/reply orders, identical explicit
retries, EALREADY after confirmed custody, refusal, disconnect, invalid controls,
short writes and the ACK barrier before reopening the transaction. A contradictory
EAGAIN after Submitted now fails closed in both implementations, retaining custody;
C has an independent socket regression. Never replay a confirmed submission.
The Rust lane borrows bounded write slices from fixed storage and adds no clocks
or polling. Its body preparation still allocates temporarily. No measured runtime
gain is claimed. Fourteen Rust lock-client tests and strict Clippy pass.
Session negotiation, journal/object handling and uploads remain unimplemented.
