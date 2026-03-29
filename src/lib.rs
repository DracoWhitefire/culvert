//! Typed access to the HDMI 2.1 SCDC (Status and Control Data Channel) register map.
//!
//! Culvert sits on top of [`hdmi_hal::scdc::ScdcTransport`] and provides named structs,
//! bitfield types, and typed operations for scrambling control, FRL training primitives,
//! and CED (Character Error Detection) reporting.
//!
//! The central type is `Scdc`, a thin stateless client that wraps a transport
//! and exposes one typed method per register group. Sequencing of register operations —
//! rate selection, timeout handling, retry logic — belongs in the link training crate
//! above.
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod client;
mod error;
mod register;

pub use error::{ProtocolError, ScdcError};
