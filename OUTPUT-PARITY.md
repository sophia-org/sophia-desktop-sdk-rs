# Output API parity

Revision 2, 2026-10-16. Contract: API-1 output files in `spec/`.
C reference: `a88efe7`, `src/sophia_output_session.h`.
Rust has codecs, provisional fixed-file startup and an immutable submission
lane; it is not yet a usable output session client.

| C capability | Rust status | Evidence or remaining work |
| --- | --- | --- |
| File record codecs | `sophia-output-protocol` | Thirty extracted tests; schema and contract match C. |
| Direct startup | `Opening::direct` on a versioned pipeline | Seven startup tests cover fragmented files and bounds. Integrate the nonblocking version handshake in the final facade. |
| Authenticated handoff | `Opening::attached` | Exact sole root on a quiescent owned pipeline; no reset or attach. Supplied socket tests only so far. |
| API / Limits | Validated through EOF and clunk | Exact API line and 72-byte Limits object; retain epoch and resource bounds. |
| Role files | Opens events, submit and ACK | Refuses non-file Qids and control iounits smaller than 24/16 bytes. |
| Observe prerequisite | Required before opening | Unsupported revision requests survive startup for server negotiation/refusal. |
| Startup progress | Caller clock, deadline, interests and byte budget | Expiry sends nothing; clock regression is nonmutating. Constructors queue requests without socket I/O. |
| Negotiation and Refused state | Missing | Intersect requested grants, require Observe, acknowledge refusal before terminal handoff. |
| Proposal admission and custody | Provisional `Submission` | Configure/epoch checks, immutable bytes, 64-ticket history, same-ID retry, domain allocation and advertised assembly deadline. Nine driver tests; session integration remains. |
| Ordered events and ACKs | Missing | Internal Negotiated/Refused/Submitted; retained application heads and ACK-progress deadlines. |
| Authoritative topology | Missing | Fetch the exact ObjectPublished Qid/epoch/topology identity through EOF and clunk before exposing or ACKing it. |
| Close, revocation and recovery | Startup close and submission disconnect | Submission retains confirmed custody and distinguishes uncertain from unsent work. Session revocation/recovery tests remain. |
| Production export / protected launch | Missing | Paired independent clients must pass before pin adoption. |

C borrows its descriptor and caller storage; Rust owns the pipeline and lends
its descriptor for polling only. Both require exclusive control of the admitted
root. The developing Rust driver requires msize 4096–65536, eight request slots,
sixteen fids and input space for two negotiated frames plus scalar replies.

Output semantics are distinct from WM: explicit negotiation refusal, Observe
required, Configure granted by the server, publication-bound topology reads,
ACK progress and per-epoch domain replay capacity. Do not substitute a WM event
or snapshot policy for those rules. Codec success is not custody, topology
commit, presentation or API parity.

Submission tests cover both Submitted/reply orders, the ACK/clunk barrier,
retry progress and backoff, immutable negotiation, grant/epoch rejection and
caller/assembly deadlines. Output permits out-of-order domain IDs and treats
EALREADY as refusal, without WM’s success exception. Independent C socket tests
exercise both rules through direct and attached startup; C implementation is
unchanged. Rust submission tests supply matched replies directly. They do not
yet prove socket dispatch, negotiation Refused handling or replay exhaustion.
