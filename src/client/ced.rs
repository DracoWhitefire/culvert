use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::address;
use crate::register::{CedCount, CedCounters, RsCorrectionCount};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Reads per-lane character error counts from `ERR_DET` registers (0x50–0x55 for
    /// lanes 0–2, 0x57–0x58 for lane 3; 0x56 is the CED checksum).
    ///
    /// Each lane's counter is decoded from a low/high byte pair. The high byte's
    /// bit 7 is a validity flag; if it is not set the lane's counter is `None`.
    pub fn read_ced(&mut self) -> Result<CedCounters, ScdcError<T::Error>> {
        let l0 = self
            .transport
            .read(address::ERR_DET_0_L)
            .map_err(ScdcError::Transport)?;
        let h0 = self
            .transport
            .read(address::ERR_DET_0_H)
            .map_err(ScdcError::Transport)?;
        let l1 = self
            .transport
            .read(address::ERR_DET_1_L)
            .map_err(ScdcError::Transport)?;
        let h1 = self
            .transport
            .read(address::ERR_DET_1_H)
            .map_err(ScdcError::Transport)?;
        let l2 = self
            .transport
            .read(address::ERR_DET_2_L)
            .map_err(ScdcError::Transport)?;
        let h2 = self
            .transport
            .read(address::ERR_DET_2_H)
            .map_err(ScdcError::Transport)?;
        let l3 = self
            .transport
            .read(address::ERR_DET_3_L)
            .map_err(ScdcError::Transport)?;
        let h3 = self
            .transport
            .read(address::ERR_DET_3_H)
            .map_err(ScdcError::Transport)?;

        let decode = |lo: u8, hi: u8| -> Option<CedCount> {
            (hi & 0x80 != 0).then(|| CedCount::new(((hi as u16) << 8) | lo as u16))
        };

        Ok(CedCounters {
            lane0: decode(l0, h0),
            lane1: decode(l1, h1),
            lane2: decode(l2, h2),
            lane3: decode(l3, h3),
        })
    }

    /// Reads the Reed-Solomon correction count from `RS_Correction_L/H` (0x59/0x5A).
    ///
    /// Returns `None` if the high byte's validity bit (bit 7) is not set. The count is
    /// maintained by the sink in FRL mode; `UpdateFlags::rsed_update` signals a change.
    pub fn read_rs_correction(&mut self) -> Result<Option<RsCorrectionCount>, ScdcError<T::Error>> {
        let lo = self
            .transport
            .read(address::RS_CORRECTION_L)
            .map_err(ScdcError::Transport)?;
        let hi = self
            .transport
            .read(address::RS_CORRECTION_H)
            .map_err(ScdcError::Transport)?;
        Ok((hi & 0x80 != 0).then(|| RsCorrectionCount::new(((hi as u16) << 8) | lo as u16)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::register::CedCount;

    #[test]
    fn ced_validity_bit_required() {
        // High byte with bit 7 clear → None regardless of counter value.
        let mut sim = TestTransport::new();
        sim.set(0x50, 0xFF);
        sim.set(0x51, 0x7F); // valid bit clear
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, None);
    }

    #[test]
    fn ced_counter_validity_bit_stripped() {
        // High byte: bit 7 set (valid), counter bits = 0x01; low byte = 0x23.
        let mut sim = TestTransport::new();
        sim.set(0x50, 0x23);
        sim.set(0x51, 0x81);
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, Some(CedCount::new(0x0123)));
    }

    #[test]
    fn ced_lane3_independent() {
        let mut sim = TestTransport::new();
        sim.set(0x56, 0xAA); // CED checksum, must not be read as lane 3
        sim.set(0x57, 0x01);
        sim.set(0x58, 0x80); // lane3 valid, count = 1
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, None);
        assert_eq!(ced.lane3.map(|c| c.value()), Some(0x0001));
    }

    #[test]
    fn transport_error_on_any_read() {
        for n in 0..8 {
            assert!(
                Scdc::new(TestTransport::failing_after(n))
                    .read_ced()
                    .is_err()
            );
        }
    }

    #[test]
    fn rs_correction_valid_and_invalid() {
        let mut sim = TestTransport::new();
        sim.set(0x59, 0x34);
        sim.set(0x5A, 0x81); // valid, upper counter bits = 0x01
        assert_eq!(
            Scdc::new(sim)
                .read_rs_correction()
                .unwrap()
                .map(|c| c.value()),
            Some(0x0134)
        );

        let mut sim = TestTransport::new();
        sim.set(0x59, 0xFF);
        sim.set(0x5A, 0x7F); // validity bit clear
        assert_eq!(Scdc::new(sim).read_rs_correction().unwrap(), None);
    }

    #[test]
    fn rs_correction_transport_error() {
        for n in 0..2 {
            assert!(
                Scdc::new(TestTransport::failing_after(n))
                    .read_rs_correction()
                    .is_err()
            );
        }
    }
}
