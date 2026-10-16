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
    let opened = opening.into_opened().unwrap();
    assert_eq!(opened.info.request, RequestCapability::Unavailable);
    assert!(opened.info.epoch > 0 && opened.pipeline.fid_count() == 1);
    assert!(opened.pipeline.is_quiescent());
    std::fs::write(
        std::env::var_os("SOPHIA_TEST_LOCK_REPORT").unwrap(),
        b"started\n",
    )
    .unwrap();
}
