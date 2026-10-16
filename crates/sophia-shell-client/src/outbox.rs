use std::collections::VecDeque;

use crate::custody::{Admission, Ledger};
use crate::{MAX_QUEUED_BYTES, MAX_QUEUED_FRAMES, ShellClientError};

// The r5 ceiling is sixteen pending actions. Each can need an ACK plus an
// indicator activation. Uploads cannot occupy these record or byte credits.
const CONTROL_RECORDS: usize = 32;
const CONTROL_BYTES: usize = CONTROL_RECORDS * 256;

#[derive(Default)]
pub(crate) struct ClientOutbox {
    frames: VecDeque<Frame>,
    bytes: usize,
    bulk_records: usize,
    bulk_bytes: usize,
}
struct Frame {
    bytes: Option<Box<[u8]>>,
    /// Charges stay here while the file wire owns the payload.
    len: usize,
    control: bool,
    ticket: u64,
}
pub(crate) struct PreparedQueue {
    frames: Vec<Frame>,
    bytes: usize,
    control: bool,
}
impl ClientOutbox {
    pub(crate) fn enqueue(
        &mut self,
        frames: Vec<Vec<u8>>,
        control: bool,
        ledger: &mut Ledger,
    ) -> Result<Admission, ShellClientError> {
        self.enqueue_after(frames, control, ledger, || Ok(()))
    }

    /// Reserve the complete FIFO transfer before updating its producer. The
    /// callback cannot access this mutably borrowed FIFO. Once it succeeds,
    /// only infallible moves and integer accounting remain before ownership.
    /// Each unit gets its ticket then, so a refused admission spends none.
    pub(crate) fn enqueue_after(
        &mut self,
        frames: Vec<Vec<u8>>,
        control: bool,
        ledger: &mut Ledger,
        before_commit: impl FnOnce() -> Result<(), ShellClientError>,
    ) -> Result<Admission, ShellClientError> {
        let prepared = self.prepare(frames, control, ledger)?;
        before_commit()?;
        Ok(self.commit(prepared, ledger))
    }

    /// The caller must exclusively retain this outbox and ledger until commit
    /// or cancellation. All queue and ledger capacity is acquired here.
    pub(crate) fn prepare(
        &mut self,
        frames: Vec<Vec<u8>>,
        control: bool,
        ledger: &mut Ledger,
    ) -> Result<PreparedQueue, ShellClientError> {
        let bytes: usize = frames.iter().map(Vec::len).sum();
        if self.frames.len().saturating_add(frames.len()) > MAX_QUEUED_FRAMES
            || self.bytes.saturating_add(bytes) > MAX_QUEUED_BYTES
            || (!control
                && (self.bulk_records.saturating_add(frames.len())
                    > MAX_QUEUED_FRAMES - CONTROL_RECORDS
                    || self.bulk_bytes.saturating_add(bytes) > MAX_QUEUED_BYTES - CONTROL_BYTES))
        {
            return Err(ShellClientError::QueueSaturated);
        }
        // Prepare all allocations before transferring either half. Extending
        // the already-reserved FIFO cannot leave just an ACK after unwind.
        let first = ledger.peek();
        let prepared: Vec<_> = frames
            .into_iter()
            .zip(first..)
            .map(|(bytes, ticket)| Frame {
                len: bytes.len(),
                bytes: Some(bytes.into_boxed_slice()),
                control,
                ticket,
            })
            .collect();
        let records = prepared.len();
        self.frames
            .try_reserve(records)
            .map_err(|_| ShellClientError::QueueSaturated)?;
        ledger.reserve(records)?;
        Ok(PreparedQueue {
            frames: prepared,
            bytes,
            control,
        })
    }

    pub(crate) fn commit(&mut self, prepared: PreparedQueue, ledger: &mut Ledger) -> Admission {
        let records = prepared.frames.len();
        self.frames.extend(prepared.frames);
        self.bytes += prepared.bytes;
        if !prepared.control {
            self.bulk_records += records;
            self.bulk_bytes += prepared.bytes;
        }
        ledger.issue(records)
    }
    pub(crate) fn front(&self) -> Option<&[u8]> {
        self.frames.front().and_then(|frame| frame.bytes.as_deref())
    }

    /// Transfer bytes without releasing the unit's admission or custody ticket.
    pub(crate) fn take_front_bytes(&mut self) -> Box<[u8]> {
        self.frames
            .front_mut()
            .and_then(|frame| frame.bytes.take())
            .expect("the current file operation takes its payload once")
    }
    /// The front unit's ticket.
    pub(crate) fn front_ticket(&self) -> Option<u64> {
        self.frames.front().map(|frame| frame.ticket)
    }

    /// Releases the front unit after the file wire settles its custody.
    pub(crate) fn retire_front(&mut self) {
        let frame = self
            .frames
            .pop_front()
            .expect("retiring requires an owned frame");
        self.bytes -= frame.len;
        if !frame.control {
            self.bulk_records -= 1;
            self.bulk_bytes -= frame.len;
        }
    }

    /// Every unit still queued, front first, by ticket.
    pub(crate) fn tickets(&self) -> impl Iterator<Item = u64> + '_ {
        self.frames.iter().map(|frame| frame.ticket)
    }
}

#[cfg(test)]
#[path = "../tests/support/outbox.rs"]
mod tests;
