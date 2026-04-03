//! `plumbob::ScdcClient` implementation for [`Scdc`].

use hdmi_hal::scdc::ScdcTransport;
use plumbob::ScdcClient;

use crate::error::ScdcError;
use crate::register::{CedCount, FfeLevels, FrlConfig, LtpReq};

use super::Scdc;

impl<T: ScdcTransport> ScdcClient for Scdc<T> {
    type Error = ScdcError<T::Error>;

    fn write_frl_config(&mut self, config: plumbob::FrlConfig) -> Result<(), Self::Error> {
        self.write_frl_config(FrlConfig {
            frl_rate: config.rate,
            dsc_frl_max: config.dsc_frl_max,
            ffe_levels: ffe_levels(config.ffe_levels),
        })
    }

    fn read_training_status(&mut self) -> Result<plumbob::TrainingStatus, Self::Error> {
        let f = self.read_status_flags()?;
        Ok(plumbob::TrainingStatus {
            flt_ready: f.flt_ready,
            frl_start: f.frl_start,
            ltp_req: ltp_req(f.ltp_req),
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
    match f {
        plumbob::FfeLevels::Ffe0 => FfeLevels::Ffe0,
        plumbob::FfeLevels::Ffe1 => FfeLevels::Ffe1,
        plumbob::FfeLevels::Ffe2 => FfeLevels::Ffe2,
        plumbob::FfeLevels::Ffe3 => FfeLevels::Ffe3,
        plumbob::FfeLevels::Ffe4 => FfeLevels::Ffe4,
        plumbob::FfeLevels::Ffe5 => FfeLevels::Ffe5,
        plumbob::FfeLevels::Ffe6 => FfeLevels::Ffe6,
        plumbob::FfeLevels::Ffe7 => FfeLevels::Ffe7,
    }
}

fn ltp_req(req: LtpReq) -> plumbob::LtpReq {
    match req {
        LtpReq::None => plumbob::LtpReq::None,
        LtpReq::Lfsr0 => plumbob::LtpReq::Lfsr0,
        LtpReq::Lfsr1 => plumbob::LtpReq::Lfsr1,
        LtpReq::Lfsr2 => plumbob::LtpReq::Lfsr2,
        LtpReq::Lfsr3 => plumbob::LtpReq::Lfsr3,
    }
}

fn ced_count(c: CedCount) -> plumbob::CedCount {
    plumbob::CedCount::new(c.value())
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;
    use display_types::HdmiForumFrl;
    use plumbob::{FfeLevels, FrlConfig, LtpReq, ScdcClient};

    // --- write_frl_config ---

    #[test]
    fn write_frl_config_rate_and_ffe() {
        // Rate12Gbps4Lanes = discriminant 6; Ffe3 = 3 → bits[7:5] = 0b011 = 0x60
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
        assert_eq!(scdc.into_transport().get(0x30), 0x06 | 0x60);
    }

    #[test]
    fn write_frl_config_dsc_frl_max() {
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
        assert_eq!(scdc.into_transport().get(0x30), 0x10);
    }

    #[test]
    fn write_frl_config_all_ffe_levels() {
        use FfeLevels::*;
        for (ffe, expected_bits) in [
            (Ffe0, 0x00u8),
            (Ffe1, 0x20),
            (Ffe2, 0x40),
            (Ffe3, 0x60),
            (Ffe4, 0x80),
            (Ffe5, 0xA0),
            (Ffe6, 0xC0),
            (Ffe7, 0xE0),
        ] {
            let mut scdc = Scdc::new(TestTransport::new());
            ScdcClient::write_frl_config(
                &mut scdc,
                FrlConfig {
                    rate: HdmiForumFrl::NotSupported,
                    ffe_levels: ffe,
                    dsc_frl_max: false,
                },
            )
            .unwrap();
            assert_eq!(
                scdc.into_transport().get(0x30),
                expected_bits,
                "ffe={ffe:?}"
            );
        }
    }

    // --- read_training_status ---

    #[test]
    fn read_training_status_flt_ready() {
        let mut sim = TestTransport::new();
        sim.set(0x40, 0x40); // flt_ready bit
        let status = Scdc::new(sim).read_training_status().unwrap();
        assert!(status.flt_ready);
        assert!(!status.frl_start);
        assert_eq!(status.ltp_req, LtpReq::None);
    }

    #[test]
    fn read_training_status_frl_start() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 0x01); // frl_start bit
        let status = Scdc::new(sim).read_training_status().unwrap();
        assert!(!status.flt_ready);
        assert!(status.frl_start);
    }

    #[test]
    fn read_training_status_ltp_req_variants() {
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
                Scdc::new(sim).read_training_status().unwrap().ltp_req,
                expected,
                "nibble={nibble}"
            );
        }
    }

    #[test]
    fn read_training_status_unknown_ltp_req_is_error() {
        let mut sim = TestTransport::new();
        sim.set(0x41, 5 << 4); // nibble 5 — undefined by spec
        assert!(Scdc::new(sim).read_training_status().is_err());
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
        for (base, val) in [(0x50u8, 1u8), (0x52, 2), (0x54, 3), (0x56, 4)] {
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
