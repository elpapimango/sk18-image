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

/// Bytes the terminal already holds for reading.
pub fn bytes_waiting(port: &File) -> usize {
    rustix::io::ioctl_fionread(port).map_or(0, |n| usize::try_from(n).unwrap_or(0))
}

/// Works around two stalls of the receive path of the SK-18's USB serial gadget (decision D-037).
///
/// Call it when `poll` timed out. (1) The terminal can hold data without waking the reader (a lost wakeup in the
/// kernel's line discipline): then the data is simply read, the caller sees `true`. (2) Or the gadget has stayed
/// throttled although the buffer is empty, so the controller NAKs the host: `tcflush(TCIFLUSH)` ends with an
/// unconditional `tty_unthrottle()`, which makes the gadget queue receive buffers again. That flush is only done
/// when nothing is waiting, so no data is thrown away.
pub fn recover(port: &File) -> bool {
    if bytes_waiting(port) > 0 {
        return true;
    }
    let _ = tcflush(port, QueueSelector::IFlush);
    false
}
