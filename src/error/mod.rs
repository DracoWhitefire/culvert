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
    ///
    /// The inner value is the raw 4-bit field read directly from the SCDC register. It is
    /// exposed so callers can log or diagnose misbehaving sinks, not because it carries
    /// semantic meaning — any value outside the spec-defined set is treated as a protocol
    /// violation regardless of its numeric value.
    ///
    /// Not currently returned: no culvert method reads an FRL rate back from the sink.
    UnknownFrlRate(u8),
    /// The requested FFE levels exceed the maximum allowed at the requested FRL rate
    /// (see [`FfeLevels::max_for`](crate::FfeLevels::max_for)). A sink treats a
    /// prohibited value as 0, so it is rejected before anything is written.
    FfeLevelsOutOfRange {
        /// The requested FRL rate.
        rate: crate::FrlRate,
        /// The requested FFE levels.
        levels: u8,
    },
    /// A clear of `Update_0` asked for `RR_Test` (Read Request Test, bit 2). Nothing is
    /// written.
    ///
    /// culvert treats `RR_Test` as the one update flag the source must not clear. Only one
    /// reference states that rule (the Intel `xe` HDMI 2.1 series: "Read Request Test is
    /// the only update flag the source cannot clear", rejecting it with `-EINVAL`), and
    /// another clears the bit regardless (Amlogic's HDMI 2.1 transmitter). culvert follows
    /// the explicit rule because leaving a sink-owned test flag alone cannot break a sink's
    /// read-request test, while clearing it might. The decision is provisional: see
    /// "RR_Test is not cleared" in `doc/architecture.md` for what would change it.
    RrTestNotClearable,
}
