//! Linux prerequisites for proposed Sophia x6jp929h, not a descriptor SDK.
//! No protocol admission, journal custody or GPU completion is simulated here.
#![cfg(target_os = "linux")]

use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{self, MemfdFlags, SealFlags};
use rustix::io::{self, Errno, FdFlags};
use rustix::net::*;
use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;

fn required() -> SealFlags {
    SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL
}
fn source(seals: SealFlags) -> OwnedFd {
    let fd = fs::memfd_create(
        "t337-fixture",
        MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
    )
    .unwrap();
    fs::ftruncate(&fd, 8).unwrap();
    assert_eq!(io::write(&fd, b"12345678").unwrap(), 8);
    fs::fcntl_add_seals(&fd, seals).unwrap();
    fd
}
fn pair() -> (OwnedFd, OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}
fn send_fds(socket: &OwnedFd, fds: &[OwnedFd]) -> Result<usize, Errno> {
    let borrowed: Vec<_> = fds.iter().map(AsFd::as_fd).collect();
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !fds.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&borrowed)));
    }
    sendmsg(
        socket,
        &[IoSlice::new(b"source")],
        &mut ancillary,
        SendFlags::NOSIGNAL,
    )
}
fn receive(socket: &OwnedFd, control_bytes: usize) -> (ReturnFlags, Vec<OwnedFd>) {
    let mut data = [0; 6];
    let mut space = vec![MaybeUninit::uninit(); control_bytes];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let result = recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut data)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(result.bytes, 6);
    assert_eq!(&data, b"source");
    let mut fds = Vec::new();
    for message in ancillary.drain() {
        if let RecvAncillaryMessage::ScmRights(rights) = message {
            fds.extend(rights);
        } else {
            panic!("unexpected ancillary type");
        }
    }
    (result.flags, fds)
}
fn fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

#[test]
fn real_seals_rights_truncation_and_backpressure() {
    // One test keeps /proc FD accounting independent of test-runner concurrency.
    let before = fd_count();
    for missing in [
        SealFlags::WRITE,
        SealFlags::GROW,
        SealFlags::SHRINK,
        SealFlags::SEAL,
    ] {
        let fd = source(required() & !missing);
        assert!(!fs::fcntl_get_seals(&fd).unwrap().contains(required()));
    }
    let future =
        source(SealFlags::FUTURE_WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL);
    assert!(
        !fs::fcntl_get_seals(&future)
            .unwrap()
            .contains(SealFlags::WRITE)
    );
    drop(future);
    {
        let (sender, receiver) = pair();
        let fds = vec![source(required())];
        assert_eq!(send_fds(&sender, &fds).unwrap(), 6);
        drop(fds); // The receiver's reference must survive producer close.
        let (flags, received) = receive(&receiver, rustix::cmsg_space!(ScmRights(4)));
        assert!(!flags.contains(ReturnFlags::CTRUNC));
        assert_eq!(received.len(), 1);
        let fd = &received[0];
        assert!(io::fcntl_getfd(fd).unwrap().contains(FdFlags::CLOEXEC));
        assert!(fs::fcntl_get_seals(fd).unwrap().contains(required()));
        assert_eq!(fs::fstat(fd).unwrap().st_size, 8);
        let mut bytes = [0; 8];
        assert_eq!(io::pread(fd, &mut bytes, 0).unwrap(), 8);
        assert_eq!(&bytes, b"12345678");
        assert_eq!(io::pwrite(fd, b"X", 0), Err(Errno::PERM));
        assert_eq!(fs::ftruncate(fd, 4), Err(Errno::PERM));
        assert_eq!(fs::ftruncate(fd, 16), Err(Errno::PERM));
    }
    for count in [0, 2, 4] {
        let (sender, receiver) = pair();
        let fds: Vec<_> = (0..count).map(|_| source(required())).collect();
        send_fds(&sender, &fds).unwrap();
        // Truncation with no control storage also exercises kernel FD cleanup.
        let (flags, received) = receive(
            &receiver,
            if count == 4 {
                0
            } else {
                rustix::cmsg_space!(ScmRights(4))
            },
        );
        if count == 4 {
            assert!(flags.contains(ReturnFlags::CTRUNC));
        } else {
            assert_eq!(received.len(), count);
            assert_ne!(received.len(), 1); // The draft expects exactly one.
        }
        drop(received); // Refusal closes every delivered reference.
    }
    {
        let (sender, receiver) = pair();
        let fds = vec![source(required())];
        let mut full = false;
        for _ in 0..65536 {
            match send_fds(&sender, &fds) {
                Ok(6) => {}
                Err(Errno::AGAIN) => {
                    full = true;
                    break;
                }
                other => panic!("unexpected send result: {other:?}"),
            }
        }
        assert!(full, "bounded loop must reach socket backpressure");
        assert_eq!(fs::fstat(&fds[0]).unwrap().st_size, 8);
        drop(receiver); // Teardown releases rights still queued in the kernel.
    }
    assert_eq!(fd_count(), before, "no received or queued reference leaked");
}
