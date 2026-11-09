//! The USB gadget's serial port (`/dev/ttyGS0` on the device).

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{open, Mode, OFlags};
use rustix::event::{poll, PollFd, PollFlags};
use rustix::termios::{tcflush, tcgetattr, tcsetattr, ControlModes, OptionalActions, QueueSelector, SpecialCodeIndex};
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

/// Releases a receive path the kernel has throttled for good.
///
/// On the SK-18 the USB serial gadget (u_serial on the sunxi musb controller) sometimes stays throttled after a
/// burst although the terminal's buffer is empty: the controller then holds the host's last packet in its FIFO
/// and NAKs, so a frame, or the end of one, only arrives when something else wakes the path (decision D-037).
/// `tcflush(TCIFLUSH)` ends with an unconditional `tty_unthrottle()`, which makes the gadget queue receive buffers
/// again. Call this only while nothing is readable: the terminal's buffer is then empty and nothing is lost.
pub fn unthrottle(port: &File) {
    let _ = tcflush(port, QueueSelector::IFlush);
}
