//! `plumbob::ScdcClient` implementation for [`Scdc`].
//!
//! Each trait method calls one culvert method; culvert's register types are converted to
//! plumbob's at this boundary with the `From` impls below. culvert's own types are
//! unchanged.

use hdmi_hal::scdc::ScdcTransport;
use plumbob::ScdcClient;

use crate::error::ScdcError;
use crate::register::{
    CedCount, CedCounters, Config0, FfeLevels, FrlConfig, LtpReq, LtpRequests, SourceTestConfig,
    UpdateFlags,
};

use super::Scdc;

/// Typed SCDC access for plumbob's link training state machine.
///
/// `Scdc` does not wait between calls. plumbob polls `read_flt_ready` and
/// `read_update_flags` in a loop whose limits assume one poll every 2 ms by default;
/// enforce that interval in the transport (or a wrapper around it).
impl<T: ScdcTransport> ScdcClient for Scdc<T> {
    type Error = ScdcError<T::Error>;

    fn read_flt_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(Scdc::read_status_flags(self)?.flt_ready)
    }

    fn read_update_flags(&mut self) -> Result<plumbob::UpdateFlags, Self::Error> {
        Scdc::read_update_flags(self).map(Into::into)
    }

    fn clear_update_flags(&mut self, flags: plumbob::UpdateFlags) -> Result<(), Self::Error> {
        Scdc::clear_update_flags(self, flags.into())
    }

    fn read_ltp_requests(&mut self) -> Result<plumbob::LtpRequests, Self::Error> {
        Scdc::read_ltp_requests(self).map(Into::into)
    }

    fn read_source_test_config(&mut self) -> Result<plumbob::SourceTestConfig, Self::Error> {
        Scdc::read_source_test_config(self).map(Into::into)
    }

    fn write_config_0_defaults(&mut self) -> Result<(), Self::Error> {
        Scdc::write_config_0(self, Config0::default())
    }

    fn write_frl_config(&mut self, config: plumbob::FrlConfig) -> Result<(), Self::Error> {
        Scdc::write_frl_config(self, config.into())
    }

    fn read_ced(&mut self) -> Result<plumbob::CedCounters, Self::Error> {
        Scdc::read_ced(self).map(Into::into)
    }
}

impl From<LtpReq> for plumbob::LtpReq {
    fn from(req: LtpReq) -> Self {
        match req {
            LtpReq::None => Self::None,
            LtpReq::AllOnes => Self::AllOnes,
            LtpReq::AllZeros => Self::AllZeros,
            LtpReq::NyquistClock => Self::NyquistClock,
            LtpReq::RxDdeCompliance => Self::DdeCompliance,
            LtpReq::Lfsr0 => Self::Lfsr0,
            LtpReq::Lfsr1 => Self::Lfsr1,
            LtpReq::Lfsr2 => Self::Lfsr2,
            LtpReq::Lfsr3 => Self::Lfsr3,
            LtpReq::FfeChange => Self::FfeChange,
            LtpReq::RateChange => Self::RateChange,
            LtpReq::Reserved(value) => Self::Reserved(value),
        }
    }
}

impl From<LtpRequests> for plumbob::LtpRequests {
    fn from(requests: LtpRequests) -> Self {
        Self {
            lane0: requests.lane0.into(),
            lane1: requests.lane1.into(),
            lane2: requests.lane2.into(),
            lane3: requests.lane3.into(),
        }
    }
}

/// The `Update_0` flags plumbob uses; the others are not part of training.
impl From<UpdateFlags> for plumbob::UpdateFlags {
    fn from(flags: UpdateFlags) -> Self {
        Self {
            source_test_update: flags.source_test_update,
            frl_start: flags.frl_start,
            flt_update: flags.flt_update,
        }
    }
}

/// The flags plumbob clears; every other `Update_0` flag is left as it is.
impl From<plumbob::UpdateFlags> for UpdateFlags {
    fn from(flags: plumbob::UpdateFlags) -> Self {
        UpdateFlags::new(
            false,
            false,
            false,
            flags.source_test_update,
            flags.frl_start,
            flags.flt_update,
            false,
        )
    }
}

impl From<SourceTestConfig> for plumbob::SourceTestConfig {
    fn from(config: SourceTestConfig) -> Self {
        Self {
            flt_no_timeout: config.flt_no_timeout,
        }
    }
}

impl From<plumbob::FrlConfig> for FrlConfig {
    fn from(config: plumbob::FrlConfig) -> Self {
        FrlConfig {
            frl_rate: config.rate,
            // plumbob's levels are 0–7, which culvert's `FfeLevels` always accepts.
            ffe_levels: FfeLevels::new(config.ffe_levels.value()).unwrap_or_default(),
        }
    }
}

impl From<CedCount> for plumbob::CedCount {
    fn from(count: CedCount) -> Self {
        Self::new(count.value())
    }
}

impl From<CedCounters> for plumbob::CedCounters {
    fn from(counters: CedCounters) -> Self {
        Self {
            lane0: counters.lane0.map(Into::into),
            lane1: counters.lane1.map(Into::into),
            lane2: counters.lane2.map(Into::into),
            lane3: counters.lane3.map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::error::{ProtocolError, ScdcError};
    use display_types::HdmiForumFrl;
    use plumbob::{FfeLevels, FrlConfig, LtpReq, LtpRequests, ScdcClient, UpdateFlags};

    fn scdc_with(regs: &[(u8, u8)]) -> Scdc<TestTransport> {
        let mut t = TestTransport::new();
        for &(addr, value) in regs {
            t.set(addr, value);
        }
        Scdc::new(t)
    }

    // --- read_flt_ready: Status_Flags_0 (0x40) bit 6

    #[test]
    fn read_flt_ready_reads_status_flags_0_bit_6() {
        assert!(ScdcClient::read_flt_ready(&mut scdc_with(&[(0x40, 0x40)])).unwrap());
        assert!(!ScdcClient::read_flt_ready(&mut scdc_with(&[(0x40, 0xBF)])).unwrap());
    }

    // --- read_update_flags: Update_0 (0x10) bits 3–5

    #[test]
    fn read_update_flags_maps_the_training_flags() {
        let read = |byte| ScdcClient::read_update_flags(&mut scdc_with(&[(0x10, byte)])).unwrap();
        assert!(read(0x08).source_test_update);
        assert!(read(0x10).frl_start);
        assert!(read(0x20).flt_update);
        // Bits plumbob does not use are not reported.
        assert_eq!(read(0xC7), UpdateFlags::default());
    }

    // --- clear_update_flags: write 1 to clear, only the given flags

    #[test]
    fn clear_update_flags_writes_only_the_given_bits() {
        let clear = |flags| {
            let mut scdc = Scdc::new(TestTransport::new());
            ScdcClient::clear_update_flags(&mut scdc, flags).unwrap();
            scdc.into_transport().get(0x10)
        };
        let flt_update = UpdateFlags {
            flt_update: true,
            ..UpdateFlags::default()
        };
        let frl_start = UpdateFlags {
            frl_start: true,
            ..UpdateFlags::default()
        };
        let source_test_update = UpdateFlags {
            source_test_update: true,
            ..UpdateFlags::default()
        };
        assert_eq!(clear(flt_update), 0x20);
        assert_eq!(clear(frl_start), 0x10);
        assert_eq!(clear(source_test_update), 0x08);
        assert_eq!(clear(UpdateFlags::default()), 0x00);
    }

    // --- read_ltp_requests: Status_Flags_1/2 (0x41/0x42), one nibble per lane

    #[test]
    fn read_ltp_requests_maps_each_lane() {
        let mut scdc = scdc_with(&[(0x41, 0x65), (0x42, 0x87)]);
        assert_eq!(
            ScdcClient::read_ltp_requests(&mut scdc).unwrap(),
            LtpRequests {
                lane0: LtpReq::Lfsr0,
                lane1: LtpReq::Lfsr1,
                lane2: LtpReq::Lfsr2,
                lane3: LtpReq::Lfsr3,
            }
        );
    }

    #[test]
    fn read_ltp_requests_maps_every_request_value() {
        let cases = [
            (0x0, LtpReq::None),
            (0x1, LtpReq::AllOnes),
            (0x2, LtpReq::AllZeros),
            (0x3, LtpReq::NyquistClock),
            (0x4, LtpReq::DdeCompliance),
            (0x5, LtpReq::Lfsr0),
            (0x6, LtpReq::Lfsr1),
            (0x7, LtpReq::Lfsr2),
            (0x8, LtpReq::Lfsr3),
            (0xE, LtpReq::FfeChange),
            (0xF, LtpReq::RateChange),
        ];
        for (nibble, expected) in cases {
            let requests =
                ScdcClient::read_ltp_requests(&mut scdc_with(&[(0x41, nibble)])).unwrap();
            assert_eq!(requests.lane0, expected, "{nibble:#x}");
        }
    }

    #[test]
    fn read_ltp_requests_passes_undefined_values_on() {
        let requests = ScdcClient::read_ltp_requests(&mut scdc_with(&[(0x42, 0x90)])).unwrap();
        assert_eq!(requests.lane3, LtpReq::Reserved(0x9));
        assert_eq!(requests.lane2, LtpReq::None);
    }

    // --- read_source_test_config: Source_Test_Configuration (0x35) bit 5

    #[test]
    fn read_source_test_config_maps_flt_no_timeout() {
        let read = |byte| {
            ScdcClient::read_source_test_config(&mut scdc_with(&[(0x35, byte)]))
                .unwrap()
                .flt_no_timeout
        };
        assert!(read(0x20));
        assert!(!read(0xDF));
    }

    // --- write_config_0_defaults: Config_0 (0x30) cleared

    #[test]
    fn write_config_0_defaults_clears_config_0() {
        let mut scdc = scdc_with(&[(0x30, 0xFF)]);
        ScdcClient::write_config_0_defaults(&mut scdc).unwrap();
        assert_eq!(scdc.into_transport().get(0x30), 0x00);
    }

    // --- write_frl_config: Config_1 (0x31), rate in bits 3:0, FFE levels in bits 7:4

    #[test]
    fn write_frl_config_encodes_rate_and_ffe_levels() {
        let mut scdc = Scdc::new(TestTransport::new());
        ScdcClient::write_frl_config(
            &mut scdc,
            FrlConfig {
                rate: HdmiForumFrl::Rate12Gbps4Lanes,
                ffe_levels: FfeLevels::new(3).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x31), 0x36);
    }

    #[test]
    fn write_frl_config_turns_frl_off() {
        let mut scdc = scdc_with(&[(0x31, 0x36)]);
        ScdcClient::write_frl_config(
            &mut scdc,
            FrlConfig {
                rate: HdmiForumFrl::NotSupported,
                ffe_levels: FfeLevels::default(),
            },
        )
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x31), 0x00);
    }

    #[test]
    fn write_frl_config_keeps_culverts_rate_check() {
        // plumbob limits levels per rate itself; culvert still rejects a level above it.
        let mut scdc = Scdc::new(TestTransport::new());
        let result = ScdcClient::write_frl_config(
            &mut scdc,
            FrlConfig {
                rate: HdmiForumFrl::Rate6Gbps4Lanes,
                ffe_levels: FfeLevels::MAX,
            },
        );
        assert!(matches!(
            result,
            Err(ScdcError::Protocol(ProtocolError::FfeLevelsOutOfRange {
                levels: 7,
                ..
            }))
        ));
    }

    // --- read_ced: ERR_DET, validity bit 7 of each high byte

    #[test]
    fn read_ced_maps_each_lane() {
        let mut scdc = scdc_with(&[
            (0x50, 0x34),
            (0x51, 0x92),
            (0x52, 0x01),
            (0x53, 0x00),
            (0x54, 0xFF),
            (0x55, 0xFF),
            (0x57, 0x00),
            (0x58, 0x80),
        ]);
        let ced = ScdcClient::read_ced(&mut scdc).unwrap();
        assert_eq!(ced.lane0.map(|c| c.value()), Some(0x1234));
        assert_eq!(ced.lane1, None);
        assert_eq!(ced.lane2.map(|c| c.value()), Some(0x7FFF));
        assert_eq!(ced.lane3.map(|c| c.value()), Some(0));
    }

    // --- transport errors

    #[test]
    fn transport_errors_are_returned() {
        let fails = || Scdc::new(TestTransport::failing_after(0));
        assert!(matches!(
            ScdcClient::read_flt_ready(&mut fails()),
            Err(ScdcError::Transport(()))
        ));
        assert!(ScdcClient::read_update_flags(&mut fails()).is_err());
        assert!(ScdcClient::clear_update_flags(&mut fails(), UpdateFlags::default()).is_err());
        assert!(ScdcClient::read_ltp_requests(&mut fails()).is_err());
        assert!(ScdcClient::read_source_test_config(&mut fails()).is_err());
        assert!(ScdcClient::write_config_0_defaults(&mut fails()).is_err());
        assert!(
            ScdcClient::write_frl_config(
                &mut fails(),
                FrlConfig {
                    rate: HdmiForumFrl::Rate6Gbps4Lanes,
                    ffe_levels: FfeLevels::default(),
                }
            )
            .is_err()
        );
        assert!(ScdcClient::read_ced(&mut fails()).is_err());
    }
}
