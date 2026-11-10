//! The deck protocol over the USB gadget's HID function (`/dev/hidg0`), decision D-041.
//!
//! The byte stream of the protocol (see `protocol`) is cut into fixed 512-byte HID reports, in both directions:
//! `[len u16 LE][up to 510 bytes of stream][zero padding]`. A report never carries a partial length, and `len == 0`
//! is an empty report that is ignored, so the reader can resynchronise on report boundaries and the stream above
//! it is the same as on a serial port. On the host the reports are written to and read from `hidraw`.

use std::fs::File;
use std::io::{self, Read, Write};

/// Size of every report (the endpoint's maximum packet size and the musb FIFO size).
pub const REPORT: usize = 512;
/// Stream bytes in one report.
pub const PAYLOAD: usize = REPORT - 2;

/// Cuts `bytes` into reports.
pub fn reports(bytes: &[u8]) -> impl Iterator<Item = [u8; REPORT]> + '_ {
    bytes.chunks(PAYLOAD).map(|chunk| {
        let mut report = [0u8; REPORT];
        report[..2].copy_from_slice(&(chunk.len() as u16).to_le_bytes());
        report[2..2 + chunk.len()].copy_from_slice(chunk);
        report
    })
}

/// The stream bytes of one received report, or `None` when the report is malformed (a length that does not fit).
pub fn payload(report: &[u8]) -> Option<&[u8]> {
    let len = usize::from(u16::from_le_bytes([*report.first()?, *report.get(1)?]));
    report.get(2..2 + len).filter(|_| len <= PAYLOAD)
}

/// A HID endpoint presented as a byte stream, like a serial port. Reads return the payload of whole reports (the
/// caller's buffer may be smaller than a report; the rest is kept for the next read). Writes are cut into reports.
pub struct HidPort {
    file: File,
    pending: Vec<u8>,
    taken: usize,
}

impl HidPort {
    pub fn new(file: File) -> Self {
        Self { file, pending: Vec::new(), taken: 0 }
    }

    /// A second handle for writing from another thread. Only the original reads.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self::new(self.file.try_clone()?))
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    fn fill(&mut self) -> io::Result<bool> {
        let mut report = [0u8; REPORT];
        let n = self.file.read(&mut report)?;
        if n == 0 {
            return Ok(false);
        }
        self.pending.clear();
        self.taken = 0;
        // A malformed report is dropped; the frame decoder above resynchronises on the next start marker.
        if let Some(data) = payload(&report[..n]) {
            self.pending.extend_from_slice(data);
        }
        Ok(true)
    }
}

impl Read for HidPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.taken >= self.pending.len() {
            if !self.fill()? {
                return Ok(0);
            }
        }
        let n = buf.len().min(self.pending.len() - self.taken);
        buf[..n].copy_from_slice(&self.pending[self.taken..self.taken + n]);
        self.taken += n;
        Ok(n)
    }
}

impl Write for HidPort {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        for report in reports(bytes) {
            // One write(2) is one report; the call blocks while the previous report waits for the host.
            let n = self.file.write(&report)?;
            if n != REPORT {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "short HID report"));
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_is_cut_into_whole_reports_and_back() {
        let data: Vec<u8> = (0..1500u32).map(|i| (i % 251) as u8).collect();
        let out: Vec<[u8; REPORT]> = reports(&data).collect();
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|r| r.len() == REPORT));
        let back: Vec<u8> = out.iter().flat_map(|r| payload(r).unwrap().to_vec()).collect();
        assert_eq!(back, data);
        assert_eq!(payload(&out[2]).unwrap().len(), 1500 - 2 * PAYLOAD);
    }

    #[test]
    fn nothing_makes_no_report_and_a_bad_length_is_refused() {
        assert_eq!(reports(&[]).count(), 0);
        let mut bad = [0u8; REPORT];
        bad[..2].copy_from_slice(&(PAYLOAD as u16 + 1).to_le_bytes());
        assert!(payload(&bad).is_none());
        assert!(payload(&[1]).is_none());
        assert_eq!(payload(&[0, 0, 9, 9]).unwrap(), &[] as &[u8]);
    }
}
