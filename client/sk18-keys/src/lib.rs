//! Reads the key events of the Waveshare SK-18.
//!
//! The SK-18 has no touchscreen device: a separate controller reports its 18 keys over UART1 as
//! press/release frames. This crate decodes that stream ([`frame`]), maps the firmware's key numbering
//! to what the user sees for a given way of holding the device ([`orient`]), and, on Unix, opens the
//! serial port ([`port`]). See `docs/uart1-key-protocol.md` for the protocol and how it was found.

pub mod frame;
pub mod orient;
#[cfg(unix)]
pub mod port;

pub use frame::{Decoder, Frame, KeyEvent, Stats};
pub use orient::Orientation;
