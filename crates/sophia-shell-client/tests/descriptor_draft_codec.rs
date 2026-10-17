//! Independent draft decoder fixtures for Sophia x6jp929h at 3a1e5588d.
//! Not a public codec, negotiated feature, or production admission implementation.
fn n(b: &[u8], at: usize, width: usize) -> Option<u64> {
    let mut value = 0;
    for (shift, byte) in b.get(at..at.checked_add(width)?)?.iter().enumerate() {
        value |= u64::from(*byte) << (shift * 8);
    }
    Some(value)
}
fn zero(b: &[u8]) -> bool {
    b.iter().all(|v| *v == 0)
}
fn metadata(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() != 96 || n(b, 0, 8)? == 0 || n(b, 8, 8)? == 0 || n(b, 16, 8)? == 0 {
        return None;
    }
    // The fixture's admitted grant/allocation, independently of packet framing.
    if [n(b, 24, 8)?, n(b, 32, 8)?, n(b, 40, 8)?, n(b, 48, 8)?] != [6, 7, 8, 9]
        || n(b, 56, 4)? != 1
        || n(b, 60, 4)? != 1
    {
        return None;
    }
    let (w, h) = (n(b, 64, 4)?, n(b, 68, 4)?);
    let bytes = w.checked_mul(h)?.checked_mul(4)?;
    if w == 0
        || h == 0
        || w > 4096
        || h > 4096
        || bytes > 4 * 1024 * 1024
        || n(b, 72, 4)? != w * 4
        || n(b, 76, 2)? != 1
        || n(b, 78, 2)? != 1
        || n(b, 80, 8)? != bytes
        || n(b, 88, 8)? != 0
    {
        return None;
    }
    Some((n(b, 0, 8)?, w * 4))
}
fn channel(b: &[u8], actual_fds: u64) -> Option<()> {
    if n(b, 0, 4)? != b.len() as u64
        || n(b, 4, 2)? != 1
        || !matches!(n(b, 8, 2)?, 1 | 2)
        || n(b, 12, 4)? != 0
        || n(b, 16, 8)? != 1
        || n(b, 24, 8)? != 2
        || n(b, 10, 2)? != actual_fds
    {
        return None;
    }
    match n(b, 6, 2)? {
        1 if b.len() == 40 && actual_fds == 0 && n(b, 32, 8)? == 0 => Some(()),
        2 if b.len() == 200 && actual_fds == 1 => {
            let (offer, stride) = metadata(&b[40..136])?;
            if n(b, 32, 8)? != offer
                || n(b, 136, 8)? != 0
                || n(b, 144, 4)? != stride
                || !zero(&b[148..])
            {
                return None;
            }
            Some(())
        }
        _ => None,
    }
}
fn record(b: &[u8]) -> Option<()> {
    if b.len() < 32 || n(b, 0, 4)? != b.len() as u64 || n(b, 4, 2)? != 1 || n(b, 8, 8)? != 1 {
        return None;
    }
    let kind = n(b, 6, 2)?;
    let submission = n(b, 16, 8)?;
    let sequence = n(b, 24, 8)?;
    if (kind == 0x7000 && (submission == 0 || sequence != 0))
        || ((0x7001..=0x7004).contains(&kind) && (submission != 0 || sequence == 0))
        || ((0x7005..=0x7006).contains(&kind) && (submission != 0 || sequence != 0))
    {
        return None;
    }
    match kind {
        0x7000 | 0x7001 => {
            metadata(&b[32..])?;
        }
        0x7002 => {
            if b.len() != 64
                || n(b, 32, 8)? == 0
                || n(b, 40, 8)? == 0
                || n(b, 48, 8)? == 0
                || !(1..=5).contains(&n(b, 56, 4)?)
                || n(b, 60, 4)? != 0
            {
                return None;
            }
        }
        0x7003 | 0x7004 => {
            if b.len() != 48
                || n(b, 32, 8)? != 2
                || n(b, 44, 4)? != 0
                || !(1..=if kind == 0x7003 { 3 } else { 4 }).contains(&n(b, 40, 4)?)
            {
                return None;
            }
        }
        0x7005 => {
            if b.len() != 136 || n(b, 32, 8)? != 2 || n(b, 40, 8)? != 1 || n(b, 128, 8)? != 0 {
                return None;
            }
            let resource = n(b, 48, 8)?;
            let offers = n(b, 84, 4)?;
            if resource == 0
                || resource > 4194304
                || n(b, 56, 8)? < resource
                || !zero(&b[64..84])
                || !(1..=4).contains(&offers)
                || !(1..=offers).contains(&n(b, 88, 4)?)
                || n(b, 92, 4)? != 0
                || !(1..=16).contains(&n(b, 96, 4)?)
                || !(1..=256).contains(&n(b, 100, 4)?)
                || !(1..=4096).contains(&n(b, 104, 4)?)
                || !(1..=4096).contains(&n(b, 108, 4)?)
                || [n(b, 112, 4)?, n(b, 116, 4)?, n(b, 120, 4)?, n(b, 124, 4)?]
                    != [2000, 500, 65536, 1]
            {
                return None;
            }
        }
        0x7006 => {
            // This fixture negotiated sealed-only; a GPU row is not authorized.
            if b.len() != 64
                || n(b, 32, 8)? != 2
                || n(b, 40, 8)? != 9
                || n(b, 48, 8)? != 1
                || n(b, 56, 4)? != 0
                || n(b, 60, 4)? != 0
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(())
}
fn discovery(b: &[u8]) -> Option<()> {
    if b.len() > 2048 || b.last() != Some(&b'\n') {
        return None;
    }
    let s = std::str::from_utf8(b).ok()?;
    let mut lines = s.strip_suffix('\n')?.split('\n');
    if lines.next()? != "sophia-image-channel version=1" {
        return None;
    }
    let fields = [
        "role",
        "connection_epoch",
        "channel_generation",
        "endpoint",
        "source_kinds",
        "max_packet_bytes",
        "max_fds",
        "max_offers",
        "max_packets_per_turn",
        "hello_timeout_ms",
        "validation_timeout_ms",
        "validation_chunk_bytes",
        "capture_interest_timeout_ms",
        "formats_generation",
        "limits_generation",
    ];
    let expected = [0, 1, 2, 0, 1, 200, 4, 4, 1, 250, 2000, 65536, 500, 9, 1];
    for (i, key) in fields.iter().enumerate() {
        let (name, value) = lines.next()?.split_once('=')?;
        if name != *key {
            return None;
        }
        if i == 3 {
            if value.is_empty()
                || value.len() > 64
                || matches!(value, "." | "..")
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            {
                return None;
            }
        } else {
            if value.is_empty()
                || (value.len() > 1 && value.starts_with('0'))
                || !value.bytes().all(|c| c.is_ascii_digit())
            {
                return None;
            }
            let number: u64 = value.parse().ok()?;
            if (i == 0 && !matches!(number, 1 | 2)) || (i != 0 && number != expected[i]) {
                return None;
            }
        }
    }
    if lines.next().is_some() {
        return None;
    }
    Some(())
}
fn accepts(family: &str, b: &[u8], fds: u64) -> bool {
    match family {
        "channel" => channel(b, fds),
        "record" => record(b),
        "discovery" => discovery(b),
        _ => None,
    }
    .is_some()
}
#[cfg(target_os = "linux")]
fn sealed_source_matches(packet: &[u8], fd: &rustix::fd::OwnedFd) -> bool {
    use rustix::fs::{SealFlags, fcntl_get_seals, fstat};
    if channel(packet, 1).is_none() || packet.len() != 200 {
        return false;
    }
    let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    fcntl_get_seals(fd).is_ok_and(|seals| seals.contains(required))
        && fstat(fd)
            .ok()
            .and_then(|stat| u64::try_from(stat.st_size).ok())
            == n(packet, 120, 8)
}
#[cfg(target_os = "linux")]
fn check_file_sizes(packet: &[u8]) {
    use rustix::fs::{self, MemfdFlags, SealFlags};
    let unsupported = fs::open(
        "/dev/null",
        fs::OFlags::RDWR | fs::OFlags::CLOEXEC,
        fs::Mode::empty(),
    )
    .unwrap();
    assert!(!sealed_source_matches(packet, &unsupported));
    for size in [0, 7, 8, 9] {
        let fd = fs::memfd_create(
            "draft-size",
            MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
        )
        .unwrap();
        fs::ftruncate(&fd, size).unwrap();
        fs::fcntl_add_seals(
            &fd,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
        )
        .unwrap();
        assert_eq!(sealed_source_matches(packet, &fd), size == 8);
    }
}
#[test]
fn draft_corpus_and_all_truncated_prefixes() {
    let mut total = 0;
    for line in include_str!("support/image_channel_draft.tsv").lines() {
        if line.starts_with('#') {
            continue;
        }
        let v: Vec<_> = line.split('\t').collect();
        assert_eq!(v.len(), 5);
        let bytes: Vec<_> = v[4]
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let fds = v[3].parse().unwrap();
        assert_eq!(accepts(v[1], &bytes, fds), v[2] == "1", "{}", v[0]);
        if v[2] == "1" {
            #[cfg(target_os = "linux")]
            if v[1] == "channel" && bytes.len() == 200 {
                check_file_sizes(&bytes);
            }
            for end in 0..bytes.len() {
                assert!(!accepts(v[1], &bytes[..end], fds), "{} prefix {end}", v[0]);
            }
            let mut extra = bytes;
            extra.push(0);
            assert!(!accepts(v[1], &extra, fds));
        }
        total += 1;
    }
    assert_eq!(total, 53);
}
