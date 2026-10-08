use hdmi_hal::scdc::ScdcTransport;

use crate::error::{ProtocolError, ScdcError};
use crate::register::address;
use crate::register::{Config0, FrlConfig, LtpReq, LtpRequests, SourceTestConfig, StatusFlags};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Writes FRL training configuration to `Config_1` (0x31).
    ///
    /// Encodes `FRL_Rate` into bits\[3:0\] and `FFE_Levels` into bits\[7:4\].
    pub fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), ScdcError<T::Error>> {
        let byte = (config.frl_rate as u8) | (config.ffe_levels.value() << 4);
        self.transport
            .write(address::CONFIG_1, byte)
            .map_err(ScdcError::Transport)
    }

    /// Writes `Config_0` (0x30): `RR_Enable` (bit 0) and `FLT_No_Retrain` (bit 1).
    pub fn write_config_0(&mut self, config: Config0) -> Result<(), ScdcError<T::Error>> {
        let byte = (config.rr_enable as u8) | ((config.flt_no_retrain as u8) << 1);
        self.transport
            .write(address::CONFIG_0, byte)
            .map_err(ScdcError::Transport)
    }

    /// Reads `Source_Test_Configuration` (0x35), written by the sink to instruct the
    /// source during compliance testing.
    pub fn read_source_test_config(&mut self) -> Result<SourceTestConfig, ScdcError<T::Error>> {
        let byte = self
            .transport
            .read(address::SOURCE_TEST_CONFIG)
            .map_err(ScdcError::Transport)?;
        Ok(SourceTestConfig {
            flt_no_timeout: byte & 0x20 != 0,
            dsc_frl_max: byte & 0x40 != 0,
        })
    }

    /// Reads `Status_Flags_0` (0x40): clock detection, lane lock, `FLT_Ready` and DSC
    /// decode failure.
    pub fn read_status_flags(&mut self) -> Result<StatusFlags, ScdcError<T::Error>> {
        let flags0 = self
            .transport
            .read(address::STATUS_FLAGS_0)
            .map_err(ScdcError::Transport)?;
        Ok(StatusFlags {
            clock_detected: flags0 & 0x01 != 0,
            ch0_locked: flags0 & 0x02 != 0,
            ch1_locked: flags0 & 0x04 != 0,
            ch2_locked: flags0 & 0x08 != 0,
            ln3_locked: flags0 & 0x10 != 0,
            flt_ready: flags0 & 0x40 != 0,
            dsc_decode_fail: flags0 & 0x80 != 0,
        })
    }

    /// Reads the per-lane link training pattern requests from `Status_Flags_1` (0x41,
    /// lanes 0–1) and `Status_Flags_2` (0x42, lanes 2–3).
    ///
    /// Returns [`crate::ProtocolError::UnknownLtpReq`] if any lane reports a value not
    /// defined by the HDMI 2.1 specification.
    pub fn read_ltp_requests(&mut self) -> Result<LtpRequests, ScdcError<T::Error>> {
        let flags1 = self
            .transport
            .read(address::STATUS_FLAGS_1)
            .map_err(ScdcError::Transport)?;
        let flags2 = self
            .transport
            .read(address::STATUS_FLAGS_2)
            .map_err(ScdcError::Transport)?;
        let decode = |nibble: u8| {
            LtpReq::from_nibble(nibble)
                .ok_or(ScdcError::Protocol(ProtocolError::UnknownLtpReq(nibble)))
        };
        Ok(LtpRequests {
            lane0: decode(flags1 & 0x0F)?,
            lane1: decode(flags1 >> 4)?,
            lane2: decode(flags2 & 0x0F)?,
            lane3: decode(flags2 >> 4)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::error::{ProtocolError, ScdcError};
    use crate::register::{Config0, FfeLevels, FrlConfig, LtpReq, SourceTestConfig, StatusFlags};
    use display_types::HdmiForumFrl;

    #[test]
    fn frl_config_rate_field() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::Rate12Gbps4Lanes, // discriminant 6
            ffe_levels: FfeLevels::default(),
        })
        .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x31), 0x06);
        assert_eq!(t.get(0x30), 0x00); // Config_0 untouched
    }

    #[test]
    fn frl_config_ffe_levels_field() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::NotSupported,
            ffe_levels: FfeLevels::new(0xF).unwrap(), // → bits[7:4]
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x31), 0xF0);
    }

    #[test]
    fn ffe_levels_range() {
        assert_eq!(FfeLevels::new(15).map(FfeLevels::value), Some(15));
        assert_eq!(FfeLevels::new(16), None);
    }

    #[test]
    fn status_flags_all_zero() {
        let f = Scdc::new(TestTransport::new()).read_status_flags().unwrap();
        assert_eq!(
            f,
            StatusFlags {
                clock_detected: false,
                ch0_locked: false,
                ch1_locked: false,
                ch2_locked: false,
                ln3_locked: false,
                flt_ready: false,
                dsc_decode_fail: false,
            }
        );
    }

    #[test]
    fn status_flags_individual_bits() {
        let read = |byte: u8| {
            let mut sim = TestTransport::new();
            sim.set(0x40, byte);
            Scdc::new(sim).read_status_flags().unwrap()
        };
        assert!(read(0x01).clock_detected);
        assert!(read(0x02).ch0_locked);
        assert!(read(0x04).ch1_locked);
        assert!(read(0x08).ch2_locked);
        assert!(read(0x10).ln3_locked);
        assert!(read(0x40).flt_ready);
        assert!(read(0x80).dsc_decode_fail);
        // Bit 5 is not defined; setting it alone sets no field.
        assert_eq!(read(0x20), read(0x00));
    }

    #[test]
    fn status_flags_reads_only_status_flags_0() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 0xFF);
        sim.set(0x42, 0xFF);
        let f = Scdc::new(sim).read_status_flags().unwrap();
        assert!(!f.flt_ready && !f.clock_detected);
    }

    #[test]
    fn ltp_requests_per_lane() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 0x65); // lane0 = 0x5 (LFSR0), lane1 = 0x6 (LFSR1)
        sim.set(0x42, 0xF1); // lane2 = 0x1 (all ones), lane3 = 0xF (rate change)
        let r = Scdc::new(sim).read_ltp_requests().unwrap();
        assert_eq!(r.lane0, LtpReq::Lfsr0);
        assert_eq!(r.lane1, LtpReq::Lfsr1);
        assert_eq!(r.lane2, LtpReq::AllOnes);
        assert_eq!(r.lane3, LtpReq::RateChange);
        assert!(!r.all_trained());
    }

    #[test]
    fn ltp_requests_all_defined_values() {
        for (nibble, req) in [
            (0x0u8, LtpReq::None),
            (0x1, LtpReq::AllOnes),
            (0x2, LtpReq::AllZeros),
            (0x3, LtpReq::NyquistClock),
            (0x4, LtpReq::RxDdeCompliance),
            (0x5, LtpReq::Lfsr0),
            (0x6, LtpReq::Lfsr1),
            (0x7, LtpReq::Lfsr2),
            (0x8, LtpReq::Lfsr3),
            (0xE, LtpReq::FfeChange),
            (0xF, LtpReq::RateChange),
        ] {
            let mut sim = TestTransport::new();
            sim.set(0x42, nibble << 4); // lane 3
            assert_eq!(Scdc::new(sim).read_ltp_requests().unwrap().lane3, req);
        }
    }

    #[test]
    fn ltp_requests_unknown_value() {
        for nibble in 0x9u8..=0xD {
            let mut sim = TestTransport::new();
            sim.set(0x41, nibble); // lane 0
            assert!(matches!(
                Scdc::new(sim).read_ltp_requests(),
                Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(n))) if n == nibble
            ));
        }
    }

    #[test]
    fn ltp_requests_all_trained_when_zero() {
        assert!(
            Scdc::new(TestTransport::new())
                .read_ltp_requests()
                .unwrap()
                .all_trained()
        );
    }

    #[test]
    fn config_0_bits() {
        for (config, expected) in [
            (Config0::default(), 0x00u8),
            (
                Config0 {
                    rr_enable: true,
                    flt_no_retrain: false,
                },
                0x01,
            ),
            (
                Config0 {
                    rr_enable: false,
                    flt_no_retrain: true,
                },
                0x02,
            ),
            (
                Config0 {
                    rr_enable: true,
                    flt_no_retrain: true,
                },
                0x03,
            ),
        ] {
            let mut scdc = Scdc::new(TestTransport::new());
            scdc.write_config_0(config).unwrap();
            assert_eq!(scdc.into_transport().get(0x30), expected, "{config:?}");
        }
    }

    #[test]
    fn source_test_config_bits() {
        let read = |byte: u8| {
            let mut sim = TestTransport::new();
            sim.set(0x35, byte);
            Scdc::new(sim).read_source_test_config().unwrap()
        };
        assert_eq!(read(0x00), SourceTestConfig::new(false, false));
        assert_eq!(read(0x20), SourceTestConfig::new(true, false));
        assert_eq!(read(0x40), SourceTestConfig::new(false, true));
        // Other bits are not defined and set no field.
        assert_eq!(read(0x9F), SourceTestConfig::new(false, false));
    }

    #[test]
    fn transport_error_propagates() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .write_frl_config(FrlConfig {
                    frl_rate: HdmiForumFrl::NotSupported,
                    ffe_levels: FfeLevels::default(),
                })
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_status_flags()
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .write_config_0(Config0::default())
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_source_test_config()
                .is_err()
        );
        // First (Status_Flags_1) and second (Status_Flags_2) read of the LTP requests.
        for n in 0..2 {
            assert!(
                Scdc::new(TestTransport::failing_after(n))
                    .read_ltp_requests()
                    .is_err()
            );
        }
    }
}
