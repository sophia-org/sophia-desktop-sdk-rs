use sophia_9p_client::pipeline::{Pipeline, PipelineLimits};
use sophia_wm_client::{
    protocol::{wm_files::*, *},
    startup::Opening,
    submission::Custody,
    *,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

fn frame(kind: u8, tag: u16, body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 7) as u32).to_le_bytes().to_vec();
    b.push(kind);
    b.extend(tag.to_le_bytes());
    b.extend(body);
    b
}
fn qid(dir: bool) -> Vec<u8> {
    let mut b = vec![if dir { 128 } else { 0 }];
    b.extend([0; 4]);
    b.extend(1u64.to_le_bytes());
    b
}
fn record(kind: u16, seq: u64, body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 32) as u32).to_le_bytes().to_vec();
    b.extend(1u16.to_le_bytes());
    b.extend(kind.to_le_bytes());
    b.extend(7u64.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    b.extend(seq.to_le_bytes());
    b.extend(body);
    b
}
fn limits() -> Vec<u8> {
    let mut body = 128u64.to_le_bytes().to_vec();
    body.extend(1_048_576u32.to_le_bytes());
    body.extend(1_048_576u32.to_le_bytes());
    body.extend(64u16.to_le_bytes());
    body.extend(32u16.to_le_bytes());
    body.extend(12_000u32.to_le_bytes());
    body.extend(4_000u32.to_le_bytes());
    body.extend([0; 4]);
    record(1, 0, &body)
}
fn read_reply(tag: u16, bytes: &[u8]) -> Vec<u8> {
    let mut body = (bytes.len() as u32).to_le_bytes().to_vec();
    body.extend(bytes);
    frame(117, tag, &body)
}
fn submitted(id: u64, kind: u16, sequence: u64) -> Vec<u8> {
    let mut body = id.to_le_bytes().to_vec();
    body.extend(kind.to_le_bytes());
    body.extend([0; 6]);
    record(17, sequence, &body)
}

struct Report {
    candidates: usize,
    retries: usize,
    acked: u64,
}
fn fixture(event_first: bool, retry: bool, eof: bool) -> (Session, thread::JoinHandle<Report>) {
    let (client, mut peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let server = thread::spawn(move || {
        let mut files = HashMap::<u32, String>::new();
        let mut candidate = vec![];
        let mut event_read = None;
        let mut queued = vec![];
        let mut report = Report {
            candidates: 0,
            retries: 0,
            acked: 0,
        };
        let mut old_submit = None;
        let mut offset = 0u64;
        loop {
            let mut h = [0; 7];
            match peer.read_exact(&mut h) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => panic!("peer: {e}"),
            }
            let mut b = vec![0; u32::from_le_bytes(h[..4].try_into().unwrap()) as usize - 7];
            peer.read_exact(&mut b).unwrap();
            let tag = u16::from_le_bytes(h[5..].try_into().unwrap());
            let fid = if b.len() >= 4 {
                u32::from_le_bytes(b[..4].try_into().unwrap())
            } else {
                0
            };
            match h[4] {
                100 => peer.write_all(&frame(101, tag, &b)).unwrap(),
                104 => {
                    files.insert(fid, "root".into());
                    peer.write_all(&frame(105, tag, &qid(true))).unwrap();
                }
                110 => {
                    assert_eq!(files[&fid], "root");
                    let newfid = u32::from_le_bytes(b[4..8].try_into().unwrap());
                    assert_eq!(u16::from_le_bytes(b[8..10].try_into().unwrap()), 1);
                    let name = String::from_utf8(b[12..].to_vec()).unwrap();
                    if name == "transaction" {
                        if report.candidates > 0 {
                            assert!(report.acked >= 2, "walk before Submitted ACK");
                        }
                        report.candidates += 1;
                        candidate.clear();
                    }
                    files.insert(newfid, name);
                    let mut body = 1u16.to_le_bytes().to_vec();
                    body.extend(qid(false));
                    peer.write_all(&frame(111, tag, &body)).unwrap();
                }
                12 => {
                    let mut body = qid(false);
                    body.extend(0u32.to_le_bytes());
                    peer.write_all(&frame(13, tag, &body)).unwrap();
                }
                116 => {
                    let at = u64::from_le_bytes(b[4..12].try_into().unwrap());
                    let count = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
                    if files[&fid] == "events" {
                        assert!(event_read.is_none());
                        assert_eq!(at, offset);
                        if queued.is_empty() {
                            event_read = Some((tag, count));
                        } else {
                            assert!(queued.len() <= count);
                            offset += queued.len() as u64;
                            peer.write_all(&read_reply(tag, &queued)).unwrap();
                            queued.clear();
                        }
                    } else {
                        let object = match files[&fid].as_str() {
                            "api" => {
                                b"sophia-wm-files version=1 output_transport=9p2000.L\n".to_vec()
                            }
                            "limits" => limits(),
                            _ => panic!("unexpected read"),
                        };
                        let at = at as usize;
                        let end = object.len().min(at + count);
                        peer.write_all(&read_reply(tag, &object[at..end])).unwrap();
                    }
                }
                118 => {
                    let at = u64::from_le_bytes(b[4..12].try_into().unwrap()) as usize;
                    let data = &b[16..];
                    let response = frame(119, tag, &(data.len() as u32).to_le_bytes());
                    match files[&fid].as_str() {
                        "transaction" => {
                            assert_eq!(at, candidate.len());
                            candidate.extend(data);
                            peer.write_all(&response).unwrap();
                        }
                        "ack" => {
                            assert_eq!(data.len(), 16);
                            report.acked = u64::from_le_bytes(data[8..16].try_into().unwrap());
                            peer.write_all(&response).unwrap();
                        }
                        "submit" => {
                            assert_eq!(data.len(), 24);
                            let id = u64::from_le_bytes(data[8..16].try_into().unwrap());
                            let kind = u16::from_le_bytes(candidate[6..8].try_into().unwrap());
                            assert_eq!(data[8..16], candidate[16..24]);
                            assert_eq!(
                                u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize,
                                candidate.len()
                            );
                            if kind == 263 && retry && report.retries == 0 {
                                report.retries += 1;
                                old_submit = Some(data.to_vec());
                                peer.write_all(&frame(7, tag, &11u32.to_le_bytes()))
                                    .unwrap();
                                continue;
                            }
                            if let Some(old) = &old_submit {
                                assert_eq!(old, data);
                            }
                            if kind == 256 {
                                queued.extend(record(16, 1, &128u64.to_le_bytes()));
                                queued.extend(submitted(id, kind, 2));
                            } else {
                                assert_eq!(kind, 263);
                                queued.extend(submitted(id, kind, 3));
                                let mut body = candidate[32..48].to_vec();
                                body.extend(1u16.to_le_bytes());
                                body.extend([0; 6]);
                                queued.extend(record(24, 4, &body));
                            }
                            let mut batch = vec![];
                            if !event_first {
                                batch.extend(&response);
                            }
                            if let Some((event_tag, count)) = event_read.take() {
                                assert!(queued.len() <= count);
                                offset += queued.len() as u64;
                                batch.extend(read_reply(event_tag, &queued));
                                queued.clear();
                            }
                            if event_first {
                                batch.extend(&response);
                            }
                            peer.write_all(&batch).unwrap();
                            if eof && kind == 263 {
                                break;
                            }
                        }
                        _ => panic!("unexpected write"),
                    }
                }
                120 => {
                    files.remove(&fid);
                    peer.write_all(&frame(121, tag, &[])).unwrap();
                }
                _ => panic!("unexpected opcode {}", h[4]),
            }
        }
        report
    });
    let wire = Pipeline::over(client, PipelineLimits::default(), Duration::from_secs(2)).unwrap();
    let mut opening = Opening::direct(
        wire,
        WmFileNegotiationOffer {
            required_capabilities: 128,
            optional_capabilities: 0,
        },
        10,
        1_000_000,
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while !opening.is_opened() {
        assert!(Instant::now() < end);
        opening.dispatch(10, 65536).unwrap();
        thread::yield_now();
    }
    (
        Session::from_opened(opening.into_opened().unwrap()).unwrap(),
        server,
    )
}

fn drive_until(s: &mut Session, now: &mut u64, done: impl Fn(&Session) -> bool) {
    let end = Instant::now() + Duration::from_secs(2);
    while !done(s) {
        assert!(Instant::now() < end);
        *now += 1;
        s.dispatch(*now, 65536).unwrap();
        thread::yield_now();
    }
}

#[test]
fn negotiated_session_submits_retries_and_drains_ordered_events_over_real_pipeline() {
    for (event_first, retry) in [(true, false), (false, false), (true, true)] {
        let (mut s, peer) = fixture(event_first, retry, false);
        let mut now = 10;
        drive_until(&mut s, &mut now, |s| {
            s.state() == SessionState::Ready && s.acked() >= 2
        });
        assert_eq!(s.custody(1), Some(Custody::Submitted));
        let candidate = Candidate::SessionOperation(WmFileSessionOperation {
            transaction: TransactionId::from_raw(19),
            request: PolicySessionOperationRequest {
                connection_epoch: 7,
                request_id: 31,
                operation: 1,
                target: None,
            },
        });
        let id = loop {
            match s.submit(&candidate, now + 100_000) {
                Ok(id) => break id,
                Err(SessionError::Submission(submission::Error::Busy)) => {
                    now += 1;
                    s.dispatch(now, 65536).unwrap();
                }
                other => panic!("admission: {other:?}"),
            }
        };
        drive_until(&mut s, &mut now, |s| s.event().is_some());
        assert_eq!(s.custody(id), Some(Custody::Submitted));
        assert_eq!(s.event().unwrap().0.sequence, 4);
        assert_eq!(s.consumed(), 3);
        s.consume().unwrap();
        drive_until(&mut s, &mut now, |s| s.acked() == 4);
        s.close();
        assert_eq!(s.custody(id), Some(Custody::Submitted));
        let report = peer.join().unwrap();
        assert_eq!(report.candidates, 2);
        assert_eq!(report.retries, usize::from(retry));
        assert_eq!(report.acked, 4);
    }
}

#[test]
fn final_journal_bytes_preserve_custody_on_eof() {
    let (mut s, peer) = fixture(false, false, true);
    let mut now = 10;
    drive_until(&mut s, &mut now, |s| {
        s.state() == SessionState::Ready && s.acked() >= 2
    });
    // Allow the negotiation transaction's clunk to settle as well.
    let candidate = Candidate::SessionOperation(WmFileSessionOperation {
        transaction: TransactionId::from_raw(19),
        request: PolicySessionOperationRequest {
            connection_epoch: 7,
            request_id: 31,
            operation: 1,
            target: None,
        },
    });
    let id = loop {
        match s.submit(&candidate, now + 100_000) {
            Ok(id) => break id,
            Err(SessionError::Submission(submission::Error::Busy)) => {
                now += 1;
                s.dispatch(now, 65536).unwrap();
            }
            other => panic!("{other:?}"),
        }
    };
    let end = Instant::now() + Duration::from_secs(2);
    while s.error().is_none() {
        assert!(Instant::now() < end);
        now += 1;
        let _ = s.dispatch(now, 65536);
        thread::yield_now();
    }
    assert_eq!(s.custody(id), Some(Custody::Submitted));
    assert!(s.fd().is_none());
    peer.join().unwrap();
}
