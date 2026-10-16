use sophia_9p_client::pipeline::{Pipeline, PipelineLimits};
use sophia_portal_client::{
    session::{Error, Session, WatchEvent},
    snapshot::File,
    startup::{Opened, Opening},
};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

fn send(s: &mut UnixStream, kind: u8, tag: &[u8], body: &[u8]) {
    let mut reply = ((body.len() + 7) as u32).to_le_bytes().to_vec();
    reply.push(kind);
    reply.extend(tag);
    reply.extend(body);
    s.write_all(&reply).unwrap();
}
fn fixture(fault: u8) -> (Opened, thread::JoinHandle<(usize, usize)>) {
    let (client, mut peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let job = thread::spawn(move || {
        let mut names = std::collections::HashMap::new();
        let mut old = None;
        let mut flushes = 0;
        let (mut versions, mut attaches, mut watches, mut clunks) = (0, 0, 0, 0);
        loop {
            let mut h = [0; 7];
            match peer.read_exact(&mut h) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => panic!("{e}"),
            }
            let mut b = vec![0; u32::from_le_bytes(h[..4].try_into().unwrap()) as usize - 7];
            if let Err(e) = peer.read_exact(&mut b) {
                assert!(fault >= 2 && e.kind() == std::io::ErrorKind::UnexpectedEof);
                break;
            }
            let fid = b
                .get(..4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .unwrap_or(u32::MAX);
            let name = names.get(&fid).map(String::as_str).unwrap_or("");
            match h[4] {
                100 => {
                    versions += 1;
                    assert_eq!(versions, 1);
                    send(&mut peer, 101, &h[5..], &b);
                }
                104 => {
                    attaches += 1;
                    assert_eq!(attaches, 1);
                    let mut q = [0; 13];
                    q[0] = 128;
                    send(&mut peer, 105, &h[5..], &q);
                }
                110 => {
                    let name = String::from_utf8(b[12..].to_vec()).unwrap();
                    assert!(name == "api" || name == "watch" || name == "status");
                    if name == "watch" {
                        watches += 1;
                    }
                    let mut r = [0; 15];
                    r[0] = 1;
                    names.insert(u32::from_le_bytes(b[4..8].try_into().unwrap()), name);
                    send(&mut peer, 111, &h[5..], &r);
                }
                12 => {
                    assert_eq!(&b[4..8], &[0; 4]);
                    send(&mut peer, 13, &h[5..], &[0; 17]);
                }
                116 => {
                    let offset = u64::from_le_bytes(b[4..12].try_into().unwrap()) as usize;
                    let count = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
                    if name != "watch" {
                        let api: &[u8] = if name == "api" {
                            b"sophia-portal-client version=1 epoch=7 request=unavailable fd_transfer=none\n"
                        } else {
                            b"request=unavailable transfer=ready\n"
                        };
                        let data = &api[offset..api.len().min(offset + count)];
                        let mut r = (data.len() as u32).to_le_bytes().to_vec();
                        r.extend(data);
                        send(&mut peer, 117, &h[5..], &r);
                    } else {
                        assert_eq!((offset, count), (0, 1));
                        assert!(old.replace(h[5..].to_vec()).is_none());
                    }
                }
                108 => {
                    flushes += 1;
                    assert_eq!(b, *old.as_ref().unwrap());
                    if fault == 1 || fault == 2 {
                        send(&mut peer, 7, old.as_ref().unwrap(), &116u32.to_le_bytes());
                    }
                    if fault == 3 {
                        send(&mut peer, 7, &h[5..], &5u32.to_le_bytes());
                    } else if fault == 4 {
                        break;
                    } else {
                        send(&mut peer, 109, &h[5..], &[]);
                    }
                }
                120 => {
                    if name == "watch" {
                        clunks += 1;
                    }
                    if fault == 2 && name == "watch" {
                        send(&mut peer, 7, &h[5..], &5u32.to_le_bytes());
                    } else {
                        send(&mut peer, 121, &h[5..], &[]);
                    }
                }
                _ => panic!("unexpected operation {}", h[4]),
            }
        }
        assert_eq!(flushes, 1);
        (watches, clunks)
    });
    let mut wire =
        Pipeline::over(client, PipelineLimits::default(), Duration::from_secs(2)).unwrap();
    // Supplied admission, not a production authentication test.
    let (tag, root) = wire.attach(b"", b"").unwrap();
    wire.wait(tag, Instant::now() + Duration::from_secs(2))
        .unwrap();
    let mut opening = Opening::from_attached(wire, root, 0, 100).unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while !opening.is_opened() {
        assert!(Instant::now() < until);
        opening.dispatch(1, 4096).unwrap();
        thread::yield_now();
    }
    (opening.into_opened().unwrap(), job)
}

#[test]
fn flush_order_and_failure_preserve_custody() {
    for fault in 0..5 {
        let (opened, job) = fixture(fault);
        let mut session = Session::new(opened).unwrap();
        session.watch(100).unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        while !session.watch_waiting() {
            assert!(Instant::now() < until);
            session.dispatch(10, 1).unwrap();
            thread::yield_now();
        }
        session.snapshot(File::Status, 100).unwrap();
        session.cancel_watch().unwrap();
        assert_eq!(session.dispatch(9, 4096), Err(Error::ClockRegression));
        assert!(session.error().is_none());
        while session.error().is_none()
            && (session.watch_event().is_none() || session.snapshot_result().is_none())
        {
            assert!(Instant::now() < until);
            let _ = session.dispatch(10, 1);
            thread::yield_now();
        }
        if fault < 2 {
            assert_eq!(
                session.watch_event(),
                Some(if fault == 0 {
                    WatchEvent::Cancelled
                } else {
                    WatchEvent::Revoked
                })
            );
            assert_eq!(
                session.snapshot_result(),
                Some(b"request=unavailable transfer=ready\n".as_slice())
            );
            session.release_watch().unwrap();
            session.release_snapshot().unwrap();
            let opened = session.finish().unwrap();
            assert_eq!(opened.pipeline.fid_count(), 1);
            assert!(opened.pipeline.is_quiescent());
            drop(opened);
        } else {
            assert!(session.error().is_some());
            assert!(session.watch_event().is_none());
            assert!(session.snapshot_result().is_none());
            assert!(session.fd().is_none());
            assert!(session.finish().is_err());
        }
        assert_eq!(job.join().unwrap(), (1, usize::from(fault < 3)));
    }
}
