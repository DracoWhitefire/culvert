//! Register addresses and per-register encoding and decoding for the SCDC register map
//! (HDMI 2.1 §10.4), without I/O.
//!
//! [`Scdc`](crate::Scdc) uses these to turn register bytes into culvert's types and back;
//! `culvert-async`'s client uses the same functions, so the register map exists once. A
//! client only performs the reads and writes: which registers each operation touches is
//! given by the address constants here.
//!
//! The constants cover every register culvert accesses, plus the CED checksum, which is
//! documented but not decoded yet. `Update_1` (0x11), `Test_Config_0` (0xC0) and the
//! manufacturer identification registers (0xD0 onwards) are described in
//! `doc/roadmap.md`.

use crate::error::ProtocolError;
use crate::register::{
    CedCount, CedCounters, Config0, FfeLevels, FrlConfig, LtpReq, LtpRequests, RsCorrectionCount,
    ScramblerStatus, SourceTestConfig, StatusFlags, TmdsConfig, UpdateFlags,
};

// --- Addresses

/// `Sink_Version` (§10.4.1).
pub const SINK_VERSION: u8 = 0x01;
/// `Source_Version` (§10.4.1).
pub const SOURCE_VERSION: u8 = 0x02;
/// `Update_0` (§10.4.2). `Update_1` (0x11) defines no fields in the HDMI 2.1
/// implementations this map is based on, so it is not accessed.
pub const UPDATE_0: u8 = 0x10;
/// `TMDS_Config` (§10.4.3).
pub const TMDS_CONFIG: u8 = 0x20;
/// `Scrambler_Status` (§10.4.3).
pub const SCRAMBLER_STATUS: u8 = 0x21;
/// `Config_0` (§10.4.4).
pub const CONFIG_0: u8 = 0x30;
/// `Config_1` (§10.4.4).
pub const CONFIG_1: u8 = 0x31;
/// `Source_Test_Configuration` (§10.4.4).
pub const SOURCE_TEST_CONFIG: u8 = 0x35;
/// `Status_Flags_0` (§10.4.4).
pub const STATUS_FLAGS_0: u8 = 0x40;
/// `Status_Flags_1` (§10.4.4): link training requests for lanes 0–1.
pub const STATUS_FLAGS_1: u8 = 0x41;
/// `Status_Flags_2` (§10.4.4): link training requests for lanes 2–3.
pub const STATUS_FLAGS_2: u8 = 0x42;
/// `ERR_DET_0_L` (§10.4.5).
pub const ERR_DET_0_L: u8 = 0x50;
/// `ERR_DET_0_H` (§10.4.5).
pub const ERR_DET_0_H: u8 = 0x51;
/// `ERR_DET_1_L` (§10.4.5).
pub const ERR_DET_1_L: u8 = 0x52;
/// `ERR_DET_1_H` (§10.4.5).
pub const ERR_DET_1_H: u8 = 0x53;
/// `ERR_DET_2_L` (§10.4.5).
pub const ERR_DET_2_L: u8 = 0x54;
/// `ERR_DET_2_H` (§10.4.5).
pub const ERR_DET_2_H: u8 = 0x55;
/// The checksum over the CED registers (§10.4.5); documented, not decoded yet.
pub const ERR_DET_CHECKSUM: u8 = 0x56;
/// `ERR_DET_3_L` (§10.4.5).
pub const ERR_DET_3_L: u8 = 0x57;
/// `ERR_DET_3_H` (§10.4.5).
pub const ERR_DET_3_H: u8 = 0x58;
/// `RS_Correction_L`: the Reed-Solomon correction count, low byte (FRL).
pub const RS_CORRECTION_L: u8 = 0x59;
/// `RS_Correction_H`: the Reed-Solomon correction count, high byte (FRL).
pub const RS_CORRECTION_H: u8 = 0x5A;

/// The registers [`decode_ced`] takes, in order: each lane's low and high byte, skipping
/// the checksum at 0x56.
pub const CED_REGISTERS: [u8; 8] = [
    ERR_DET_0_L,
    ERR_DET_0_H,
    ERR_DET_1_L,
    ERR_DET_1_H,
    ERR_DET_2_L,
    ERR_DET_2_H,
    ERR_DET_3_L,
    ERR_DET_3_H,
];

// --- Encoding and decoding

/// Encodes `TMDS_Config`: `Scrambling_Enable` (bit 0) and `TMDS_Bit_Clock_Ratio` (bit 1).
pub fn encode_tmds_config(config: TmdsConfig) -> u8 {
    (config.scrambling_enable as u8) | ((config.high_tmds_clock_ratio as u8) << 1)
}

/// Decodes `Scrambler_Status`: the sink confirms TMDS scrambling is active (bit 0).
pub fn decode_scrambler_status(byte: u8) -> ScramblerStatus {
    ScramblerStatus {
        scrambling_active: byte & 0x01 != 0,
    }
}

/// Decodes `Update_0`.
pub fn decode_update_flags(update_0: u8) -> UpdateFlags {
    UpdateFlags {
        status_update: update_0 & 0x01 != 0,
        ced_update: update_0 & 0x02 != 0,
        rr_test: update_0 & 0x04 != 0,
        source_test_update: update_0 & 0x08 != 0,
        frl_start: update_0 & 0x10 != 0,
        flt_update: update_0 & 0x20 != 0,
        rsed_update: update_0 & 0x40 != 0,
    }
}

/// Encodes the `Update_0` write that clears the flags set in `flags` (write 1 to clear).
///
/// `rr_test` is never cleared: Read Request Test is the one update flag the source must
/// not clear, so it is left out.
pub fn encode_clear_update_flags(flags: UpdateFlags) -> u8 {
    (flags.status_update as u8)
        | ((flags.ced_update as u8) << 1)
        | ((flags.source_test_update as u8) << 3)
        | ((flags.frl_start as u8) << 4)
        | ((flags.flt_update as u8) << 5)
        | ((flags.rsed_update as u8) << 6)
}

/// Encodes `Config_0`: `RR_Enable` (bit 0) and `FLT_No_Retrain` (bit 1).
pub fn encode_config_0(config: Config0) -> u8 {
    (config.rr_enable as u8) | ((config.flt_no_retrain as u8) << 1)
}

/// Encodes `Config_1`: `FRL_Rate` in bits\[3:0\] and `FFE_Levels` in bits\[7:4\].
///
/// Returns [`ProtocolError::FfeLevelsOutOfRange`] if the FFE levels exceed
/// [`FfeLevels::max_for`] the rate; a sink treats a prohibited value as 0.
pub fn encode_config_1(config: FrlConfig) -> Result<u8, ProtocolError> {
    if config.ffe_levels.value() > FfeLevels::max_for(config.frl_rate).value() {
        return Err(ProtocolError::FfeLevelsOutOfRange {
            rate: config.frl_rate,
            levels: config.ffe_levels.value(),
        });
    }
    Ok((config.frl_rate as u8) | (config.ffe_levels.value() << 4))
}

/// Decodes `Source_Test_Configuration`. Bits 0 and 4 are reserved.
pub fn decode_source_test_config(byte: u8) -> SourceTestConfig {
    SourceTestConfig {
        txffe_pre_shoot_only: byte & 0x02 != 0,
        txffe_de_emphasis_only: byte & 0x04 != 0,
        txffe_no_ffe: byte & 0x08 != 0,
        flt_no_timeout: byte & 0x20 != 0,
        dsc_frl_max: byte & 0x40 != 0,
        frl_max: byte & 0x80 != 0,
    }
}

/// Decodes `Status_Flags_0`: clock detection, lane lock, `FLT_Ready` and DSC decode
/// failure.
pub fn decode_status_flags(status_flags_0: u8) -> StatusFlags {
    StatusFlags {
        clock_detected: status_flags_0 & 0x01 != 0,
        ch0_locked: status_flags_0 & 0x02 != 0,
        ch1_locked: status_flags_0 & 0x04 != 0,
        ch2_locked: status_flags_0 & 0x08 != 0,
        ln3_locked: status_flags_0 & 0x10 != 0,
        flt_ready: status_flags_0 & 0x40 != 0,
        dsc_decode_fail: status_flags_0 & 0x80 != 0,
    }
}

/// Decodes the per-lane link training requests from `Status_Flags_1` (lanes 0–1) and
/// `Status_Flags_2` (lanes 2–3), one nibble per lane, low nibble first.
///
/// Returns [`ProtocolError::UnknownLtpReq`] if any lane reports a value not defined by
/// the HDMI 2.1 specification.
pub fn decode_ltp_requests(
    status_flags_1: u8,
    status_flags_2: u8,
) -> Result<LtpRequests, ProtocolError> {
    let decode =
        |nibble: u8| LtpReq::from_nibble(nibble).ok_or(ProtocolError::UnknownLtpReq(nibble));
    Ok(LtpRequests {
        lane0: decode(status_flags_1 & 0x0F)?,
        lane1: decode(status_flags_1 >> 4)?,
        lane2: decode(status_flags_2 & 0x0F)?,
        lane3: decode(status_flags_2 >> 4)?,
    })
}

/// Decodes the per-lane character error counts from the bytes of [`CED_REGISTERS`], in
/// that order. Each high byte's bit 7 is a validity flag; a lane whose flag is clear is
/// `None`.
pub fn decode_ced(bytes: [u8; 8]) -> CedCounters {
    let [l0, h0, l1, h1, l2, h2, l3, h3] = bytes;
    let decode = |lo: u8, hi: u8| -> Option<CedCount> {
        (hi & 0x80 != 0).then(|| CedCount::new(((hi as u16) << 8) | lo as u16))
    };
    CedCounters {
        lane0: decode(l0, h0),
        lane1: decode(l1, h1),
        lane2: decode(l2, h2),
        lane3: decode(l3, h3),
    }
}

/// Decodes the Reed-Solomon correction count from `RS_Correction_L/H`; `None` if the high
/// byte's validity bit (bit 7) is not set.
pub fn decode_rs_correction(low: u8, high: u8) -> Option<RsCorrectionCount> {
    (high & 0x80 != 0).then(|| RsCorrectionCount::new(((high as u16) << 8) | low as u16))
}
