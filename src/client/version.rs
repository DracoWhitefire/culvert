use hdmi_hal::scdc::ScdcTransport;

use crate::error::ScdcError;
use crate::register::address;

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
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

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::TestTransport;

    #[test]
    fn read_sink_version_returns_register_value() {
        let mut sim = TestTransport::new();
        sim.set(0x01, 0x01);
        assert_eq!(Scdc::new(sim).read_sink_version().unwrap(), 0x01);
    }

    #[test]
    fn write_source_version_writes_register() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.write_source_version(0x01).unwrap();
        assert_eq!(scdc.into_transport().get(0x02), 0x01);
    }

    #[test]
    fn transport_error_propagates() {
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_sink_version()
                .is_err()
        );
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .write_source_version(1)
                .is_err()
        );
    }
}
