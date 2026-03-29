//! The [`Scdc`] client.

use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::address;
use crate::register::{ScramblerStatus, TmdsConfig};

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
}
