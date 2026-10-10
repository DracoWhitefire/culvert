//! Typed SCDC register map: bitfield structs and typed values.

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

/// FFE (Feed-Forward Equalization) levels written into `Config_1` bits\[7:4\]: the highest
/// TxFFE level index the source supports for the requested rate.
///
/// The allowed maximum depends on the FRL rate: 3 up to 12 Gbps, 7 for faster rates (see
/// [`FfeLevels::max_for`]). [`Scdc::write_frl_config`](crate::Scdc::write_frl_config)
/// rejects a configuration that exceeds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FfeLevels(u8);

impl FfeLevels {
    /// Constructs `FfeLevels`; `None` above 7, the highest level any FRL rate allows.
    pub const fn new(levels: u8) -> Option<Self> {
        if levels <= 7 {
            Some(Self(levels))
        } else {
            None
        }
    }

    /// The highest FFE level allowed at `rate`: 3 for rates up to 12 Gbps, 7 above.
    ///
    /// `HdmiForumFrl` values are in rate order (12 Gbps is 6; the 16, 20 and 24 Gbps rates
    /// that allow 7 follow it), so rates added to `HdmiForumFrl` later get 7 without a
    /// change here.
    pub fn max_for(rate: HdmiForumFrl) -> Self {
        // 3 for rates up to 12 Gbps, 3 + 4 = 7 for faster ones.
        let faster_than_12g = (rate as u8 > HdmiForumFrl::Rate12Gbps4Lanes as u8) as u8;
        Self(3 + 4 * faster_than_12g)
    }

    /// Returns the level index.
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Configuration written to `Config_1` (0x31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrlConfig {
    /// FRL rate to request. Use [`HdmiForumFrl::NotSupported`] to clear FRL mode.
    pub frl_rate: HdmiForumFrl,
    /// FFE levels to advertise to the sink.
    pub ffe_levels: FfeLevels,
}

/// Content written to `Config_0` (0x30).
///
/// [`Scdc::write_config_0`](crate::Scdc::write_config_0) writes the whole register, so
/// both fields are always set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Config0 {
    /// Bit 0: the sink may raise read requests.
    pub rr_enable: bool,
    /// Bit 1: the sink should not request link retraining.
    pub flt_no_retrain: bool,
}

/// Decoded content of `Source_Test_Configuration` (0x35).
///
/// Written by the sink, typically during compliance testing, to instruct the source.
/// Read it when [`UpdateFlags::source_test_update`] is set. Bits 0 and 4 are reserved.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTestConfig {
    /// Bit 1: use pre-shoot only for TxFFE.
    pub txffe_pre_shoot_only: bool,
    /// Bit 2: use de-emphasis only for TxFFE.
    pub txffe_de_emphasis_only: bool,
    /// Bit 3: use no TxFFE.
    pub txffe_no_ffe: bool,
    /// Bit 5: the source should not time out link training.
    pub flt_no_timeout: bool,
    /// Bit 6: the source should use the maximum FRL rate with DSC.
    pub dsc_frl_max: bool,
    /// Bit 7: the source should use the maximum FRL rate.
    pub frl_max: bool,
}

impl SourceTestConfig {
    /// Constructs `SourceTestConfig` from its flags, in bit order.
    pub fn new(
        txffe_pre_shoot_only: bool,
        txffe_de_emphasis_only: bool,
        txffe_no_ffe: bool,
        flt_no_timeout: bool,
        dsc_frl_max: bool,
        frl_max: bool,
    ) -> Self {
        Self {
            txffe_pre_shoot_only,
            txffe_de_emphasis_only,
            txffe_no_ffe,
            flt_no_timeout,
            dsc_frl_max,
            frl_max,
        }
    }
}

/// Link Training Pattern requested by the sink for one lane: a 4-bit field in
/// `Status_Flags_1` (0x41, lanes 0–1) or `Status_Flags_2` (0x42, lanes 2–3).
///
/// Every 4-bit value decodes: the values the HDMI 2.1 specification leaves undefined
/// (0x9–0xD) are [`Reserved`](Self::Reserved), with the raw value. Whether one matters
/// depends on the lanes in use, which the link training layer knows and culvert does not.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LtpReq {
    /// 0x0: no pattern requested; the lane is trained.
    None,
    /// 0x1: all-ones pattern.
    AllOnes,
    /// 0x2: all-zeros pattern.
    AllZeros,
    /// 0x3: Nyquist clock pattern.
    NyquistClock,
    /// 0x4: DDE compliance pattern (called RxDDE on the receiver side and TxDDE on the
    /// transmitter side of the Xilinx drivers).
    RxDdeCompliance,
    /// 0x5: LFSR 0.
    Lfsr0,
    /// 0x6: LFSR 1.
    Lfsr1,
    /// 0x7: LFSR 2.
    Lfsr2,
    /// 0x8: LFSR 3.
    Lfsr3,
    /// 0xE: the sink requests a change of FFE level.
    FfeChange,
    /// 0xF: the sink requests a lower FRL rate.
    RateChange,
    /// 0x9–0xD: a value the specification leaves undefined.
    Reserved(u8),
}

impl LtpReq {
    /// Decodes a 4-bit LTP request field (the low nibble of `nibble`).
    pub(crate) const fn from_nibble(nibble: u8) -> Self {
        match nibble & 0x0F {
            0x0 => Self::None,
            0x1 => Self::AllOnes,
            0x2 => Self::AllZeros,
            0x3 => Self::NyquistClock,
            0x4 => Self::RxDdeCompliance,
            0x5 => Self::Lfsr0,
            0x6 => Self::Lfsr1,
            0x7 => Self::Lfsr2,
            0x8 => Self::Lfsr3,
            0xE => Self::FfeChange,
            0xF => Self::RateChange,
            undefined => Self::Reserved(undefined),
        }
    }

    /// The request's 4-bit value.
    pub const fn value(self) -> u8 {
        match self {
            Self::None => 0x0,
            Self::AllOnes => 0x1,
            Self::AllZeros => 0x2,
            Self::NyquistClock => 0x3,
            Self::RxDdeCompliance => 0x4,
            Self::Lfsr0 => 0x5,
            Self::Lfsr1 => 0x6,
            Self::Lfsr2 => 0x7,
            Self::Lfsr3 => 0x8,
            Self::FfeChange => 0xE,
            Self::RateChange => 0xF,
            Self::Reserved(value) => value,
        }
    }
}

/// Per-lane link training pattern requests from `Status_Flags_1` (0x41) and
/// `Status_Flags_2` (0x42).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LtpRequests {
    /// Lane 0 (`Status_Flags_1` bits 3:0).
    pub lane0: LtpReq,
    /// Lane 1 (`Status_Flags_1` bits 7:4).
    pub lane1: LtpReq,
    /// Lane 2 (`Status_Flags_2` bits 3:0).
    pub lane2: LtpReq,
    /// Lane 3 (`Status_Flags_2` bits 7:4); `LtpReq::None` in 3-lane FRL.
    pub lane3: LtpReq,
}

impl LtpRequests {
    /// Constructs `LtpRequests` from the per-lane requests.
    pub fn new(lane0: LtpReq, lane1: LtpReq, lane2: LtpReq, lane3: LtpReq) -> Self {
        Self {
            lane0,
            lane1,
            lane2,
            lane3,
        }
    }

    /// Returns `true` when no lane requests a pattern, i.e. link training passed.
    pub fn all_trained(&self) -> bool {
        [self.lane0, self.lane1, self.lane2, self.lane3]
            .iter()
            .all(|r| *r == LtpReq::None)
    }
}

/// Decoded content of `Status_Flags_0` (0x40).
///
/// The per-lane link training pattern requests live in `Status_Flags_1`/`_2` and are
/// read separately with [`Scdc::read_ltp_requests`](crate::Scdc::read_ltp_requests);
/// `FRL_Start` is an `Update_0` flag ([`UpdateFlags::frl_start`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusFlags {
    /// Bit 0: the sink detects a clock.
    pub clock_detected: bool,
    /// Bit 1: channel 0 locked.
    pub ch0_locked: bool,
    /// Bit 2: channel 1 locked.
    pub ch1_locked: bool,
    /// Bit 3: channel 2 locked.
    pub ch2_locked: bool,
    /// Bit 4: lane 3 locked (FRL 4-lane only).
    pub ln3_locked: bool,
    /// Bit 6: the sink is ready for link training.
    pub flt_ready: bool,
    /// Bit 7: the sink failed to decode the DSC stream.
    pub dsc_decode_fail: bool,
}

impl StatusFlags {
    /// Constructs `StatusFlags` from the `Status_Flags_0` flags, in bit order.
    pub fn new(
        clock_detected: bool,
        ch0_locked: bool,
        ch1_locked: bool,
        ch2_locked: bool,
        ln3_locked: bool,
        flt_ready: bool,
        dsc_decode_fail: bool,
    ) -> Self {
        Self {
            clock_detected,
            ch0_locked,
            ch1_locked,
            ch2_locked,
            ln3_locked,
            flt_ready,
            dsc_decode_fail,
        }
    }
}

/// Decoded content of `Update_0` (0x10).
///
/// Flags are set by the sink to notify the source of state changes. The source
/// reads and then clears them via [`Scdc::clear_update_flags`](crate::Scdc::clear_update_flags)
/// (write-1-to-clear). `Update_1` (0x11) defines no fields and is not accessed.
///
/// Because this type is both returned by `read_update_flags` and accepted by
/// `clear_update_flags`, use [`UpdateFlags::new`] to construct it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateFlags {
    /// Bit 0: general status has changed.
    pub status_update: bool,
    /// Bit 1: CED counters have been updated; re-read the `ERR_DET` registers.
    pub ced_update: bool,
    /// Bit 2: read request test. Never cleared by
    /// [`Scdc::clear_update_flags`](crate::Scdc::clear_update_flags): the source must not
    /// clear it.
    pub rr_test: bool,
    /// Bit 3: the sink has written `Source_Test_Configuration` (0x35).
    pub source_test_update: bool,
    /// Bit 4: link training passed; the source may start FRL transmission.
    pub frl_start: bool,
    /// Bit 5: the per-lane link training pattern requests have changed.
    pub flt_update: bool,
    /// Bit 6: the Reed-Solomon correction count has been updated.
    pub rsed_update: bool,
}

impl UpdateFlags {
    /// Constructs `UpdateFlags` from the `Update_0` flags, in bit order.
    pub fn new(
        status_update: bool,
        ced_update: bool,
        rr_test: bool,
        source_test_update: bool,
        frl_start: bool,
        flt_update: bool,
        rsed_update: bool,
    ) -> Self {
        Self {
            status_update,
            ced_update,
            rr_test,
            source_test_update,
            frl_start,
            flt_update,
            rsed_update,
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

/// A 15-bit Reed-Solomon correction count decoded from `RS_Correction_L/H` (0x59/0x5A).
///
/// The high byte's bit 7 is the validity flag consumed by
/// [`Scdc::read_rs_correction`](crate::Scdc::read_rs_correction); the counter occupies
/// bits\[14:0\]. Values are always ≤ `0x7FFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RsCorrectionCount(u16);

impl RsCorrectionCount {
    /// Constructs an `RsCorrectionCount`, masking to 15 bits.
    pub fn new(raw: u16) -> Self {
        Self(raw & 0x7FFF)
    }

    /// Returns the Reed-Solomon correction count.
    pub fn value(self) -> u16 {
        self.0
    }
}

/// Per-lane character error counts decoded from `ERR_DET` registers (0x50–0x55 for
/// lanes 0–2, 0x57–0x58 for lane 3).
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

    #[test]
    fn status_flags_new_field_order() {
        // Each parameter maps to the named field for its Status_Flags_0 bit, in bit order.
        let all = |f: StatusFlags| {
            [
                f.clock_detected,
                f.ch0_locked,
                f.ch1_locked,
                f.ch2_locked,
                f.ln3_locked,
                f.flt_ready,
                f.dsc_decode_fail,
            ]
        };
        for i in 0..7 {
            let mut args = [false; 7];
            args[i] = true;
            let f = StatusFlags::new(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            );
            assert_eq!(all(f), args, "parameter {i}");
        }
    }

    #[test]
    fn ltp_req_from_nibble_decodes_every_value() {
        for n in 0u8..=0xF {
            let req = LtpReq::from_nibble(n);
            assert_eq!(req.value(), n, "nibble {n:#x}");
            assert_eq!(
                matches!(req, LtpReq::Reserved(_)),
                (0x9..=0xD).contains(&n),
                "nibble {n:#x}"
            );
        }
        // Only the low nibble is a request.
        assert_eq!(LtpReq::from_nibble(0xF3), LtpReq::NyquistClock);
    }

    #[test]
    fn ltp_requests_all_trained() {
        assert!(
            LtpRequests::new(LtpReq::None, LtpReq::None, LtpReq::None, LtpReq::None).all_trained()
        );
        assert!(
            !LtpRequests::new(LtpReq::None, LtpReq::None, LtpReq::None, LtpReq::Lfsr0)
                .all_trained()
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
        // Each parameter maps to the named field for its Update_0 bit, in bit order.
        let all = |f: UpdateFlags| {
            [
                f.status_update,
                f.ced_update,
                f.rr_test,
                f.source_test_update,
                f.frl_start,
                f.flt_update,
                f.rsed_update,
            ]
        };
        for bit in 0..7 {
            let mut args = [false; 7];
            args[bit] = true;
            let f = UpdateFlags::new(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            );
            assert_eq!(all(f), args, "parameter {bit}");
        }
    }
}
