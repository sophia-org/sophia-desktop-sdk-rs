use super::*;
use sophia_image_producer::OwnedRingSlotFill;

impl ImageAdapter {
    fn owned_fill_ready(
        &mut self,
        wire: &FileWire,
        now: Instant,
    ) -> Result<u64, ImageAdapterError> {
        let now_ms = self.advance_clock(now)?;
        if wire.peer_closed() || self.failure.is_some() || self.final_notified {
            return Err(ImageAdapterError::Closed);
        }
        if self.pending.is_some() {
            return Err(ImageAdapterError::Again);
        }
        Ok(now_ms)
    }

    pub(crate) fn acquire_owned_fill(
        &mut self,
        wire: &FileWire,
        byte_len: usize,
        now: Instant,
    ) -> Result<OwnedRingSlotFill, ImageAdapterError> {
        let now_ms = self.owned_fill_ready(wire, now)?;
        Ok(self
            .ring
            .as_mut()
            .ok_or(ImageAdapterError::Closed)?
            .acquire_owned_slot(byte_len, now_ms)?)
    }

    pub(crate) fn commit_owned_fill(
        &mut self,
        wire: &FileWire,
        fill: &mut OwnedRingSlotFill,
        upload: sophia_image_producer::RingUpload,
        now: Instant,
    ) -> Result<u64, ImageAdapterError> {
        let now_ms = self.owned_fill_ready(wire, now)?;
        let (offer, bytes) = self
            .ring
            .as_mut()
            .ok_or(ImageAdapterError::Closed)?
            .commit_owned_slot(fill, upload, now_ms)?;
        self.pending = Some(PendingControl {
            bytes,
            offer: Some(offer),
        });
        Ok(offer)
    }

    pub(crate) fn abort_owned_fill(
        &mut self,
        fill: &mut OwnedRingSlotFill,
    ) -> Result<(), ImageAdapterError> {
        Ok(self
            .ring
            .as_mut()
            .ok_or(ImageAdapterError::Closed)?
            .abort_owned_slot(fill)?)
    }
}
