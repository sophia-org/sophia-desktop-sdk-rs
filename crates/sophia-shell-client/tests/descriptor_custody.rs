//! Sealed-source lifecycle peer for proposed x6jp929h at 0f3289c2e.
//! Control admission/events and worker completion are explicit fixture calls,
//! not a production 9P endpoint, SDK API, worker, authentication or GPU proof.
#![cfg(target_os = "linux")]

use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{self, MemfdFlags, SealFlags};
use rustix::io;
use rustix::net::*;
use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;

#[path = "support/descriptor_producer_peer.rs"]
mod producer;

fn seals() -> SealFlags {
    SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL
}
fn source() -> OwnedFd {
    source_with(&[1, 2, 3, 255, 4, 5, 6, 255], seals())
}
fn source_with(bytes: &[u8], required: SealFlags) -> OwnedFd {
    let fd = fs::memfd_create(
        "custody-peer",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    assert_eq!(io::write(&fd, bytes).unwrap(), bytes.len());
    fs::fcntl_add_seals(&fd, required).unwrap();
    fd
}
fn packet(role: u16, id: u64) -> [u8; 200] {
    let mut b = [0; 200];
    for (at, value) in [
        (0, 200u32),
        (96, 1),
        (100, 1),
        (104, 2),
        (108, 1),
        (112, 8),
        (144, 8),
    ] {
        b[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(4, 1u16), (6, 2), (8, role), (10, 1), (116, 1), (118, 1)] {
        b[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [
        (16, 1u64),
        (24, 2),
        (32, id),
        (40, id),
        (48, 4),
        (56, 5),
        (64, 6),
        (72, 7),
        (80, 8),
        (88, 9),
        (120, 8),
    ] {
        b[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    b
}
fn sockets() -> (OwnedFd, OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}
fn send(socket: &OwnedFd, b: &[u8], fd: &OwnedFd) {
    assert_eq!(try_send(socket, b, fd).unwrap(), b.len());
}
fn try_send(socket: &OwnedFd, b: &[u8], fd: &OwnedFd) -> Result<usize, io::Errno> {
    try_send_many(socket, b, &[fd])
}
fn try_send_many(socket: &OwnedFd, b: &[u8], fds: &[&OwnedFd]) -> Result<usize, io::Errno> {
    let refs: Vec<_> = fds.iter().map(|fd| fd.as_fd()).collect();
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !refs.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&refs)));
    }
    sendmsg(
        socket,
        &[IoSlice::new(b)],
        &mut ancillary,
        SendFlags::NOSIGNAL,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Admitted,
    Received,
    Validating,
    Accepted,
}
struct Offer {
    id: u64,
    phase: Phase,
    fd: Option<OwnedFd>,
    cancelled: bool,
    retired: bool,
    leases: usize,
    deadline: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Terminal {
    id: u64,
    accepted: bool,
    disposition: u32,
}
struct Owner {
    role: u16,
    ready: bool,
    live_journal: bool,
    high: u64,
    offers: Vec<Offer>,
    events: Vec<Terminal>,
    reserved: usize,
    capacity: usize,
    backing_bytes: usize,
    faults: usize,
    disabled: bool,
}
impl Owner {
    fn new(role: u16) -> Self {
        Self {
            role,
            ready: false,
            live_journal: true,
            high: 0,
            offers: Vec::new(),
            events: Vec::new(),
            reserved: 0,
            capacity: 9,
            backing_bytes: 32,
            faults: 0,
            disabled: false,
        }
    }
    fn admit(&mut self, id: u64) -> bool {
        // Two terminal records per offer plus one permanently reserved Closed
        // slot; ordinary admission/Ready records are outside this ledger.
        if !self.ready
            || self.disabled
            || !self.live_journal
            || id <= self.high
            || self
                .offers
                .iter()
                .filter(|o| o.phase != Phase::Accepted)
                .count()
                == 4
            || (self.offers.len() + 1) * 8 > self.backing_bytes
            || self.reserved + 2 * self.events.len() + 2 + 1 > self.capacity
        {
            return false;
        }
        self.high = id;
        self.reserved += 2;
        self.offers.push(Offer {
            id,
            phase: Phase::Admitted,
            fd: None,
            cancelled: false,
            retired: false,
            leases: 0,
            deadline: 2000,
        });
        true
    }
    fn terminal(&mut self, index: usize, accepted: bool, disposition: u32) {
        let id = self.offers[index].id;
        self.reserved -= 2;
        if self.live_journal {
            self.events.push(Terminal {
                id,
                accepted,
                disposition,
            });
        }
        if accepted {
            self.offers[index].phase = Phase::Accepted;
        } else {
            self.offers.remove(index);
        }
    }
    fn cancel(&mut self, id: u64) {
        if let Some(i) = self.offers.iter().position(|o| o.id == id) {
            match self.offers[i].phase {
                Phase::Validating => self.offers[i].cancelled = true,
                Phase::Accepted => {
                    self.offers[i].retired = true;
                    self.collect();
                }
                _ => self.terminal(i, false, 3),
            }
        }
    }
    fn collect(&mut self) {
        self.offers
            .retain(|o| !(o.phase == Phase::Accepted && o.retired && o.leases == 0));
    }
    fn expire(&mut self, now: u64) {
        let ids: Vec<_> = self
            .offers
            .iter()
            .filter(|o| o.phase != Phase::Accepted && now >= o.deadline)
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }
    fn close_channel(&mut self) {
        self.ready = false;
        self.disabled = true;
        let ids: Vec<_> = self
            .offers
            .iter()
            .filter(|o| o.phase != Phase::Accepted)
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }
    fn end_epoch(&mut self) {
        self.live_journal = false;
        self.close_channel();
        for o in &mut self.offers {
            if o.phase == Phase::Accepted {
                o.retired = true;
            }
        }
        self.collect();
    }
    fn receive(&mut self, socket: &OwnedFd) {
        let mut b = [0; 200];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let result = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut b)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        let mut fds = Vec::new();
        for msg in ancillary.drain() {
            if let RecvAncillaryMessage::ScmRights(rights) = msg {
                fds.extend(rights);
            } else {
                panic!("unexpected ancillary message");
            }
        }
        let id = u64::from_le_bytes(b[32..40].try_into().unwrap());
        if !self.ready {
            return;
        }
        if result.bytes != 200
            || result
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || b != packet(self.role, id)
            || fds.len() != 1
            || id > self.high
        {
            self.faults += 1;
            self.close_channel();
            return;
        }
        let Some(i) = self.offers.iter().position(|o| o.id == id) else {
            return;
        };
        if self.offers[i].phase != Phase::Admitted {
            self.faults += 1;
            self.close_channel();
            return;
        }
        let fd = fds.pop().unwrap();
        if !fs::fcntl_get_seals(&fd).is_ok_and(|s| s.contains(seals()))
            || fs::fstat(&fd).unwrap().st_size != 8
        {
            self.terminal(i, false, 3);
            return;
        }
        assert!(io::fcntl_getfd(&fd).unwrap().contains(io::FdFlags::CLOEXEC));
        self.offers[i].fd = Some(fd);
        self.offers[i].phase = Phase::Received;
    }
    fn start_worker(&mut self, id: u64) {
        let o = self.offers.iter_mut().find(|o| o.id == id).unwrap();
        assert_eq!(o.phase, Phase::Received);
        o.phase = Phase::Validating;
    }
    fn worker_returns(&mut self, id: u64) {
        let i = self.offers.iter().position(|o| o.id == id).unwrap();
        let o = &self.offers[i];
        assert_eq!(o.phase, Phase::Validating);
        let mut pixels = [0; 8];
        assert_eq!(
            io::pread(o.fd.as_ref().unwrap(), &mut pixels, 0).unwrap(),
            8
        );
        let valid = pixels
            .chunks_exact(4)
            .all(|p| p[..3].iter().all(|c| *c <= p[3]));
        let accepted = valid && !o.cancelled && self.live_journal;
        self.terminal(i, accepted, if accepted { 1 } else { 3 });
    }
}

#[test]
fn sealed_lifecycle_races_keep_custody_and_reservations() {
    let baseline = std::fs::read_dir("/proc/self/fd").unwrap().count();
    for role in [1, 2] {
        for timing in 0..4 {
            let (tx, rx) = sockets();
            let mut owner = Owner::new(role);
            assert!(!owner.admit(3)); // No Begin before Ready.
            owner.ready = true;
            assert!(owner.admit(3));
            assert!(!owner.admit(3));
            let fd = source();
            send(&tx, &packet(role, 3), &fd);
            drop(fd); // Accepted send is not yet accepted content.
            assert!(owner.events.is_empty());
            if timing == 0 {
                owner.cancel(3);
            }
            owner.receive(&rx); // Also drains a packet queued before expiry.
            if timing == 1 {
                owner.cancel(3);
            }
            if timing >= 2 {
                owner.start_worker(3);
                if timing == 2 {
                    owner.cancel(3);
                    assert!(owner.events.is_empty());
                    assert!(owner.offers[0].fd.is_some()); // Worker still owns its reference.
                }
                owner.worker_returns(3);
            }
            assert_eq!(
                owner.events,
                [Terminal {
                    id: 3,
                    accepted: timing == 3,
                    disposition: if timing == 3 { 1 } else { 3 }
                }]
            );
            assert_eq!(owner.reserved, 0);
            owner.cancel(3); // Repeated cancellation cannot emit a second release.
            assert_eq!(owner.events.len(), 1);
            assert!(owner.offers.is_empty());
            // Expired and skipped lower IDs are drained; no journal amplification.
            for id in [2, 3] {
                let fd = source();
                send(&tx, &packet(role, id), &fd);
                owner.receive(&rx);
            }
            assert_eq!(owner.events.len(), 1);
            assert_eq!(owner.faults, 0);
        }
        let (tx, rx) = sockets();
        let mut owner = Owner::new(role);
        owner.ready = true;
        for id in 1..=4 {
            assert!(owner.admit(id));
        }
        assert!(!owner.admit(5));
        assert_eq!(owner.high, 4);
        owner.cancel(1);
        assert!(!owner.admit(5)); // Freed slot cannot bypass unacknowledged terminal records.
        owner.events.clear(); // Explicit terminal-journal consumption in this peer.
        assert!(owner.admit(5));
        let fd = source();
        send(&tx, &packet(role, 6), &fd); // Never admitted, above the high-water mark.
        owner.receive(&rx);
        assert_eq!(owner.faults, 1);
        assert!(!owner.ready);
        assert!(!owner.events.iter().any(|e| e.id == 6));
        assert_eq!(owner.reserved, 0);
    }
    journal_loss_and_retirement_do_not_invent_release();
    producer::exercise();
    refusal_expiry_and_duplicate_packets();
    assert_eq!(
        std::fs::read_dir("/proc/self/fd").unwrap().count(),
        baseline
    );
}

fn refusal_expiry_and_duplicate_packets() {
    for role in [1, 2] {
        for case in 0..8 {
            let (tx, rx) = sockets();
            let mut owner = Owner::new(role);
            owner.ready = true;
            assert!(owner.admit(1));
            let fd = match case {
                0 => source_with(&[0; 7], seals()),
                1 => source_with(&[0; 9], seals()),
                2 => source_with(&[0; 8], seals() & !SealFlags::WRITE),
                3 => source_with(&[0; 8], seals() & !SealFlags::GROW),
                4 => source_with(&[0; 8], seals() & !SealFlags::SHRINK),
                5 => source_with(&[0; 8], seals() & !SealFlags::SEAL),
                6 => std::fs::File::open("/dev/null").unwrap().into(),
                _ => source_with(&[1, 2, 3, 0, 0, 0, 0, 0], seals()),
            };
            send(&tx, &packet(role, 1), &fd);
            drop(fd);
            owner.receive(&rx);
            if case == 7 {
                owner.start_worker(1);
                owner.worker_returns(1);
            }
            assert_eq!(
                owner.events,
                [Terminal {
                    id: 1,
                    accepted: false,
                    disposition: 3
                }]
            );
            assert!(owner.offers.is_empty());
            assert_eq!(owner.faults, 0);
        }
        for count in [0, 2, 4] {
            let (tx, rx) = sockets();
            let mut owner = Owner::new(role);
            owner.ready = true;
            assert!(owner.admit(1));
            let fds: Vec<_> = (0..count).map(|_| source()).collect();
            let refs: Vec<_> = fds.iter().collect();
            assert_eq!(try_send_many(&tx, &packet(role, 1), &refs).unwrap(), 200);
            drop(fds);
            owner.receive(&rx);
            assert_eq!(owner.faults, 1);
            assert!(owner.offers.is_empty());
        }
        for case in 0..6 {
            let (tx, rx) = sockets();
            let mut owner = Owner::new(role);
            owner.ready = true;
            assert!(owner.admit(3));
            let fd = source();
            let mut b = packet(role, 3);
            match case {
                0 => b[12] = 1, // Malformed header must close its received right.
                1 => b[16] = 9, // Foreign epoch cannot attach to a live offer.
                2 => {
                    owner.expire(1999);
                    assert_eq!(owner.offers.len(), 1);
                    owner.expire(2000);
                }
                3 => owner.close_channel(),
                4 => owner.end_epoch(),
                _ => (),
            }
            send(&tx, &b, &fd);
            owner.receive(&rx);
            if case == 5 {
                assert_eq!(owner.offers[0].phase, Phase::Received);
                send(&tx, &b, &fd);
                owner.receive(&rx); // Duplicate faults, never replaces the first reference.
                assert_eq!(owner.faults, 1);
            }
            assert!(owner.offers.is_empty());
            assert_eq!(owner.reserved, 0);
            assert_eq!(owner.events.len(), usize::from(case != 4));
            if case <= 1 {
                assert_eq!(owner.faults, 1);
            }
        }
        let (tx, rx) = sockets();
        let mut owner = Owner::new(role);
        owner.ready = true;
        assert!(owner.admit(1));
        let fd = source();
        send(&tx, &packet(role, 1), &fd);
        owner.receive(&rx);
        owner.start_worker(1);
        owner.expire(2000);
        assert!(owner.events.is_empty());
        assert_eq!(owner.reserved, 2);
        owner.worker_returns(1);
        assert_eq!(
            owner.events,
            [Terminal {
                id: 1,
                accepted: false,
                disposition: 3
            }]
        );
    }
}

fn journal_loss_and_retirement_do_not_invent_release() {
    for role in [1, 2] {
        let (tx, rx) = sockets();
        let mut owner = Owner::new(role);
        owner.ready = true;
        assert!(owner.admit(1));
        let fd = source();
        send(&tx, &packet(role, 1), &fd);
        owner.receive(&rx);
        owner.start_worker(1);
        owner.end_epoch();
        assert!(owner.events.is_empty());
        assert_eq!(owner.reserved, 2);
        assert!(owner.offers[0].fd.is_some());
        owner.worker_returns(1);
        assert!(owner.events.is_empty()); // Producer still has unknown custody, not release.
        assert!(owner.offers.is_empty());
        assert_eq!(owner.reserved, 0);

        let mut owner = Owner::new(role);
        owner.ready = true;
        assert!(owner.admit(2));
        send(&tx, &packet(role, 2), &fd);
        owner.receive(&rx);
        owner.start_worker(2);
        owner.worker_returns(2);
        owner.offers[0].leases = 1;
        owner.cancel(2);
        assert_eq!(owner.offers.len(), 1);
        assert_eq!(io::pwrite(&fd, b"X", 0), Err(io::Errno::PERM));
        owner.offers[0].leases = 0;
        owner.collect();
        assert!(owner.offers.is_empty());
        assert_eq!(owner.events.len(), 1);
    }
}
