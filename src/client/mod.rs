//! The [`Scdc`] client.

use hdmi_hal::scdc::ScdcTransport;

use crate::error::{ProtocolError, ScdcError};
use crate::register::address;
use crate::register::{
    CedCount, CedCounters, FrlConfig, LtpReq, ScramblerStatus, StatusFlags, TmdsConfig, UpdateFlags,
};

/// Typed client for the HDMI 2.1 SCDC (Status and Control Data Channel) register map.
///
/// `Scdc<T>` wraps a [`ScdcTransport`] and exposes one typed method per register group.
/// It holds no protocol state; all sequencing logic belongs in the caller.
///
/// # Example
///
/// ```ignore
/// let mut scdc = Scdc::new(transport);
/// let sink_version = scdc.read_sink_version()?;
/// scdc.write_source_version(1)?;
/// ```
pub struct Scdc<T> {
    transport: T,
}

impl<T: ScdcTransport> Scdc<T> {
    /// Creates a new `Scdc` client wrapping the given transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Consumes the client and returns the underlying transport.
    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Reads the sink's SCDC protocol version from `Sink_Version` (0x01).
    pub fn read_sink_version(&mut self) -> Result<u8, ScdcError<T::Error>> {
        self.transport
            .read(address::SINK_VERSION)
            .map_err(ScdcError::Transport)
    }

    /// Writes the source's SCDC protocol version to `Source_Version` (0x02).
    pub fn write_source_version(&mut self, version: u8) -> Result<(), ScdcError<T::Error>> {
        self.transport
            .write(address::SOURCE_VERSION, version)
            .map_err(ScdcError::Transport)
    }

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
    use core::convert::Infallible;
    use display_types::HdmiForumFrl;

    use super::*;
    use crate::error::ProtocolError;
    use crate::register::{CedCount, FfeLevels, FrlConfig, LtpReq, ScramblerStatus, StatusFlags};

    struct Sim {
        regs: [u8; 256],
    }

    impl Sim {
        fn new() -> Self {
            Self { regs: [0u8; 256] }
        }

        fn set(&mut self, addr: u8, val: u8) {
            self.regs[addr as usize] = val;
        }

        fn get(&self, addr: u8) -> u8 {
            self.regs[addr as usize]
        }
    }

    impl ScdcTransport for Sim {
        type Error = Infallible;

        fn read(&mut self, reg: u8) -> Result<u8, Infallible> {
            Ok(self.regs[reg as usize])
        }

        fn write(&mut self, reg: u8, value: u8) -> Result<(), Infallible> {
            self.regs[reg as usize] = value;
            Ok(())
        }
    }

    // ── TMDS_Config ───────────────────────────────────────────────────────────

    #[test]
    fn tmds_config_scrambling_only() {
        let mut scdc = Scdc::new(Sim::new());
        scdc.write_tmds_config(TmdsConfig { scrambling_enable: true, high_tmds_clock_ratio: false })
            .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x01);
    }

    #[test]
    fn tmds_config_clock_ratio_only() {
        let mut scdc = Scdc::new(Sim::new());
        scdc.write_tmds_config(TmdsConfig { scrambling_enable: false, high_tmds_clock_ratio: true })
            .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x02);
    }

    #[test]
    fn tmds_config_both_clear() {
        let mut scdc = Scdc::new(Sim::new());
        scdc.write_tmds_config(TmdsConfig { scrambling_enable: false, high_tmds_clock_ratio: false })
            .unwrap();
        assert_eq!(scdc.into_transport().get(0x20), 0x00);
    }

    // ── Scrambler_Status ──────────────────────────────────────────────────────

    #[test]
    fn scrambler_status_bit0_set() {
        let mut sim = Sim::new();
        sim.set(0x21, 0xFF); // all bits set; only bit 0 is defined
        let mut scdc = Scdc::new(sim);
        assert_eq!(
            scdc.read_scrambler_status().unwrap(),
            ScramblerStatus { scrambling_active: true }
        );
    }

    #[test]
    fn scrambler_status_bit0_clear() {
        let mut sim = Sim::new();
        sim.set(0x21, 0xFE); // bit 0 clear, other bits set (reserved)
        let mut scdc = Scdc::new(sim);
        assert_eq!(
            scdc.read_scrambler_status().unwrap(),
            ScramblerStatus { scrambling_active: false }
        );
    }

    // ── Config_0 ─────────────────────────────────────────────────────────────

    #[test]
    fn frl_config_rate_field() {
        let mut scdc = Scdc::new(Sim::new());
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
        let mut scdc = Scdc::new(Sim::new());
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
        let mut scdc = Scdc::new(Sim::new());
        scdc.write_frl_config(FrlConfig {
            frl_rate: HdmiForumFrl::NotSupported,
            dsc_frl_max: false,
            ffe_levels: FfeLevels::Ffe7, // discriminant 7 → bits[7:5] = 0b111 = 0xE0
        })
        .unwrap();
        assert_eq!(scdc.into_transport().get(0x30), 0xE0);
    }

    // ── Status_Flags ─────────────────────────────────────────────────────────

    #[test]
    fn status_flags_all_zero() {
        let mut scdc = Scdc::new(Sim::new());
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
            let mut sim = Sim::new();
            sim.set(0x41, nibble << 4);
            let mut scdc = Scdc::new(sim);
            assert_eq!(scdc.read_status_flags().unwrap().ltp_req, expected);
        }
    }

    #[test]
    fn status_flags_unknown_ltp_req() {
        for nibble in 5u8..=15 {
            let mut sim = Sim::new();
            sim.set(0x41, nibble << 4);
            let mut scdc = Scdc::new(sim);
            assert!(matches!(
                scdc.read_status_flags(),
                Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(n))) if n == nibble
            ));
        }
    }

    // ── Update flags ─────────────────────────────────────────────────────────

    #[test]
    fn update_flags_individual_bits() {
        // status_update = bit 0 of Update_0
        let mut sim = Sim::new();
        sim.set(0x10, 0x01);
        assert!(Scdc::new(sim).read_update_flags().unwrap().status_update);

        // ced_update = bit 1
        let mut sim = Sim::new();
        sim.set(0x10, 0x02);
        assert!(Scdc::new(sim).read_update_flags().unwrap().ced_update);

        // frl_update = bit 2
        let mut sim = Sim::new();
        sim.set(0x10, 0x04);
        assert!(Scdc::new(sim).read_update_flags().unwrap().frl_update);

        // dsc_update = bit 0 of Update_1
        let mut sim = Sim::new();
        sim.set(0x11, 0x01);
        assert!(Scdc::new(sim).read_update_flags().unwrap().dsc_update);
    }

    #[test]
    fn clear_update_flags_w1c() {
        let mut scdc = Scdc::new(Sim::new());
        scdc.clear_update_flags(UpdateFlags::new(true, true, true, true)).unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x10), 0x07);
        assert_eq!(t.get(0x11), 0x01);
    }

    #[test]
    fn clear_update_flags_partial() {
        let mut scdc = Scdc::new(Sim::new());
        scdc.clear_update_flags(UpdateFlags::new(false, true, false, false)).unwrap();
        let t = scdc.into_transport();
        assert_eq!(t.get(0x10), 0x02); // only ced_update bit
        assert_eq!(t.get(0x11), 0x00);
    }

    // ── CED ──────────────────────────────────────────────────────────────────

    #[test]
    fn ced_validity_bit_required() {
        // High byte with bit 7 clear → None regardless of counter value.
        let mut sim = Sim::new();
        sim.set(0x50, 0xFF);
        sim.set(0x51, 0x7F); // valid bit clear
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, None);
    }

    #[test]
    fn ced_counter_validity_bit_stripped() {
        // High byte: bit 7 set (valid), counter bits = 0x01; low byte = 0x23.
        let mut sim = Sim::new();
        sim.set(0x50, 0x23);
        sim.set(0x51, 0x81);
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, Some(CedCount::new(0x0123)));
    }

    #[test]
    fn ced_lane3_independent() {
        let mut sim = Sim::new();
        sim.set(0x56, 0x01);
        sim.set(0x57, 0x80); // lane3 valid, count = 1
        let ced = Scdc::new(sim).read_ced().unwrap();
        assert_eq!(ced.lane0, None);
        assert_eq!(ced.lane3.map(|c| c.value()), Some(0x0001));
    }
}
