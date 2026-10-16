# Administrative SDK parity

Revision 5, 2026-10-16. Neither SDK yet provides the typed administrative
client. This is required t333 work, not an unsupported-role waiver.

The pinned file contract is copied unchanged from Sophia
`baedfd0c9`, `docs/sophia-admin-files.md`; SHA-256 is
`3cc2db4cce9d2154bf7765f074a7f78bd041c20bdc26903167d489f539fcba52`.
Its historical qualification paragraphs are not evidence for these SDKs.
The current runtime reference is `control/admin_client.rs`; passive codecs
are `sophia-protocol/src/admin_files{,/discovery}.rs` and
`control_records.rs`. The administrative owner and executor stay in Sophia.

| Required operation or behavior | Implementation/coverage needed in both SDKs |
| --- | --- |
| Connect and discover | HostDomain admission, standard 9P, immutable info snapshot and EOF, service/connection identity, validated advertised deadlines. No launch-role impersonation or protocol fallback. |
| Catalog | Complete bounded snapshot through EOF; 259 rows maximum, strict ordering, names with spaces and only the three defined Session operations. Retain exact generation. |
| Invocation encoding | Canonical nonzero identities and counted UTF-8 text, at most 512 bytes. One complete offset-zero ctl write; no cross-write staging or partial-write replay. |
| Admission | One request at a time, monotonic request IDs, validated catalog membership. A complete write refusal establishes non-acceptance; short or lost replies leave uncertainty. |
| Status | Reopen immutable ready/pending/outcome snapshots. Check connection, request and catalog generation and the owner's legal success outcomes. Reads never acknowledge an outcome. |
| Settlement | Preserve all ten outcomes, including timed-out and indeterminate. A newer accepted invocation acknowledges the prior outcome. No durable exactly-once claim. |
| External loop | Descriptor/readiness, bounded byte/syscall work, caller-monotonic deadlines and next wake. Poll status only while a command is pending; never poll while idle. |
| Cancellation and loss | Closing or flushing does not undo dispatched effects. Preserve uncertainty and forbid automatic reconnect/replay. Recovery requires a new explicit client and command. |
| Limits and lifetime | Bounded catalogs/status buffers, one immutable invocation, owned/borrowed lifetimes, request exhaustion and absolute deadlines that progress cannot extend. |
| Independent evidence | Literal malformed/boundary codecs, scripted partial/reordered traffic, same production owner/ticket scenarios, revocation, process loss and a foreign-language C binding smoke. |

Use the runtime's one-millisecond pending-status interval as a compatibility
reference, exposed through caller-driven wakeups. Do not copy its blocking
sleep loop into either SDK's external-loop API. Catalog ownership should
transfer or remain borrowed rather than cloning on every query. These are
design requirements; no client implementation or speedup is claimed here.

Both SDKs now decode the complete info record without heap allocation. Rust
returns a Copy value; C fills caller storage only after validating the entire
record. Independent tests cover literal values, maximum identities/timeouts,
zero and overflow refusals, every truncated prefix, noncanonical numbers,
extra fields, control bytes and malformed UTF-8. Parsing does not establish
EOF, admission or a deadline policy; those remain client responsibilities.
Both also decode invocation and ready/pending/outcome records without allocation.
Command names and outcome details borrow the input; keep that storage alive
and unchanged while using the result. All ten outcomes retain their distinct
values. UTF-8 byte counts and Unicode control-character rejection are checked.
Invocation encoding uses fixed 512-byte scratch storage, then copies the
complete record into caller storage. Invalid values or insufficient capacity
leave output unchanged. C also permits the source name to overlap destination
storage. Neither language turns a syntactically valid name into catalog authority.

Independent tests cover literal wire records, maximum IDs and name/detail
lengths, all outcomes, borrowed pointers, short output buffers, every truncated
prefix, malformed spelling and invalid text. Six Rust tests and the three C
administrative test binaries pass, with strict Rust Clippy.

Complete catalog decoding and binary-search membership lookup are now available
in both SDKs. The catalog contains at most 259 inline rows whose names borrow
the immutable input; neither parsing nor lookup allocates. Validation rejects
zero/noncanonical generations, excessive counts, missing or surplus rows,
duplicates, bad owner/name ordering, more than 256 policy actions and unsupported
Session names. C leaves the output catalog unchanged on failure; Rust returns
no catalog. Neither exposes a partially validated catalog.

Nine Rust administrative tests and all four C administrative test binaries pass.
Catalog tests cover all 259 commands, every truncated prefix of a literal
two-row snapshot, the 256-policy boundary, 128-byte names, malformed snapshots,
borrowed names and lookup. The external-loop clients and paired production
gates remain; no performance measurement or client parity is claimed.

The provisional snapshot components now produce Walk/Open/Read/Clunk operations
for info, commands and status on an existing connection. Reads respect msize,
iounit and file bounds, including a one-byte EOF probe at the bound. Results
remain unavailable until EOF, syntax validation and clunk succeed. Deadlines
are absolute caller-monotonic times; clock regression changes no state. Remote
refusal, invalid replies and expiry require the connection driver to close,
without automatic retry or reconnect.

Rust accepts reusable Vec storage, reserves the file bound once and returns
that storage without copying. C uses caller storage of at least the file bound.
Raw scratch bytes are not validated results. Three Rust component tests and
C's independent snapshot test cover fragmented reads of all three files,
storage identity, EOF/clunk barriers, malformed EOF, overflow, oversized replies,
clunk refusal, EAGAIN refusal, deadline and clock regression. Strict Clippy
passes. These tests supply matched replies; connection/tag ownership, readiness,
bounded dispatch, startup, invocation custody and production gates remain.
