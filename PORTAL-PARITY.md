# Portal SDK parity

Revision 3, 2026-10-16. Recipient codecs are developing; there is no complete
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
