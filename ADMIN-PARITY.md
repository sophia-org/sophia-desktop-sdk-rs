# Administrative SDK parity

Revision 1, 2026-10-16. Neither SDK yet provides the typed administrative
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
design requirements; no implementation or speedup is claimed here.
