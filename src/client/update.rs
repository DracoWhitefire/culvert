use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::UpdateFlags;
use crate::register::address;

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Reads update flags from `Update_0` (0x10) and `Update_1` (0x11).
    pub fn read_update_flags(&mut self) -> Result<UpdateFlags, ScdcError<T::Error>> {
        let u0 = self
            .transport
            .read(address::UPDATE_0)
            .map_err(ScdcError::Transport)?;
        let u1 = self
            .transport
            .read(address::UPDATE_1)
            .map_err(ScdcError::Transport)?;
        Ok(UpdateFlags {
            status_update: u0 & 0x01 != 0,
            ced_update: u0 & 0x02 != 0,
            frl_update: u0 & 0x04 != 0,
            dsc_update: u1 & 0x01 != 0,
        })
    }

    /// Clears the specified update flags in `Update_0` (0x10) and `Update_1` (0x11).
    ///
    /// Each flag set to `true` in `flags` is cleared (write-1-to-clear). Flags
    /// set to `false` are left unchanged.
    pub fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), ScdcError<T::Error>> {
        let u0 = (flags.status_update as u8)
            | ((flags.ced_update as u8) << 1)
            | ((flags.frl_update as u8) << 2);
        let u1 = flags.dsc_update as u8;
        self.transport
            .write(address::UPDATE_0, u0)
            .map_err(ScdcError::Transport)?;
        self.transport
            .write(address::UPDATE_1, u1)
            .map_err(ScdcError::Transport)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::register::UpdateFlags;

    #[test]
    fn update_flags_individual_bits() {
        // status_update = bit 0 of Update_0
        let mut sim = TestTransport::new();
        sim.set(0x10, 0x01);
        assert!(Scdc::new(sim).read_update_flags().unwrap().status_update);

        // ced_update = bit 1
        let mut sim = TestTransport::new();
        sim.set(0x10, 0x02);
        assert!(Scdc::new(sim).read_update_flags().unwrap().ced_update);

        // frl_update = bit 2
        let mut sim = TestTransport::new();
        sim.set(0x10, 0x04);
        assert!(Scdc::new(sim).read_update_flags().unwrap().frl_update);

        // dsc_update = bit 0 of Update_1
        let mut sim = TestTransport::new();
        sim.set(0x11, 0x01);
        assert!(Scdc::new(sim).read_update_flags().unwrap().dsc_update);
    }

    #[test]
    fn clear_update_flags_w1c() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.clear_update_flags(UpdateFlags::new(true, true, true, true))
            .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x10), 0x07);
        assert_eq!(t.get(0x11), 0x01);
    }

    #[test]
    fn clear_update_flags_partial() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.clear_update_flags(UpdateFlags::new(false, true, false, false))
            .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x10), 0x02); // only ced_update bit
        assert_eq!(t.get(0x11), 0x00);
    }

    #[test]
    fn transport_error_propagates() {
        // First read fails.
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_update_flags()
                .is_err()
        );
        // Second read (Update_1) fails.
        assert!(
            Scdc::new(TestTransport::failing_after(1))
                .read_update_flags()
                .is_err()
        );
        // First write fails.
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .clear_update_flags(UpdateFlags::new(false, false, false, false))
                .is_err()
        );
        // Second write (Update_1) fails.
        assert!(
            Scdc::new(TestTransport::failing_after(1))
                .clear_update_flags(UpdateFlags::new(false, false, false, false))
                .is_err()
        );
    }
}
