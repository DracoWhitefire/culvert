//! The [`Scdc`] client.

use hdmi_hal::scdc::ScdcTransport;

use crate::error::{ProtocolError, ScdcError};
use crate::register::address;

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
}
