//! Protected namespace startup peer; the fixture preserves the launch channel on stdin.
use sophia_portal_client::{protocol::RequestCapability, startup::Opening};
use std::{
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};
fn main() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let stdin = std::io::stdin();
    rustix::io::fcntl_setfd(&stdin, rustix::io::FdFlags::CLOEXEC).unwrap();
    let relay = UnixStream::from(rustix::io::fcntl_dupfd_cloexec(&stdin, 3).unwrap());
    let directory = std::env::var_os("SOPHIA_NAMESPACE_DIR").unwrap();
    let names = sophia_namespace_client::Namespace::open(std::path::Path::new(&directory)).unwrap();
    let mut auth = sophia_launch_client::Client::over(
        names.connect("roles/portal").unwrap(),
        relay,
        b"namespace-client",
        b"portal-client",
    )
    .unwrap();
    while !auth.ready() {
        assert!(Instant::now() < deadline);
        auth.poll().unwrap();
        if !auth.ready() {
            auth.wait_for_io(Duration::from_millis(10)).unwrap();
        }
    }
    let admitted = auth.into_authenticated().unwrap();
    let _relay = admitted.relay;
    let start = Instant::now();
    let mut opening = Opening::from_attached(admitted.role, admitted.root, 0, 2000).unwrap();
    while !opening.is_opened() {
        assert!(Instant::now() < deadline);
        let interest = opening.io_interest().unwrap();
        let mut flags = rustix::event::PollFlags::IN;
        if interest.writable {
            flags |= rustix::event::PollFlags::OUT;
        }
        let fd = opening.fd().unwrap();
        let mut fds = [rustix::event::PollFd::new(&fd, flags)];
        let now = start.elapsed().as_millis() as u64;
        let wait = Duration::from_millis(if interest.immediate {
            0
        } else {
            opening.deadline().unwrap().saturating_sub(now)
        });
        rustix::event::poll(
            &mut fds,
            Some(&rustix::event::Timespec::try_from(wait).unwrap()),
        )
        .unwrap();
        opening
            .dispatch(start.elapsed().as_millis() as u64, 65536)
            .unwrap();
    }
    let mut opened = opening.into_opened().unwrap();
    let mode = std::env::var("SOPHIA_TEST_PORTAL_BOUNDARY").unwrap();
    let request_mode = mode.starts_with("sdk-request");
    let refused = matches!(
        mode.as_str(),
        "sdk-request-refused" | "sdk-request-busy" | "sdk-request-stale"
    );
    let refusal_errno = match mode.as_str() {
        "sdk-request-busy" => 11,
        "sdk-request-stale" => 116,
        _ => 13,
    };
    let cancelled = mode.starts_with("sdk-request-cancelled");
    let frame_mode = mode.starts_with("sdk-frame") || request_mode;
    let revoke = mode == "sdk-frame-revoke";
    assert_eq!(
        opened.info.request,
        if frame_mode {
            RequestCapability::OutputCaptureV1
        } else {
            RequestCapability::Unavailable
        }
    );
    assert!(opened.info.epoch > 0 && opened.pipeline.fid_count() == 1);
    assert!(opened.pipeline.is_quiescent());
    if mode.starts_with("sdk-transfer") {
        let absent = mode == "sdk-transfer-absent";
        let revoked = mode == "sdk-transfer-revoke";
        let mut storage = vec![0xa5; 65536];
        let pointer = storage.as_ptr();
        let now = start.elapsed().as_millis() as u64;
        let mut transfer =
            sophia_portal_client::transfer::Transfer::new(opened, &mut storage, now, now + 3000)
                .unwrap();
        let mut paused = false;
        while transfer.result().is_none() && transfer.error().is_none() {
            assert!(Instant::now() < deadline);
            let interest = transfer.io_interest().unwrap();
            let fd = transfer.fd().unwrap();
            let mut flags = rustix::event::PollFlags::IN;
            if interest.writable {
                flags |= rustix::event::PollFlags::OUT;
            }
            let mut fds = [rustix::event::PollFd::new(&fd, flags)];
            let wait = Duration::from_millis(if interest.immediate {
                0
            } else {
                transfer
                    .deadline()
                    .unwrap()
                    .saturating_sub(start.elapsed().as_millis() as u64)
            });
            rustix::event::poll(
                &mut fds,
                Some(&rustix::event::Timespec::try_from(wait).unwrap()),
            )
            .unwrap();
            let result = transfer.dispatch(
                start.elapsed().as_millis() as u64,
                if revoked && !paused { 1 } else { 65536 },
            );
            if !absent && !revoked {
                result.unwrap();
            }
            if revoked && !paused && transfer.received() > 0 {
                assert!(transfer.result().is_none());
                let report = std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap();
                std::fs::write(&report, b"transfer-partial\n").unwrap();
                while std::fs::read_to_string(&report).unwrap() != "continue\n" {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                paused = true;
            }
        }
        if absent || revoked {
            assert!(transfer.result().is_none() && transfer.fd().is_none());
            assert_eq!(
                transfer.error(),
                Some(&sophia_portal_client::transfer::Error::Snapshot(
                    sophia_portal_client::snapshot::Error::Remote(if absent { 2 } else { 116 })
                ))
            );
            assert!(!revoked || paused);
        } else {
            let (role, bytes) = transfer.finish().unwrap();
            assert_eq!(bytes, b"granted");
            assert_eq!(bytes.as_ptr(), pointer);
            assert_eq!(role.pipeline.fid_count(), 1);
            assert!(role.pipeline.is_quiescent());
            assert!(storage[7..].iter().all(|b| *b == 0xa5));
        }
        std::fs::write(
            std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
            b"transfer-done\n",
        )
        .unwrap();
        return;
    }
    if request_mode {
        let now = start.elapsed().as_millis() as u64;
        let mut request = sophia_portal_client::capture_request::Request::new(
            opened,
            sophia_portal_client::protocol::CaptureRequest {
                serial: if mode == "sdk-request-busy" { 2 } else { 1 },
                output: 7,
            },
            0,
            now,
            now + 3000,
        )
        .unwrap();
        while request.outcome().is_none() && request.error().is_none() {
            assert!(Instant::now() < deadline);
            let interest = request.io_interest().unwrap();
            let fd = request.fd().unwrap();
            let mut flags = rustix::event::PollFlags::IN;
            if interest.writable {
                flags |= rustix::event::PollFlags::OUT;
            }
            let mut fds = [rustix::event::PollFd::new(&fd, flags)];
            let wait = Duration::from_millis(if interest.immediate {
                0
            } else {
                request
                    .next_wake()
                    .unwrap()
                    .saturating_sub(start.elapsed().as_millis() as u64)
            });
            rustix::event::poll(
                &mut fds,
                Some(&rustix::event::Timespec::try_from(wait).unwrap()),
            )
            .unwrap();
            let result = request.dispatch(start.elapsed().as_millis() as u64, 65536);
            if !refused {
                result.unwrap();
            }
        }
        if refused {
            assert_eq!(
                request.custody(),
                sophia_portal_client::capture::Custody::Refused
            );
            assert_eq!(
                request.error(),
                Some(&sophia_portal_client::capture_request::Error::Capture(
                    sophia_portal_client::capture::Error::Refused(refusal_errno)
                ))
            );
            assert!(request.outcome().is_none() && request.fd().is_none());
            std::fs::write(
                std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
                b"request-refused\n",
            )
            .unwrap();
            return;
        }
        let (role, outcome, serial) = request.finish().unwrap();
        assert_eq!(
            outcome,
            if cancelled {
                sophia_portal_client::protocol::CaptureState::Cancelled
            } else {
                sophia_portal_client::protocol::CaptureState::Ready
            }
        );
        assert_eq!(serial, 1);
        assert_eq!(role.pipeline.fid_count(), 1);
        if cancelled {
            assert!(role.pipeline.is_quiescent());
            std::fs::write(
                std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
                b"request-cancelled\n",
            )
            .unwrap();
            return;
        }
        opened = role;
    }
    if frame_mode {
        let now = start.elapsed().as_millis() as u64;
        let mut frame = sophia_portal_client::frame::Frame::new(
            opened,
            Vec::with_capacity(256 * 128 * 4),
            now,
            now + 3000,
        )
        .unwrap();
        let mut paused = false;
        while frame.result().is_none() && frame.error().is_none() {
            assert!(Instant::now() < deadline);
            let interest = frame.io_interest().unwrap();
            let fd = frame.fd().unwrap();
            let mut flags = rustix::event::PollFlags::IN;
            if interest.writable {
                flags |= rustix::event::PollFlags::OUT;
            }
            let mut fds = [rustix::event::PollFd::new(&fd, flags)];
            let wait = Duration::from_millis(if interest.immediate {
                0
            } else {
                frame
                    .deadline()
                    .unwrap()
                    .saturating_sub(start.elapsed().as_millis() as u64)
            });
            rustix::event::poll(
                &mut fds,
                Some(&rustix::event::Timespec::try_from(wait).unwrap()),
            )
            .unwrap();
            let result = frame.dispatch(
                start.elapsed().as_millis() as u64,
                if revoke && !paused { 1 } else { 65536 },
            );
            if !revoke {
                result.unwrap();
            }
            if revoke && !paused && frame.received() > 0 {
                assert!(frame.received() < 256 * 128 * 4 && frame.result().is_none());
                let report = std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap();
                std::fs::write(&report, b"partial\n").unwrap();
                while std::fs::read_to_string(&report).unwrap() != "continue\n" {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                paused = true;
            }
        }
        if revoke {
            assert!(paused && frame.result().is_none() && frame.fd().is_none());
            assert!(!matches!(
                frame.error(),
                Some(sophia_portal_client::frame::Error::Deadline)
            ));
            std::fs::write(
                std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
                b"revoked\n",
            )
            .unwrap();
            return;
        }
        let (opened, info, pixels) = frame.finish().unwrap();
        assert_eq!(
            (
                info.output,
                info.output_generation,
                info.presentation_generation
            ),
            (7, 9, 11)
        );
        assert_eq!((info.width, info.height), (256, 128));
        assert_eq!(pixels.len(), 256 * 128 * 4);
        assert!(pixels.iter().all(|b| *b == 17));
        assert_eq!(opened.pipeline.fid_count(), 1);
    }
    std::fs::write(
        std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
        if frame_mode {
            b"frame\n".as_slice()
        } else {
            b"started\n"
        },
    )
    .unwrap();
}
