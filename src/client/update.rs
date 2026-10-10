use hdmi_hal::scdc::ScdcTransport;

use crate::codec;
use crate::error::ScdcError;
use crate::register::{ClearableUpdateFlags, UpdateFlags};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Reads update flags from `Update_0` (0x10).
    pub fn read_update_flags(&mut self) -> Result<UpdateFlags, ScdcError<T::Error>> {
        let update_0 = self
            .transport
            .read(codec::UPDATE_0)
            .map_err(ScdcError::Transport)?;
        Ok(codec::decode_update_flags(update_0))
    }

    /// Clears the specified update flags in `Update_0` (0x10).
    ///
    /// Each flag set to `true` in `flags` is cleared (write-1-to-clear). Flags
    /// set to `false` are left unchanged, and so is `RR_Test`, which the source does not
    /// clear ([`ClearableUpdateFlags`] has no field for it). To acknowledge every flag just
    /// read, pass [`UpdateFlags::clearable`].
    pub fn clear_update_flags(
        &mut self,
        flags: ClearableUpdateFlags,
    ) -> Result<(), ScdcError<T::Error>> {
        let byte = codec::encode_clear_update_flags(flags);
        self.transport
            .write(codec::UPDATE_0, byte)
            .map_err(ScdcError::Transport)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::register::{ClearableUpdateFlags, UpdateFlags};

    fn read(byte: u8) -> UpdateFlags {
        let mut sim = TestTransport::new();
        sim.set(0x10, byte);
        Scdc::new(sim).read_update_flags().unwrap()
    }

    #[test]
    fn update_flags_individual_bits() {
        assert!(read(0x01).status_update);
        assert!(read(0x02).ced_update);
        assert!(read(0x04).rr_test);
        assert!(read(0x08).source_test_update);
        assert!(read(0x10).frl_start);
        assert!(read(0x20).flt_update);
        assert!(read(0x40).rsed_update);
        assert_eq!(
            read(0x00),
            UpdateFlags::new(false, false, false, false, false, false, false)
        );
    }

    #[test]
    fn update_1_is_not_read() {
        let mut sim = TestTransport::new();
        sim.set(0x11, 0xFF);
        assert_eq!(
            Scdc::new(sim).read_update_flags().unwrap(),
            UpdateFlags::new(false, false, false, false, false, false, false)
        );
    }

    #[test]
    fn clear_update_flags_w1c() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.clear_update_flags(ClearableUpdateFlags::new(
            true, true, true, true, true, true,
        ))
        .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x10), 0x7B); // every flag except RR_Test (bit 2)
        assert_eq!(t.get(0x11), 0x00);
    }

    #[test]
    fn read_then_clear_acknowledges_every_flag_but_rr_test() {
        // A sink under read-request test keeps RR_Test set; clearing what was read must
        // still acknowledge the other flags.
        let mut sim = TestTransport::new();
        sim.set(0x10, 0x24); // RR_Test | FLT_update
        let mut scdc = Scdc::new(sim);
        let flags = scdc.read_update_flags().unwrap();
        assert!(flags.rr_test && flags.flt_update);
        scdc.clear_update_flags(flags.clearable()).unwrap();
        assert_eq!(scdc.into_transport().get(0x10), 0x20);
    }

    #[test]
    fn clear_update_flags_partial() {
        let mut scdc = Scdc::new(TestTransport::new());
        // Clear only flt_update, as the training loop does after reading LTP requests.
        scdc.clear_update_flags(ClearableUpdateFlags::new(
            false, false, false, false, true, false,
        ))
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x10), 0x20);
    }

    #[test]
    fn transport_error_propagates() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_update_flags()
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .clear_update_flags(ClearableUpdateFlags::default())
                .is_err()
        );
    }
}
