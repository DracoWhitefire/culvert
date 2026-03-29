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

/// Decoded content of `Update_0` (0x10) and `Update_1` (0x11).
///
/// Flags are set by the sink to notify the source of state changes. The source
/// reads and then clears them via [`Scdc::clear_update_flags`](crate::Scdc::clear_update_flags).
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

/// A 15-bit character error count decoded from an ERR_DET register pair.
///
/// The high byte's bit 7 is the validity flag consumed by [`CedCounters`];
/// the counter occupies bits\[14:0\]. Values are always ≤ `0x7FFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CedCount(u16);

impl CedCount {
    /// Constructs a `CedCount`, masking to 15 bits.
    pub(crate) fn new(raw: u16) -> Self {
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
