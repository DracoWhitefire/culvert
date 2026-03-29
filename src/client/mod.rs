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
