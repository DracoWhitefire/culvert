use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::address;
use crate::register::{CedCount, CedCounters};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Reads per-lane character error counts from `ERR_DET` registers (0x50–0x57).
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
        sim.set(0x56, 0x01);
        sim.set(0x57, 0x80); // lane3 valid, count = 1
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
}
