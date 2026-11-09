//! The open-source firmware's link to controldeck: the device side of the SK-18 deck protocol over a USB serial port.
//!
//! [`protocol`] is the framing, [`device`] the protocol logic (pure, over a [`device::Hardware`]), [`display`] turns
//! the host's JPEG pictures into framebuffer pixels, and, on Unix, [`hw`] is the real hardware and [`serial`] the
//! serial port. See `docs/stock-serial-protocol.md` and decision D-034.

pub mod device;
pub mod display;
pub mod ha;
pub mod protocol;
pub mod settings;
#[cfg(unix)]
pub mod hw;
#[cfg(unix)]
pub mod serial;
