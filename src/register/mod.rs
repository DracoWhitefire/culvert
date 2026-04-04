//! Typed SCDC register map: bitfield structs and typed values.

pub(crate) mod address;

use display_types::HdmiForumFrl;

// Re-export for use in the public API.
pub use display_types::HdmiForumFrl as FrlRate;

/// Configuration written to `TMDS_Config` (0x20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TmdsConfig {
    /// Enable TMDS scrambling.
    pub scrambling_enable: bool,
    /// TMDS bit clock ratio: `false` = divide by 10, `true` = divide by 40.
    pub high_tmds_clock_ratio: bool,
}

/// Decoded content of `Scrambler_Status` (0x21).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScramblerStatus {
    /// The sink confirms that TMDS scrambling is active.
    pub scrambling_active: bool,
}

impl ScramblerStatus {
    /// Constructs a `ScramblerStatus`.
    pub fn new(scrambling_active: bool) -> Self {
        Self { scrambling_active }
    }
}

/// FFE (Feed-Forward Equalization) level count written into `Config_0` bits\[5:3\].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfeLevels {
    /// No FFE levels.
    Ffe0 = 0,
    /// 1 FFE level.
    Ffe1 = 1,
    /// 2 FFE levels.
    Ffe2 = 2,
    /// 3 FFE levels.
    Ffe3 = 3,
    /// 4 FFE levels.
    Ffe4 = 4,
    /// 5 FFE levels.
    Ffe5 = 5,
    /// 6 FFE levels.
    Ffe6 = 6,
    /// 7 FFE levels.
    Ffe7 = 7,
}

/// Configuration written to `Config_0` (0x30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrlConfig {
    /// FRL rate to request. Use [`HdmiForumFrl::NotSupported`] to clear FRL mode.
    pub frl_rate: HdmiForumFrl,
    /// Request DSC at the maximum supported FRL rate (`DSC_FRL_Max`).
    pub dsc_frl_max: bool,
    /// Number of FFE levels to advertise to the sink.
    pub ffe_levels: FfeLevels,
}

/// Link Training Pattern requested by the sink via `Status_Flags_1` bits\[7:4\].
///
/// An undefined nibble value from the sink surfaces as
/// [`ProtocolError::UnknownLtpReq`](crate::ProtocolError::UnknownLtpReq).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LtpReq {
    /// No LTP requested; training is complete or not yet started.
    None = 0,
    /// Request LFSR0 training pattern on all active lanes.
    Lfsr0 = 1,
    /// Request LFSR1 training pattern on all active lanes.
    Lfsr1 = 2,
    /// Request LFSR2 training pattern on all active lanes.
    Lfsr2 = 3,
    /// Request LFSR3 training pattern on all active lanes.
    Lfsr3 = 4,
}

/// Decoded content of `Status_Flags_0` (0x40) and `Status_Flags_1` (0x41).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusFlags {
    /// A TMDS or FRL clock signal is detected on the cable.
    pub clock_detected: bool,
    /// A cable is detected on the HDMI connector.
    pub cable_connected: bool,
    /// Lane 0 has achieved symbol lock.
    pub ch0_locked: bool,
    /// Lane 1 has achieved symbol lock.
    pub ch1_locked: bool,
    /// Lane 2 has achieved symbol lock.
    pub ch2_locked: bool,
    /// Lane 3 has achieved symbol lock (FRL 4-lane only).
    pub ch3_locked: bool,
    /// The sink is ready to begin FRL link training (`FLT_Ready`).
    pub flt_ready: bool,
    /// The sink signals that FRL training may begin (`FRL_Start`).
    pub frl_start: bool,
    /// The link training pattern currently requested by the sink.
    pub ltp_req: LtpReq,
}

impl StatusFlags {
    /// Constructs a `StatusFlags`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        clock_detected: bool,
        cable_connected: bool,
        ch0_locked: bool,
        ch1_locked: bool,
        ch2_locked: bool,
        ch3_locked: bool,
        flt_ready: bool,
        frl_start: bool,
        ltp_req: LtpReq,
    ) -> Self {
        Self {
            clock_detected,
            cable_connected,
            ch0_locked,
            ch1_locked,
            ch2_locked,
            ch3_locked,
            flt_ready,
            frl_start,
            ltp_req,
        }
    }
}

/// Decoded content of `Update_0` (0x10) and `Update_1` (0x11).
///
/// Flags are set by the sink to notify the source of state changes. The source
/// reads and then clears them via [`Scdc::clear_update_flags`](crate::Scdc::clear_update_flags).
///
/// Because this type is both returned by `read_update_flags` and accepted by
/// `clear_update_flags`, use [`UpdateFlags::new`] to construct it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateFlags {
    /// FRL status has changed; re-read `Status_Flags`.
    pub frl_update: bool,
    /// CED counters have been updated; re-read `ERR_DET` registers.
    pub ced_update: bool,
    /// General status has changed.
    pub status_update: bool,
    /// DSC status has changed (`Update_1` bit 0).
    pub dsc_update: bool,
}

impl UpdateFlags {
    /// Constructs an `UpdateFlags` value.
    pub fn new(status_update: bool, ced_update: bool, frl_update: bool, dsc_update: bool) -> Self {
        Self {
            status_update,
            ced_update,
            frl_update,
            dsc_update,
        }
    }
}

/// A 15-bit character error count decoded from an ERR_DET register pair.
///
/// The high byte's bit 7 is the validity flag consumed by [`CedCounters`];
/// the counter occupies bits\[14:0\]. Values are always ≤ `0x7FFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CedCount(u16);

impl CedCount {
    /// Constructs a `CedCount`, masking to 15 bits.
    pub fn new(raw: u16) -> Self {
        Self(raw & 0x7FFF)
    }

    /// Returns the character error count.
    pub fn value(self) -> u16 {
        self.0
    }
}

/// Per-lane character error counts decoded from `ERR_DET` registers (0x50–0x57).
///
/// A lane's counter is `None` when its validity bit is not set. `lane3` is only
/// populated in 4-lane FRL mode.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CedCounters {
    /// Character error count for lane 0, or `None` if the validity bit is not set.
    pub lane0: Option<CedCount>,
    /// Character error count for lane 1, or `None` if the validity bit is not set.
    pub lane1: Option<CedCount>,
    /// Character error count for lane 2, or `None` if the validity bit is not set.
    pub lane2: Option<CedCount>,
    /// Character error count for lane 3, or `None` if the validity bit is not set.
    /// Always `None` in TMDS mode or 3-lane FRL mode.
    pub lane3: Option<CedCount>,
}

impl CedCounters {
    /// Constructs a `CedCounters`.
    pub fn new(
        lane0: Option<CedCount>,
        lane1: Option<CedCount>,
        lane2: Option<CedCount>,
        lane3: Option<CedCount>,
    ) -> Self {
        Self {
            lane0,
            lane1,
            lane2,
            lane3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ced_count_masks_validity_bit() {
        // The high byte's bit 7 (validity) must not bleed into the value.
        assert_eq!(CedCount::new(0xFFFF).value(), 0x7FFF);
        assert_eq!(CedCount::new(0x8000).value(), 0x0000);
    }

    #[test]
    fn ced_count_preserves_15_bit_value() {
        assert_eq!(CedCount::new(0x0000).value(), 0x0000);
        assert_eq!(CedCount::new(0x0001).value(), 0x0001);
        assert_eq!(CedCount::new(0x7FFF).value(), 0x7FFF);
    }

    fn status_flags_all_false() -> StatusFlags {
        StatusFlags::new(
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            LtpReq::None,
        )
    }

    #[test]
    fn status_flags_new_field_order() {
        assert!(
            StatusFlags::new(
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                LtpReq::None
            )
            .clock_detected
        );
        assert!(
            StatusFlags::new(
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                LtpReq::None
            )
            .cable_connected
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                LtpReq::None
            )
            .ch0_locked
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                LtpReq::None
            )
            .ch1_locked
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                LtpReq::None
            )
            .ch2_locked
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                LtpReq::None
            )
            .ch3_locked
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                LtpReq::None
            )
            .flt_ready
        );
        assert!(
            StatusFlags::new(
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                LtpReq::None
            )
            .frl_start
        );
        assert_eq!(status_flags_all_false().ltp_req, LtpReq::None);
        assert_eq!(
            StatusFlags::new(
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                LtpReq::Lfsr2
            )
            .ltp_req,
            LtpReq::Lfsr2
        );
    }

    #[test]
    fn ced_counters_new_field_order() {
        let a = CedCount::new(1);
        let b = CedCount::new(2);
        let c = CedCount::new(3);
        let d = CedCount::new(4);
        let counters = CedCounters::new(Some(a), Some(b), Some(c), Some(d));
        assert_eq!(counters.lane0.unwrap().value(), 1);
        assert_eq!(counters.lane1.unwrap().value(), 2);
        assert_eq!(counters.lane2.unwrap().value(), 3);
        assert_eq!(counters.lane3.unwrap().value(), 4);
    }

    #[test]
    fn ced_counters_new_lane3_none() {
        let counters = CedCounters::new(
            Some(CedCount::new(0)),
            Some(CedCount::new(0)),
            Some(CedCount::new(0)),
            None,
        );
        assert!(counters.lane3.is_none());
    }

    #[test]
    fn scrambler_status_new() {
        assert!(ScramblerStatus::new(true).scrambling_active);
        assert!(!ScramblerStatus::new(false).scrambling_active);
    }

    #[test]
    fn update_flags_new_field_order() {
        // Verify each parameter maps to the correct named field.
        let f = UpdateFlags::new(true, false, false, false);
        assert!(f.status_update);
        assert!(!f.ced_update && !f.frl_update && !f.dsc_update);

        let f = UpdateFlags::new(false, true, false, false);
        assert!(f.ced_update);
        assert!(!f.status_update && !f.frl_update && !f.dsc_update);

        let f = UpdateFlags::new(false, false, true, false);
        assert!(f.frl_update);
        assert!(!f.status_update && !f.ced_update && !f.dsc_update);

        let f = UpdateFlags::new(false, false, false, true);
        assert!(f.dsc_update);
        assert!(!f.status_update && !f.ced_update && !f.frl_update);
    }
}
