use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::address;
use crate::register::{ScramblerStatus, TmdsConfig};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Writes scrambling configuration to `TMDS_Config` (0x20).
    ///
    /// Sets `Scrambling_Enable` (bit 0) and `TMDS_Bit_Clock_Ratio` (bit 1).
    pub fn write_tmds_config(&mut self, config: TmdsConfig) -> Result<(), ScdcError<T::Error>> {
        let byte = (config.scrambling_enable as u8) | ((config.high_tmds_clock_ratio as u8) << 1);
        self.transport
            .write(address::TMDS_CONFIG, byte)
            .map_err(ScdcError::Transport)
    }

    /// Reads scrambler acknowledgement from `Scrambler_Status` (0x21).
    ///
    /// Returns [`ScramblerStatus::scrambling_active`] set when the sink confirms
    /// that TMDS scrambling is active (bit 0).
    pub fn read_scrambler_status(&mut self) -> Result<ScramblerStatus, ScdcError<T::Error>> {
        let byte = self
            .transport
            .read(address::SCRAMBLER_STATUS)
            .map_err(ScdcError::Transport)?;
        Ok(ScramblerStatus {
            scrambling_active: byte & 0x01 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::register::{ScramblerStatus, TmdsConfig};

    #[test]
    fn tmds_config_scrambling_only() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_tmds_config(TmdsConfig {
            scrambling_enable: true,
            high_tmds_clock_ratio: false,
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x01);
    }

    #[test]
    fn tmds_config_clock_ratio_only() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_tmds_config(TmdsConfig {
            scrambling_enable: false,
            high_tmds_clock_ratio: true,
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x02);
    }

    #[test]
    fn tmds_config_both_clear() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_tmds_config(TmdsConfig {
            scrambling_enable: false,
            high_tmds_clock_ratio: false,
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x00);
    }

    #[test]
    fn scrambler_status_bit0_set() {
        let mut sim = TestTransport::new();
        sim.set(0x21, 0xFF); // all bits set; only bit 0 is defined
        assert_eq!(
            Scdc::new(sim).read_scrambler_status().unwrap(),
            ScramblerStatus {
                scrambling_active: true
            }
        );
    }

    #[test]
    fn scrambler_status_bit0_clear() {
        let mut sim = TestTransport::new();
        sim.set(0x21, 0xFE); // bit 0 clear, other bits set (reserved)
        assert_eq!(
            Scdc::new(sim).read_scrambler_status().unwrap(),
            ScramblerStatus {
                scrambling_active: false
            }
        );
    }

    #[test]
    fn transport_error_propagates() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .write_tmds_config(TmdsConfig {
                    scrambling_enable: false,
                    high_tmds_clock_ratio: false
                })
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_scrambler_status()
                .is_err()
        );
    }
}
