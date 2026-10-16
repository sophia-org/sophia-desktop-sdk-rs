# Portal SDK parity

Revision 16, 2026-10-16. Recipient components are developing; there is no complete
portal client or parity claim.

The unchanged recipient contract in `spec/portal-client-files.md` comes from
Sophia `9b57fd040`, SHA-256
`d5e2bceed3ecbd56c119ecd87547f9437ee43a813b591df1f638f404681a5b97`.
Its installation and qualification statements describe that source snapshot,
not SDK evidence.

Both SDKs decode the exact 64-byte frame record into passive fixed-size values.
They validate version, format, nonzero identities, u32 dimensions, canonical
stride and the 64 MiB image bound without allocating or touching pixels.
C leaves output unchanged on failure. Independent tests cover truncated and
trailing records, the exact limit, oversized and overflowing dimensions,
invalid fields and a one-pixel image. Both pass, as does strict Rust Clippy.

Both SDKs now encode the exact 24-byte capture request and decode the complete
32-byte capture-status snapshot without allocation. All eight states remain
distinct. Idle requires zero identities; other states require nonzero serial
and output. Rust returns an inline byte array; C writes into caller storage,
leaving output unchanged on error and preserving bytes beyond the record.
Independent literal and malformed-boundary tests pass, including every truncated
status prefix, trailing bytes, invalid version/state and identity combinations.
The two Rust portal tests, both C portal tests and strict Rust Clippy pass.
These codecs neither enforce serial ordering nor confer frame authority.

Both now decode exact version-1 api and status snapshots without allocation.
Request capability and transfer state remain distinct passive values. Decoders
reject unknown capabilities, invalid epochs, truncation and trailing bytes.
The maximum api/status records are 101/43 bytes. Independent discovery tests
and strict Rust Clippy pass; C preserves caller output on failure.

Remaining recipient work includes authenticated startup, retained-object frame and transfer reads,
watch/cancellation, custody, revocation and external-loop APIs. Reads should
use caller-controlled storage and bounded chunks; do not create another full
image copy. Decoded metadata alone never authorizes pixels or proves presentation.
Production gates and foreign-language bindings remain required.

The trusted portal-requester is a separate authority and contract; no recipient
API may approve transfers or claim executor custody. Requester coverage and the
broker migration prerequisite remain explicit t333 work.

Provisional metadata fetchers now emit Walk/Open/Read/Clunk operations for api,
status and capture-status. They expose results only after EOF, decoding and
successful clunk. Reads obey msize, iounit and per-file limits, with a one-byte
EOF probe at the bound. Caller-monotonic absolute deadlines cannot be extended
by partial progress. Refusals preserve wire errno and forbid automatic retry;
the driver must match replies and close on terminal failure.

Rust retains at most 101 bytes inline; C borrows caller storage. These fetchers
allocate no storage themselves, though Rust's supplied transport Reply may own
allocated data. They adapt each SDK's administrative snapshot driver without
importing administrative authority or dependencies. Independent tests cover
all three files, fragmentation, EOF/clunk barriers, overflow, malformed EOF,
short iounit, refusal, expiry and clock regression. Three Rust component tests,
C's snapshot test and strict Rust Clippy pass. They do not authenticate or own
the connection; startup, frame streaming and production gates remain.

Metadata startup now accepts the sole attached root on a quiescent role
pipeline, fetches api through EOF/clunk and returns the same root and pipeline.
The caller must supply the root from completed launch authentication and keep
the relay alive separately; root ownership is checked but proves no authority.
No version, attach or reconnect occurs. One caller-monotonic deadline covers
startup. Rust shares a bounded I/O budget; C services its pipeline once per
dispatch. Failure drops Rust's role pipeline; C requires its caller to close it.
C startup state must not move because its fetch borrows inline scratch.

Independent socket peers pass one-byte dispatch, retained-root handoff, invalid
root, unsupported api version, directory replies, clunk refusal and expiry.
Five Rust client tests, C's startup test and strict Rust Clippy pass. Their
scripted initial attach stands in for completed authentication; these tests do
not qualify the real launch exchange. Authentication composition, recipient
session operations, retained frame reads and production gates remain.

Independently built C and Rust peers now pass Sophia's opt-in
`external_sdk_portal_startup_uses_real_namespace_authentication` gate.
The production namespace launcher and Factotum admit each protected peer;
namespace discovery selects the portal endpoint, and the SDK authenticates
before handing its existing root to metadata startup. Each observes the
unavailable request capability and retains only the root after api clunk.
The original launch relay remains open until peer exit. Strict Rust Clippy
passes. This qualifies startup composition, not frame delivery, capture
custody, revocation or physical presentation. No SDK pin adoption is implied.

The frame range scheduler now keeps four requests in fixed storage. Counts
respect msize minus reply framing, iounit and caller chunk limits. Out-of-order
replies place bytes at their reserved offsets; short reads schedule only the
unread suffix. Tokens belong to one window and must be matched to its wire tags.
If admission is busy, retain the reserved request and submit it once.
Zero-length, oversized, unknown and duplicate replies fail the window closed.
The caller closes it on timeout, cancellation, revocation or transport failure.

Independent tests cover reversed short reads with exact byte coverage, four-slot
pressure, framing/iounit limits, terminal errors and a full 64 MiB range without
allocating image storage. Both tests and strict Rust Clippy pass. Payload range
completion is not image completion: the eventual session must verify EOF,
reread the retained metadata fid to reject replacement, and clunk successfully.
No private qid arithmetic is required. This scheduler copies no pixels; wire
integration, frame storage ownership and production delivery remain unfinished.

Both now integrate retained-frame reads with the four-request window. They
retain frame-info and frame fids, require exact payload and EOF, reread the
original metadata fid, and require both clunks before exposing a completed
image. No private qid arithmetic or automatic reconnect is used. An explicit
second read reuses the root and image storage without fetching api again.

Rust owns the role pipeline and reusable Vec, growing it once if needed after
metadata validation. It preserves initialized storage between captures, avoiding
another zero-fill for an unchanged size. C borrows the pipeline and caller image
storage, rejects insufficient capacity and allocates nothing. Both place each
reply directly in the final image buffer; there is no second full-image copy.
Rust transports still allocate reply data. Rust drops its pipeline on failure;
C requires the caller to close its borrowed descriptor. Partial storage is
scratch, never a completed image. Retain the launch relay separately.

Independent socket tests pass short reads, one-byte dispatch, buffer/connection
reuse, stale or changed metadata, extra pixels, early termination, clunk refusal,
deadline expiry and cancellation. C additionally tests insufficient capacity.
All nine Rust client tests and strict Clippy pass; C's frame-client test passes.
These are scripted peers, not production frame-delivery or revocation evidence.
Capture requests and the remaining recipient operations are still unfinished.

Both independently built peers now pass production authenticated delivery of
a supplied 256×128 RGBA8 frame. The trusted fixture submits the capture request
and the real coordinator installs the frame; each SDK authenticates, reads it,
checks output/generation/presentation identities and every pixel, and retains
only the role root afterwards. Both external startup/delivery gates and strict
Rust Clippy pass. This qualifies the SDK/export boundary with supplied policy
and pixels, not SDK request custody, mid-read revocation, native presentation,
physical installation or performance.

Both peers also pass the production mid-read revocation gate. Each pauses after
accepting some pixels; the fixture advances the lock fence before reading
continues. Both terminate without a completed image, and deadline expiry cannot
pass the test. The new received-byte accessor reports scratch progress only.
Bytes already buffered before revocation may still arrive; this gate does not
claim they can be recalled. All three external portal gates, the existing
portal-dispatch regressions and strict Rust Clippy pass. Capture-request custody
and the remaining recipient operations still need implementation.

Capture-custody components now distinguish queued, issued, accepted, settled,
refused, dropped-unsent and unknown requests. They keep the 24-byte write inline
and allocate nothing. Pipeline admission marks issued; only an exact Rwrite
marks accepted. A complete Rlerror records refusal; short or lost replies forbid
replay. Closing locally cannot cancel work already admitted by the server.

Only fresh EOF/clunk-verified capture-status snapshots may settle a request.
Serial and output must match. Queued/capturing remain pending, while ready,
denied, unavailable, pressure and cancelled remain distinct terminal outcomes.
Ready grants no frame authority. A caller-monotonic deadline of at most ten
seconds covers the whole observation, without extension from partial progress.
Pending status schedules another snapshot no sooner than the next millisecond;
settled or failed requests schedule none.

Rust `capture::Pending` and C `sophia_pc_*` pass independent tests of all
outcomes, refusal, short replies, loss/cancellation, mismatched status, malformed
records, clock regression and fixed deadlines. All twelve Rust client tests
and strict Clippy pass; C's capture-client test passes. These are component
tests: serial allocation, write limits, wire driving and production request
qualification remain unfinished. Existing authenticated frame/revocation gates
do not qualify this new request path.

The request driver now composes custody with the authenticated role transport:
Rust `capture_request::Request` and C `sophia_pcr_*`. It checks the advertised
capability and caller-supplied serial high-water mark, opens capture write-only,
and admits one complete offset-zero write within msize/iounit limits. It closes
that fid before opening fresh status snapshots. Results require EOF and clunk;
pending status uses the caller's timer, without sleeping inside dispatch.
Failure closes Rust's owned pipeline; C callers must close their borrowed fd.
Neither driver retries or reconnects.

Successful handoff preserves the root and returns the serial for the next
explicit request. The caller must maintain the recipient owner's high-water
mark across reconnects and reserve each serial before use. Rust returns its
owned Opened value; C exclusively borrows an immovable opening and restores it
on finish. C's driver and metadata scratch allocate nothing. Rust uses inline
metadata storage but its transport still allocates replies.

Independent socket tests cover exact bytes, one-byte dispatch, fragmented
status, repeated explicit requests, refusal, short writes, disconnect, wrong
status identity, terminal-status clunk refusal, small iounit, directory replies,
unsupported capability, old serials, expiry and cancellation. The fifteen Rust
client tests (including the added request cases), C request test and strict
Clippy pass.

Both peers pass Sophia's production request-to-frame gate: real namespace
authentication, SDK request submission, coordinator settlement, frame metadata
and every pixel verified, then root-only ownership. All four external portal
gates pass independently in C and Rust. The fixture supplies policy and pixels;
this is neither native capture acceptance nor measured performance evidence.
Production request refusal, cancellation/revocation and reconnect audits remain,
alongside transfer/watch, trusted requester and release work.

Both peers now pass production capture refusal and lock-cancellation gates.
Locked admission returns EACCES, an occupied queue EAGAIN, and an old serial
ESTALE; each SDK records Refused with the exact errno and no outcome. Advancing
the execution fence cancels queued work and work already taken by its owner.
Both SDKs report Cancelled after a verified status snapshot and retain only the
root. Unlock does not revive the old work: the fixture rejects its late Ready
completion and confirms no frame was installed.

All nine external portal gates pass independently in both languages; strict
Rust Clippy also passes. These checks use the real request queue and export
with supplied policy. They do not qualify native scheduling, local cancellation
after wire admission, reconnect recovery, or the remaining transfer/watch APIs.

The snapshot operation driver now supports opaque retained transfers, capped at
65,536 bytes. Rust `Fetch::transfer` borrows a mutable caller slice; C uses
`SOPHIA_PS_TRANSFER` with its existing borrowed storage. Both require at least
64 KiB, leave unused bytes untouched and allocate no payload buffer. Rust's
Fetch now carries a storage lifetime; metadata constructors still store their
small records inline. Transport reply allocation is unchanged.

The driver reads bounded chunks on one retained fid, probes EOF at the limit
and exposes bytes only after successful clunk. It does not decode payloads or
infer a relationship to a separately walked watch fid. Independent tests pass
empty, one-byte and exact-limit payloads, fragmented replies, undersized storage,
overflow, stale reads and clunk refusal. All seventeen Rust client tests,
C transfer/snapshot/request tests and strict Clippy pass. Rebuilt peers pass
the nine existing production gates as regression checks; these do not exercise
the new transfer path. Transfer transport composition and production delivery,
watch and the remaining recovery audits still need implementation.

Rust `transfer::Transfer` and C `sophia_pt_*` now drive the retained transfer
reader on an exclusive authenticated root. Both borrow caller payload storage,
use bounded dispatch and one absolute deadline, and return the same role after
EOF/clunk without copying the payload. Rust drops its owned role on failure;
C callers close their borrowed fd. C borrows an immovable opening exclusively
and restores it on successful finish. Scratch progress is not a completed result.

Both independent peers pass production delivery, absent-object and mid-read
revocation gates. Delivery verifies the original buffer pointer, unchanged
unused bytes and root-only ownership. Absence preserves ENOENT. Revocation
after payload receipt but before completion returns ESTALE without exposing a
result; buffered bytes are not claimed to be recalled. All twelve external
portal gates, seventeen Rust component tests and strict Clippy pass. The
production payload is seven supplied bytes; the 64 KiB boundary remains
component evidence. Watch, transfer recovery/local cancellation and release
qualification remain open.

Rust `watch::Watch` and C `sophia_pw_*` now retain a watch fid and issue one
nonempty offset-zero read. They neither poll the file repeatedly nor reopen it
automatically. ESTALE on the retained object is followed by clunk; only then is
revocation reported and the original root returned. A separately fetched
transfer is not assumed to share the watch's object identity.

Local cancellation closes Rust's owned role; C marks the operation closed and
requires its caller to close the borrowed fd. Expiry follows the same failure
path. Neither reports revocation or preserves the connection through a flush.
Each driver owns the role exclusively, so composing concurrent watch and other
recipient operations remains a session-integration gap.

Both peers pass production lock, replacement, source-generation, absence,
local-cancellation and deadline cases. Revocation returns root-only ownership;
absence preserves ENOENT; cancellation/expiry remain distinct from revocation.
All eighteen external portal gates and strict Clippy pass. These do not measure
idle cost or qualify malformed watch replies, clunk refusal, flush-based
cancellation or reconnect recovery; those audits and release work remain.
