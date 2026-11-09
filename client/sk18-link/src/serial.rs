//! The USB gadget's serial port (`/dev/ttyGS0` on the device).

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};
use rustix::event::{poll, PollFd, PollFlags};
use rustix::termios::{tcgetattr, tcsetattr, ControlModes, OptionalActions, SpecialCodeIndex};
use rustix::time::Timespec;

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

/// Waits up to `ms` milliseconds for something to read (or for the host to hang up, which also counts as readable).
pub fn wait_readable(port: &File, ms: i64) -> io::Result<bool> {
    let timeout = Timespec { tv_sec: ms / 1000, tv_nsec: (ms % 1000) * 1_000_000 };
    let mut fds = [PollFd::new(port, PollFlags::IN)];
    Ok(poll(&mut fds, Some(&timeout))? > 0)
}

/// Bytes the terminal already holds for reading.
pub fn bytes_waiting(port: &File) -> usize {
    rustix::io::ioctl_fionread(port).map_or(0, |n| usize::try_from(n).unwrap_or(0))
}

/// Called when `poll` timed out: is there data the terminal holds without having woken us (a lost wakeup in the
/// kernel's line discipline, decision D-037)? Nothing is ever flushed: an earlier version flushed the input on every
/// idle poll, which cut holes into frames that were still arriving and made the host's frames fail their CRC.
pub fn data_waiting(port: &File) -> bool {
    bytes_waiting(port) > 0
}
