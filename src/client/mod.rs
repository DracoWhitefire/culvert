//! The [`Scdc`] client.

use hdmi_hal::scdc::ScdcTransport;

mod ced;
mod frl;
mod scrambling;
mod update;
mod version;

#[cfg(test)]
mod test_transport;

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
}
