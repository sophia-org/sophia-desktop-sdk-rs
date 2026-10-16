//! Capture write and status observation on an exclusive authenticated role.
use crate::{
    capture::{Custody, Pending},
    protocol::{CaptureRequest, CaptureState, RequestCapability},
    snapshot::{Fetch, File, Operation},
    startup::Opened,
};
use sophia_9p_client::pipeline::{IoBudget, IoInterest, PipelineError, Reply};
use sophia_9p_records::{Fid, QidKind, Tag};
use std::os::fd::{AsFd, BorrowedFd};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Unsupported,
    ClockRegression,
    Closed,
    Remote(u32),
    Wire(PipelineError),
    Capture(crate::capture::Error),
    Snapshot(crate::snapshot::Error),
}
impl From<PipelineError> for Error {
    fn from(e: PipelineError) -> Self {
        Self::Wire(e)
    }
}
impl From<crate::capture::Error> for Error {
    fn from(e: crate::capture::Error) -> Self {
        Self::Capture(e)
    }
}
impl From<crate::snapshot::Error> for Error {
    fn from(e: crate::snapshot::Error) -> Self {
        Self::Snapshot(e)
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Walk,
    Open,
    Write,
    Clunk,
    Wait,
    Status,
    Ready,
}

pub struct Request {
    opened: Option<Opened>,
    capture: Pending,
    fetch: Option<Fetch<'static>>,
    stage: Stage,
    pending: Option<Tag>,
    fid: Fid,
    serial: u64,
    now: u64,
    deadline: u64,
    error: Option<Error>,
}
impl Request {
    /// The caller owns the recipient's durable serial high-water mark, including
    /// across reconnects. Supply it here and reserve request.serial before use;
    /// failure never permits reuse. Retain the launch relay separately.
    pub fn new(
        opened: Opened,
        request: CaptureRequest,
        previous_serial: u64,
        now: u64,
        deadline: u64,
    ) -> Result<Self, Error> {
        if now < opened.now {
            return Err(Error::ClockRegression);
        }
        if opened.info.request != RequestCapability::OutputCaptureV1 {
            return Err(Error::Unsupported);
        }
        let w = &opened.pipeline;
        if request.serial <= previous_serial
            || !w.is_quiescent()
            || w.fid_count() != 1
            || !w.owns_fid(opened.root)
            || w.limits().max_fids < 2
            || w.limits().max_outstanding < 2
            || !(512..=65536).contains(&w.msize())
            || w.limits().max_buffered_input < w.msize() as usize
        {
            return Err(Error::Invalid);
        }
        let mut s = Self {
            opened: Some(opened),
            capture: Pending::new(request, now, deadline)?,
            fetch: None,
            stage: Stage::Walk,
            pending: None,
            fid: Fid::NOFID,
            serial: request.serial,
            now,
            deadline,
            error: None,
        };
        s.drive()?;
        Ok(s)
    }
    pub fn custody(&self) -> Custody {
        self.capture.custody()
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }
    pub fn outcome(&self) -> Option<CaptureState> {
        (self.stage == Stage::Ready && self.error.is_none())
            .then(|| self.capture.outcome())
            .flatten()
    }
    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        self.opened.as_ref().map(|o| o.pipeline.as_fd())
    }
    pub fn io_interest(&self) -> Result<IoInterest, Error> {
        self.opened
            .as_ref()
            .ok_or(Error::Closed)?
            .pipeline
            .io_interest()
            .map_err(Into::into)
    }
    pub fn next_wake(&self) -> Option<u64> {
        if self.error.is_some() || self.stage == Stage::Ready {
            None
        } else if self.stage == Stage::Wait {
            self.capture.next_wake()
        } else {
            Some(self.deadline)
        }
    }
    fn fail(&mut self, e: Error) -> Error {
        self.capture.close();
        self.opened.take();
        self.error.get_or_insert(e).clone()
    }
    /// Cancels local observation; an issued request may still run on the server.
    pub fn close(&mut self) {
        self.fail(Error::Closed);
    }
    /// Same root and pipeline, with no reauthentication. The returned serial is
    /// the high-water mark for the next explicit request, not permission to replay.
    pub fn finish(mut self) -> Result<(Opened, CaptureState, u64), Error> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let outcome = self.outcome().ok_or(Error::Invalid)?;
        let mut opened = self.opened.take().ok_or(Error::Closed)?;
        opened.now = self.now;
        Ok((opened, outcome, self.serial))
    }
    /// Bounded work, caller time and no internal wait. Call at next_wake even
    /// without descriptor readiness so a due status snapshot can be submitted.
    pub fn dispatch(&mut self, now: u64, bytes: usize) -> Result<(), Error> {
        if now < self.now {
            return Err(Error::ClockRegression);
        }
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        self.now = now;
        if self.stage == Stage::Ready {
            return Ok(());
        }
        if let Err(e) = self.capture.advance(now) {
            return Err(self.fail(e.into()));
        }
        if let Err(e) = self.drive() {
            return Err(self.fail(e));
        }
        let mut budget = IoBudget::new(bytes);
        for _ in 0..32 {
            let w = &mut self.opened.as_mut().ok_or(Error::Closed)?.pipeline;
            if let Err(e) = w.poll_with_budget(&mut budget) {
                return Err(self.fail(e.into()));
            }
            let Some((tag, reply)) = w.take_reply() else {
                break;
            };
            if self.pending.take() != Some(tag) {
                return Err(self.fail(Error::Invalid));
            }
            if let Err(e) = self.accept(reply).and_then(|()| self.drive()) {
                return Err(self.fail(e));
            }
            if self.stage == Stage::Ready || budget.exhausted() {
                break;
            }
        }
        Ok(())
    }
    fn drive(&mut self) -> Result<(), Error> {
        if self.pending.is_some() || self.stage == Stage::Ready {
            return Ok(());
        }
        if self.stage == Stage::Wait {
            if self.capture.next_wake().is_some_and(|t| t > self.now) {
                return Ok(());
            }
            let deadline = self.capture.start_status(self.now)?;
            self.fetch = Some(Fetch::new(File::CaptureStatus, self.now, deadline)?);
            self.stage = Stage::Status;
        }
        let o = self.opened.as_mut().ok_or(Error::Closed)?;
        let w = &mut o.pipeline;
        self.pending = Some(match self.stage {
            Stage::Walk => {
                let (t, f) = w.walk(o.root, &[b"capture"])?;
                self.fid = f;
                t
            }
            Stage::Open => w.lopen(self.fid, 1)?,
            Stage::Write => {
                let tag = w.write(self.fid, 0, self.capture.bytes())?;
                self.capture.issued(self.now)?;
                tag
            }
            Stage::Clunk => w.clunk(self.fid)?,
            Stage::Status => {
                let fetch = self.fetch.as_mut().ok_or(Error::Invalid)?;
                let op = fetch
                    .operation(self.now, w.msize())?
                    .ok_or(Error::Invalid)?;
                let mut fid = Fid::NOFID;
                let t = match op {
                    Operation::Walk(file) => {
                        let (t, f) = w.walk(o.root, &[file.name()])?;
                        fid = f;
                        t
                    }
                    Operation::Open(f) => w.lopen(f, 0)?,
                    Operation::Read { fid, offset, count } => w.read(fid, offset, count)?,
                    Operation::Clunk(f) => w.clunk(f)?,
                };
                fetch.issued(self.now, w.msize(), fid)?;
                t
            }
            _ => return Err(Error::Invalid),
        });
        Ok(())
    }
    fn accept(&mut self, reply: Reply) -> Result<(), Error> {
        if self.stage == Stage::Write {
            let result = match reply {
                Reply::Write(n) => Ok(n),
                Reply::Error(e) => Err(e.0),
                _ => return Err(Error::Invalid),
            };
            self.capture.write_reply(result, self.now)?;
            self.stage = Stage::Clunk;
            return Ok(());
        }
        if let Reply::Error(e) = reply {
            return Err(Error::Remote(e.0));
        }
        if self.stage == Stage::Status {
            let fetch = self.fetch.as_mut().ok_or(Error::Invalid)?;
            fetch.reply(reply, self.now)?;
            if let Some(bytes) = fetch.result() {
                self.capture.status(bytes, self.now)?;
                self.stage = if self.capture.outcome().is_some() {
                    Stage::Ready
                } else {
                    Stage::Wait
                };
                self.fetch = None;
            }
            return Ok(());
        }
        match (self.stage, reply) {
            (Stage::Walk, Reply::Walk(q)) if q.len() == 1 && q[0].kind == QidKind::File => {
                self.stage = Stage::Open
            }
            (Stage::Open, Reply::Lopen { qid, iounit })
                if qid.kind == QidKind::File && (iounit == 0 || iounit >= 24) =>
            {
                self.stage = Stage::Write
            }
            (Stage::Clunk, Reply::Clunk) => self.stage = Stage::Wait,
            _ => return Err(Error::Invalid),
        }
        Ok(())
    }
}
