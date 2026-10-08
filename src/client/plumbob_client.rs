//! `plumbob::ScdcClient` implementation for [`Scdc`].

use hdmi_hal::scdc::ScdcTransport;
use plumbob::ScdcClient;

use crate::error::{ProtocolError, ScdcError};
use crate::register::{CedCount, FfeLevels, FrlConfig, LtpReq, LtpRequests};

use super::Scdc;

impl<T: ScdcTransport> ScdcClient for Scdc<T> {
    type Error = ScdcError<T::Error>;

    fn write_frl_config(&mut self, config: plumbob::FrlConfig) -> Result<(), Self::Error> {
        // plumbob 0.1's `dsc_frl_max` has no place in `Config_1`: `DSC_FRL_Max` is a
        // sink-written test flag in `Source_Test_Configuration` (0x35). It is ignored.
        self.write_frl_config(FrlConfig {
            frl_rate: config.rate,
            ffe_levels: ffe_levels(config.ffe_levels),
        })
    }

    fn read_training_status(&mut self) -> Result<plumbob::TrainingStatus, Self::Error> {
        let flags = self.read_status_flags()?;
        let updates = self.read_update_flags()?;
        let requests = self.read_ltp_requests()?;
        Ok(plumbob::TrainingStatus {
            flt_ready: flags.flt_ready,
            frl_start: updates.frl_start,
            ltp_req: ltp_req(requests).map_err(ScdcError::Protocol)?,
        })
    }

    fn read_ced(&mut self) -> Result<plumbob::CedCounters, Self::Error> {
        let c = self.read_ced()?;
        Ok(plumbob::CedCounters {
            lane0: c.lane0.map(ced_count),
            lane1: c.lane1.map(ced_count),
            lane2: c.lane2.map(ced_count),
            lane3: c.lane3.map(ced_count),
        })
    }
}

fn ffe_levels(f: plumbob::FfeLevels) -> FfeLevels {
    // plumbob 0.1 levels are 0–7, which always fit the 4-bit field.
    FfeLevels::new(f as u8).unwrap_or_default()
}

/// Projects the per-lane requests onto plumbob 0.1's single request.
///
/// Interim mapping until plumbob handles per-lane requests: plumbob 0.1 passes its
/// request to the PHY as the raw pattern number, and its variants cover the raw values
/// 0–4. Requests are forwarded only when all lanes agree on such a value; anything
/// else is reported instead of training with a wrong pattern.
fn ltp_req(requests: LtpRequests) -> Result<plumbob::LtpReq, ProtocolError> {
    let lanes = [
        requests.lane0,
        requests.lane1,
        requests.lane2,
        requests.lane3,
    ];
    let unsupported = || ProtocolError::UnsupportedLtpRequests(requests);
    if lanes.iter().any(|l| *l != lanes[0]) {
        return Err(unsupported());
    }
    Ok(match lanes[0] {
        LtpReq::None => plumbob::LtpReq::None,
        LtpReq::AllOnes => plumbob::LtpReq::Lfsr0, // raw 1
        LtpReq::AllZeros => plumbob::LtpReq::Lfsr1, // raw 2
        LtpReq::NyquistClock => plumbob::LtpReq::Lfsr2, // raw 3
        LtpReq::RxDdeCompliance => plumbob::LtpReq::Lfsr3, // raw 4
        _ => return Err(unsupported()),
    })
}

fn ced_count(c: CedCount) -> plumbob::CedCount {
    plumbob::CedCount::new(c.value())
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use crate::error::{ProtocolError, ScdcError};
    use display_types::HdmiForumFrl;
    use plumbob::{FfeLevels, FrlConfig, LtpReq, ScdcClient};

    // --- write_frl_config ---

    #[test]
    fn write_frl_config_rate_and_ffe() {
        // Rate12Gbps4Lanes = discriminant 6; Ffe3 = 3 → bits[7:4] = 0x30
        let mut scdc = Scdc::new(TestTransport::new());
        ScdcClient::write_frl_config(
            &mut scdc,
            FrlConfig {
                rate: HdmiForumFrl::Rate12Gbps4Lanes,
                ffe_levels: FfeLevels::Ffe3,
                dsc_frl_max: false,
            },
        )
        .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x31), 0x36);
        assert_eq!(t.get(0x30), 0x00);
    }

    #[test]
    fn write_frl_config_ignores_dsc_frl_max() {
        let mut scdc = Scdc::new(TestTransport::new());
        ScdcClient::write_frl_config(
            &mut scdc,
            FrlConfig {
                rate: HdmiForumFrl::NotSupported,
                ffe_levels: FfeLevels::Ffe0,
                dsc_frl_max: true,
            },
        )
        .unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x31), 0x00);
        assert_eq!(t.get(0x35), 0x00);
    }

    #[test]
    fn write_frl_config_all_ffe_levels() {
        use FfeLevels::*;
        for (level, expected) in [
            (Ffe0, 0x00u8),
            (Ffe1, 0x10),
            (Ffe2, 0x20),
            (Ffe3, 0x30),
            (Ffe4, 0x40),
            (Ffe5, 0x50),
            (Ffe6, 0x60),
            (Ffe7, 0x70),
        ] {
            let mut scdc = Scdc::new(TestTransport::new());
            ScdcClient::write_frl_config(
                &mut scdc,
                FrlConfig {
                    rate: HdmiForumFrl::NotSupported,
                    ffe_levels: level,
                    dsc_frl_max: false,
                },
            )
            .unwrap();
            assert_eq!(scdc.into_transport().get(0x31), expected, "{level:?}");
        }
    }

    #[test]
    fn read_training_status_flt_ready() {
        let mut sim = TestTransport::new();
        sim.set(0x40, 0x40); // Status_Flags_0: FLT_Ready (bit 6)
        let status = Scdc::new(sim).read_training_status().unwrap();
        assert!(status.flt_ready);
        assert!(!status.frl_start);
        assert_eq!(status.ltp_req, LtpReq::None);
    }

    #[test]
    fn read_training_status_frl_start() {
        let mut sim = TestTransport::new();
        sim.set(0x10, 0x10); // Update_0: FRL_Start (bit 4)
        let status = Scdc::new(sim).read_training_status().unwrap();
        assert!(!status.flt_ready);
        assert!(status.frl_start);
    }

    #[test]
    fn read_training_status_ltp_req_passes_raw_value() {
        // plumbob 0.1 forwards its request to the PHY as the raw pattern number.
        for (raw, expected) in [
            (0u8, LtpReq::None),
            (1, LtpReq::Lfsr0),
            (2, LtpReq::Lfsr1),
            (3, LtpReq::Lfsr2),
            (4, LtpReq::Lfsr3),
        ] {
            let mut sim = TestTransport::new();
            sim.set(0x41, raw | (raw << 4));
            sim.set(0x42, raw | (raw << 4));
            let status = Scdc::new(sim).read_training_status().unwrap();
            assert_eq!(status.ltp_req, expected, "raw={raw}");
            assert_eq!(status.ltp_req as u8, raw);
        }
    }

    #[test]
    fn read_training_status_differing_lanes_is_error() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 0x11); // lanes 0/1 = all ones
        sim.set(0x42, 0x21); // lane 2 = all ones, lane 3 = all zeros
        assert!(matches!(
            Scdc::new(sim).read_training_status(),
            Err(ScdcError::Protocol(ProtocolError::UnsupportedLtpRequests(
                _
            )))
        ));
    }

    #[test]
    fn read_training_status_unrepresentable_ltp_req_is_error() {
        // LFSR 0 (0x5), FFE change (0xE) and rate change (0xF) on every lane.
        for raw in [0x5u8, 0xE, 0xF] {
            let mut sim = TestTransport::new();
            sim.set(0x41, raw | (raw << 4));
            sim.set(0x42, raw | (raw << 4));
            assert!(matches!(
                Scdc::new(sim).read_training_status(),
                Err(ScdcError::Protocol(ProtocolError::UnsupportedLtpRequests(
                    _
                )))
            ));
        }
    }

    #[test]
    fn read_training_status_unknown_ltp_req_is_error() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 0x09); // lane 0 = 0x9, undefined by the spec
        assert!(matches!(
            Scdc::new(sim).read_training_status(),
            Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(9)))
        ));
    }

    // --- read_ced ---

    #[test]
    fn read_ced_lane0_valid() {
        let mut sim = TestTransport::new();
        sim.set(0x50, 0x23); // low byte
        sim.set(0x51, 0x81); // high byte: validity bit set, counter hi = 0x01
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0.map(|c| c.value()), Some(0x0123));
    }

    #[test]
    fn read_ced_lane_invalid_when_validity_clear() {
        let mut sim = TestTransport::new();
        sim.set(0x50, 0xFF);
        sim.set(0x51, 0x7F); // validity bit clear
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, None);
    }

    #[test]
    fn read_ced_all_lanes_propagate() {
        let mut sim = TestTransport::new();
        // Set validity + value for each lane
        for (base, val) in [(0x50u8, 1u8), (0x52, 2), (0x54, 3), (0x57, 4)] {
            sim.set(base, val);
            sim.set(base + 1, 0x80);
        }
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0.map(|c| c.value()), Some(1));
        assert_eq!(ced.lane1.map(|c| c.value()), Some(2));
        assert_eq!(ced.lane2.map(|c| c.value()), Some(3));
        assert_eq!(ced.lane3.map(|c| c.value()), Some(4));
    }

    // --- error propagation ---

    #[test]
    fn write_frl_config_transport_error() {
        assert!(
            ScdcClient::write_frl_config(
                &mut Scdc::new(TestTransport::failing_after(0)),
                FrlConfig {
                    rate: HdmiForumFrl::NotSupported,
                    ffe_levels: FfeLevels::Ffe0,
                    dsc_frl_max: false,
                }
            )
            .is_err()
        );
    }

    #[test]
    fn read_training_status_transport_error() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_training_status()
                .is_err()
        );
    }

    #[test]
    fn read_ced_transport_error() {
        for n in 0..8 {
            assert!(
                Scdc::new(TestTransport::failing_after(n))
                    .read_ced()
                    .is_err(),
                "should fail when transport fails after {n} ops"
            );
        }
    }
}
