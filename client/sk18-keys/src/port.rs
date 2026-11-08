//! The serial port of the key controller (Unix only).

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};
use rustix::termios::{
    tcflush, tcgetattr, tcsetattr, ControlModes, OptionalActions, QueueSelector, SpecialCodeIndex,
};

/// The controller talks at 115200 baud, 8 data bits (decision log D-023).
pub const BAUD: u32 = 115_200;

/// Opens `path` read-only and sets it up as a raw 115200 8N1 line without flow control, then drops
/// whatever is already queued so the first frame read starts at a clean point.
pub fn open_port(path: &Path) -> io::Result<File> {
    let fd = open(path, OFlags::RDONLY | OFlags::NOCTTY | OFlags::CLOEXEC, Mode::empty())?;
    let mut t = tcgetattr(&fd)?;
    t.make_raw();
    t.set_input_speed(BAUD)?;
    t.set_output_speed(BAUD)?;
    t.control_modes.insert(ControlModes::CLOCAL | ControlModes::CREAD);
    t.control_modes.remove(ControlModes::CRTSCTS);
    t.special_codes[SpecialCodeIndex::VMIN] = 1;
    t.special_codes[SpecialCodeIndex::VTIME] = 0;
    tcsetattr(&fd, OptionalActions::Now, &t)?;
    tcflush(&fd, QueueSelector::IFlush)?;
    Ok(File::from(fd))
}
