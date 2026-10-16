use crate::{Session, SessionError, SessionState, startup::Opening};
use sophia_9p_client::pipeline::{IoInterest, Pipeline, PipelineLimits, VersionHandshake};
use sophia_9p_records::Fid;
use sophia_wm_protocol::wm_files::WmFileNegotiationOffer;
use std::os::{fd::BorrowedFd, unix::net::UnixStream};

enum Phase {
    Version(VersionHandshake),
    Files(Box<Opening>),
    Session(Box<Session>),
}

/// Nonblocking WM startup, from a connected socket or an authenticated root.
/// One absolute deadline covers version, file startup and role negotiation.
pub struct Connecting {
    phase: Option<Phase>,
    offer: WmFileNegotiationOffer,
    now: u64,
    deadline: u64,
    error: Option<SessionError>,
}
impl Connecting {
    /// For supplied-admission exports. Protected production launches use
    /// `attached`; authentication failure must never fall back to this path.
    pub fn direct(
        stream: UnixStream,
        limits: PipelineLimits,
        offer: WmFileNegotiationOffer,
        now: u64,
        deadline: u64,
    ) -> Result<Self, SessionError> {
        if offer.required_capabilities & offer.optional_capabilities != 0 {
            return Err(SessionError::Protocol("overlapping capability offer"));
        }
        Ok(Self {
            phase: Some(Phase::Version(VersionHandshake::new(
                stream, limits, now, deadline,
            )?)),
            offer,
            now,
            deadline,
            error: None,
        })
    }
    pub fn attached(
        pipeline: Pipeline,
        root: Fid,
        offer: WmFileNegotiationOffer,
        now: u64,
        deadline: u64,
    ) -> Result<Self, SessionError> {
        Ok(Self {
            phase: Some(Phase::Files(Box::new(Opening::attached(
                pipeline, root, offer, now, deadline,
            )?))),
            offer,
            now,
            deadline,
            error: None,
        })
    }
    pub fn ready(&self) -> bool {
        self.error.is_none()
            && matches!(&self.phase,Some(Phase::Session(s)) if s.state()==SessionState::Ready)
    }
    pub fn deadline(&self) -> u64 {
        self.deadline
    }
    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        match self.phase.as_ref()? {
            Phase::Version(v) => v.fd(),
            Phase::Files(f) => f.fd(),
            Phase::Session(s) => s.fd(),
        }
    }
    pub fn io_interest(&self) -> Result<IoInterest, SessionError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(match self.phase.as_ref().ok_or(SessionError::Closed)? {
            Phase::Version(v) => v.io_interest(),
            Phase::Files(f) => f.io_interest()?,
            Phase::Session(s) => s.io_interest()?,
        })
    }
    /// One phase performs socket I/O per call; phase transitions do not
    /// replenish the caller's budget. Each next phase is queued without I/O.
    pub fn dispatch(&mut self, now: u64, bytes: usize) -> Result<(), SessionError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if now < self.now {
            return Err(SessionError::ClockRegression);
        }
        self.now = now;
        let mut phase = self.phase.take().ok_or(SessionError::Closed)?;
        let result = (|| -> Result<(), SessionError> {
            match &mut phase {
                Phase::Version(v) => {
                    v.dispatch(now, bytes)?;
                    if let Some(pipeline) = v.take_pipeline() {
                        phase = Phase::Files(Box::new(Opening::direct(
                            pipeline,
                            self.offer,
                            now,
                            self.deadline,
                        )?));
                    }
                }
                Phase::Files(f) => f.dispatch(now, bytes)?,
                Phase::Session(s) => s.dispatch(now, bytes)?,
            }
            Ok(())
        })();
        if result.is_ok()
            && matches!(&phase,Phase::Files(f) if f.is_opened())
            && let Phase::Files(files) = phase
        {
            match (*files)
                .into_opened()
                .map_err(SessionError::from)
                .and_then(Session::from_opened)
            {
                Ok(session) => phase = Phase::Session(Box::new(session)),
                Err(error) => {
                    self.error = Some(error.clone());
                    return Err(error);
                }
            }
        }
        self.phase = Some(phase);
        if let Err(error) = &result {
            self.error = Some(error.clone());
            self.close_phase();
        }
        result
    }
    pub fn take_session(&mut self) -> Option<Session> {
        if !self.ready() {
            return None;
        }
        match self.phase.take()? {
            Phase::Session(s) => Some(*s),
            _ => unreachable!(),
        }
    }
    fn close_phase(&mut self) {
        if let Some(phase) = &mut self.phase {
            match phase {
                Phase::Version(v) => v.close(),
                Phase::Files(f) => f.close(),
                Phase::Session(s) => s.close(),
            }
        }
    }
    pub fn close(&mut self) {
        self.close_phase();
        self.error.get_or_insert(SessionError::Closed);
    }
}
