//! The USB gadget's serial port (`/dev/ttyGS0` on the device).

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};
use rustix::termios::{tcgetattr, tcsetattr, ControlModes, OptionalActions, SpecialCodeIndex};

/// Opens `path` read/write as a raw port: no echo, no newline translation, no flow control, reads return as soon as
/// one byte is there. The baud rate is irrelevant on a USB gadget.
pub fn open_raw(path: &Path) -> io::Result<File> {
    let fd = open(path, OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC, Mode::empty())?;
    let mut t = tcgetattr(&fd)?;
    t.make_raw();
    t.control_modes.insert(ControlModes::CLOCAL | ControlModes::CREAD);
    t.control_modes.remove(ControlModes::CRTSCTS);
    t.special_codes[SpecialCodeIndex::VMIN] = 1;
    t.special_codes[SpecialCodeIndex::VTIME] = 0;
    tcsetattr(&fd, OptionalActions::Now, &t)?;
    Ok(File::from(fd))
}
