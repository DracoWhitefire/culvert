//! Error types returned by [`crate::Scdc`] operations.

/// Errors returned by [`Scdc`](crate::Scdc) register operations.
///
/// Two categories of failure are distinguished so that callers can handle them
/// separately: a transport failure means the I²C/DDC bus returned an error; a
/// protocol violation means the sink returned register content that does not
/// conform to the SCDC specification.
#[non_exhaustive]
#[derive(Debug)]
pub enum ScdcError<E> {
    /// The underlying transport returned an error.
    Transport(E),
    /// The sink returned register content that violates the SCDC protocol.
    Protocol(ProtocolError),
}

/// Protocol-level violations detected while decoding SCDC register content.
#[non_exhaustive]
#[derive(Debug)]
pub enum ProtocolError {
    /// The sink reported an FRL rate value not defined by the HDMI 2.1 specification.
    UnknownFrlRate(u8),
    /// The sink reported an LTP request value not defined by the HDMI 2.1 specification.
    UnknownLtpReq(u8),
}
