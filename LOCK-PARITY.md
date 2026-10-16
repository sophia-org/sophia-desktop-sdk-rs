# Experimental lock SDK parity

Revision 1, 2026-10-16. The pinned lock file contract remains draft.
C reference: `cce3196`, `src/sophia_lock_client.h` and `src/lock_files/client*.c`.
Rust `sophia-lock-client` has typed immutable candidate preparation only;
it is not yet a usable lock session.

| Capability | Rust status / remaining work |
| --- | --- |
| Candidate preparation | All seven candidate kinds; epoch/submission envelope, typed body validation, fixed 128-byte owned storage. Three tests cover literal bytes, caller mutation, full-size presentation and invalid shapes/offers. |
| Direct/attached startup | Missing. Exact API epoch, Limits, four fixed streams and admitted-root handoff. |
| Negotiation | Local offer checks match C: Present required, known bits, chords require Chords, valid revision range. Server decides unsupported revision and semantic chord refusal. Welcome/refusal handling remains. |
| Submission and custody | Missing. Keep transaction fid open, one immutable submission, explicit retry after EAGAIN, cumulative Submitted ACK barrier and latest-result inspection. |
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
