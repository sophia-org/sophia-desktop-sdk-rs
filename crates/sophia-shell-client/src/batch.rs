//! Atomic local admission of independent content operations.
use crate::wire::Outbound;
use crate::{Admission, ContentReservation, QueueReservation, ShellClientError, ShellConnection};
use sophia_shell_protocol::{ShellContentRecord, TransactionId};

/// One operation in a bounded content batch. A whole candidate is one file
/// record; Begin/Chunk/End fragments are not independent candidate submissions.
#[derive(Clone, Copy)]
pub enum ContentBatchItem<'a> {
    Record(TransactionId, &'a ShellContentRecord),
    Candidate(TransactionId, &'a [ShellContentRecord]),
}

impl ContentBatchItem<'_> {
    pub(crate) fn outbound(self) -> Outbound {
        match self {
            Self::Record(tx, record) => Outbound::Content(tx, record.clone()),
            Self::Candidate(tx, records) => Outbound::ContentGroup(tx, records.to_vec()),
        }
    }

    fn is_control(self) -> bool {
        matches!(self, Self::Record(_, ShellContentRecord::ActionAck(_)))
    }
}

impl ShellConnection {
    /// Admit all operations or none, with one ticket per item and no socket
    /// I/O. Each operation still has its own remote custody and outcome; this
    /// is not an atomic server transaction. Mixed batches use bulk credits;
    /// only all-ACK batches use the control allowance. Input is copied.
    pub fn enqueue_content_batch(
        &mut self,
        items: &[ContentBatchItem<'_>],
    ) -> Result<Admission, ShellClientError> {
        Ok(self.reserve_content_batch(items)?.commit())
    }

    /// Prepare an atomic batch before a local effect. Drop cancels every item
    /// and staged upload claim. Commit cannot allocate, perform I/O or fail for
    /// capacity. As with other prepared handles, the connection stays borrowed.
    /// This does not register ContentLifecycle metadata.
    pub fn reserve_content_batch(
        &mut self,
        items: &[ContentBatchItem<'_>],
    ) -> Result<ContentReservation<'_>, ShellClientError> {
        self.wire.io_interest()?;
        let units = self.wire.encode_content_batch(items)?;
        self.wire.reserve_encoded()?;
        let prepared = self.output.prepare(
            units,
            items.iter().all(|item| item.is_control()),
            &mut self.ledger,
        )?;
        Ok(ContentReservation {
            connection: self,
            prepared: Some(prepared),
        })
    }

    /// Commit an atomic content batch against delayed-content capacity. Size,
    /// encoding or allocation failure preserves the reservation and issues no
    /// tickets. Successful commit releases unused credits. An observed terminal
    /// connection releases the reservation and admits nothing.
    pub fn commit_reserved_content_batch(
        &mut self,
        token: &QueueReservation,
        items: &[ContentBatchItem<'_>],
    ) -> Result<Admission, ShellClientError> {
        if let Err(error) = self.wire.io_interest() {
            self.output.release_reservation();
            return Err(error);
        }
        self.output.validate_reservation(token)?;
        let units = self.wire.encode_content_batch(items)?;
        self.wire.reserve_encoded()?;
        let prepared = self.output.prepare_reserved(
            token,
            units,
            items.iter().all(|item| item.is_control()),
            &mut self.ledger,
        )?;
        let admission = self.output.commit(prepared, &mut self.ledger);
        self.wire.commit_encoded();
        Ok(admission)
    }
}
