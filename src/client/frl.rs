use hdmi_hal::scdc::ScdcTransport;

use crate::error::{ProtocolError, ScdcError};
use crate::register::address;
use crate::register::{FrlConfig, LtpReq, StatusFlags};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Writes FRL training configuration to `Config_0` (0x30).
    ///
    /// Encodes `FRL_Rate` into bits\[3:0\], `DSC_FRL_Max` into bit\[4\], and
    /// `FFE_Levels` into bits\[7:5\].
    pub fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), ScdcError<T::Error>> {
        let byte = (config.frl_rate as u8)
            | ((config.dsc_frl_max as u8) << 4)
            | ((config.ffe_levels as u8) << 5);
        self.transport
            .write(address::CONFIG_0, byte)
            .map_err(ScdcError::Transport)
    }

    /// Reads FRL status from `Status_Flags_0` (0x40) and `Status_Flags_1` (0x41).
    ///
    /// Returns [`crate::ProtocolError::UnknownLtpReq`] if the sink reports an
    /// LTP request value not defined by the HDMI 2.1 specification.
    pub fn read_status_flags(&mut self) -> Result<StatusFlags, ScdcError<T::Error>> {
        let flags0 = self
            .transport
            .read(address::STATUS_FLAGS_0)
            .map_err(ScdcError::Transport)?;
        let flags1 = self
            .transport
            .read(address::STATUS_FLAGS_1)
            .map_err(ScdcError::Transport)?;

        let ltp_req = match (flags1 >> 4) & 0x0F {
            0 => LtpReq::None,
            1 => LtpReq::Lfsr0,
            2 => LtpReq::Lfsr1,
            3 => LtpReq::Lfsr2,
            4 => LtpReq::Lfsr3,
            raw => return Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(raw))),
        };

        Ok(StatusFlags {
            clock_detected: flags0 & 0x01 != 0,
            cable_connected: flags0 & 0x02 != 0,
            ch0_locked: flags0 & 0x04 != 0,
            ch1_locked: flags0 & 0x08 != 0,
            ch2_locked: flags0 & 0x10 != 0,
            ch3_locked: flags0 & 0x20 != 0,
            flt_ready: flags0 & 0x40 != 0,
            frl_start: flags1 & 0x01 != 0,
            ltp_req,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::error::{ProtocolError, ScdcError};
    use crate::register::{FfeLevels, FrlConfig, LtpReq, StatusFlags};
    use display_types::HdmiForumFrl;

    #[test]
    fn frl_config_rate_field() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::Rate12Gbps4Lanes, // discriminant 6
            dsc_frl_max: false,
            ffe_levels: FfeLevels::Ffe0,
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x30), 0x06);
    }

    #[test]
    fn frl_config_dsc_frl_max_field() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::NotSupported,
            dsc_frl_max: true,
            ffe_levels: FfeLevels::Ffe0,
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x30), 0x10);
    }

    #[test]
    fn frl_config_ffe_levels_field() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::NotSupported,
            dsc_frl_max: false,
            ffe_levels: FfeLevels::Ffe7, // discriminant 7 → bits[7:5] = 0b111 = 0xE0
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x30), 0xE0);
    }

    #[test]
    fn status_flags_all_zero() {
        let mut scdc = Scdc::new(TestTransport::new());
        assert_eq!(
            scdc.read_status_flags().unwrap(),
            StatusFlags {
                clock_detected: false,
                cable_connected: false,
                ch0_locked: false,
                ch1_locked: false,
                ch2_locked: false,
                ch3_locked: false,
                flt_ready: false,
                frl_start: false,
                ltp_req: LtpReq::None,
            }
        );
    }

    #[test]
    fn status_flags_ltp_req_variants() {
        for (nibble, expected) in [
            (0u8, LtpReq::None),
            (1, LtpReq::Lfsr0),
            (2, LtpReq::Lfsr1),
            (3, LtpReq::Lfsr2),
            (4, LtpReq::Lfsr3),
        ] {
            let mut sim = TestTransport::new();
            sim.set(0x41, nibble << 4);
            assert_eq!(
                Scdc::new(sim).read_status_flags().unwrap().ltp_req,
                expected
            );
        }
    }

    #[test]
    fn status_flags_unknown_ltp_req() {
        for nibble in 5u8..=15 {
            let mut sim = TestTransport::new();
            sim.set(0x41, nibble << 4);
            assert!(matches!(
                Scdc::new(sim).read_status_flags(),
                Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(n))) if n == nibble
            ));
        }
    }

    #[test]
    fn status_flags_all_flags0_bits_set() {
        // Ensures the true-branch of every flags0 bit expression is exercised.
        let mut sim = TestTransport::new();
        sim.set(0x40, 0x7F); // clock_detected | cable_connected | ch0–ch3_locked | flt_ready
        sim.set(0x41, 0x01); // frl_start
        let f = Scdc::new(sim).read_status_flags().unwrap();
        assert!(f.clock_detected && f.cable_connected);
        assert!(f.ch0_locked && f.ch1_locked && f.ch2_locked && f.ch3_locked);
        assert!(f.flt_ready && f.frl_start);
        assert_eq!(f.ltp_req, LtpReq::None);
    }

    #[test]
    fn transport_error_propagates() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .write_frl_config(FrlConfig {
                    frl_rate: HdmiForumFrl::NotSupported,
                    dsc_frl_max: false,
                    ffe_levels: FfeLevels::Ffe0,
                })
                .is_err()
        );
        // First read of status flags fails.
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_status_flags()
                .is_err()
        );
        // Second read (Status_Flags_1) fails.
        assert!(
            Scdc::new(TestTransport::failing_after(1))
                .read_status_flags()
                .is_err()
        );
    }
}
