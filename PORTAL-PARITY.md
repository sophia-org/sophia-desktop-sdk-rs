# Portal SDK parity

Revision 1, 2026-10-16. Recipient codecs are developing; there is no complete
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

Remaining recipient work includes discovery/status, capture request/status
records, authenticated startup, retained-object frame and transfer reads,
watch/cancellation, custody, revocation and external-loop APIs. Reads should
use caller-controlled storage and bounded chunks; do not create another full
image copy. Decoded metadata alone never authorizes pixels or proves presentation.
Production gates and foreign-language bindings remain required.

The trusted portal-requester is a separate authority and contract; no recipient
API may approve transfers or claim executor custody. Requester coverage and the
broker migration prerequisite remain explicit t333 work.
