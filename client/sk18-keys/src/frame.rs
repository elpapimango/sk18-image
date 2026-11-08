//! Frame decoder for the key controller of the SK-18 (see `docs/uart1-key-protocol.md`).
//!
//! A frame is 13 bytes:
//!
//! ```text
//! AA 55 | CS | 06 F9 | 16 S R C 00 K | F5 5F
//! ```
//!
//! `CS` is the 8-bit sum of the six payload bytes (`16 S R C 00 K`), `06 F9` are the payload length and its
//! bitwise complement, `S` is 1 for pressed and 0 for released, `R` is the row (0..=5), `C` the column
//! (0..=2) and `K` a key code (a USB HID keyboard usage). The decoder resynchronises on the header after
//! any damaged byte and counts what it dropped; it never panics on arbitrary input.

/// Rows of the key grid as the firmware numbers them (portrait orientation).
pub const ROWS: u8 = 6;
/// Columns of the key grid as the firmware numbers them (portrait orientation).
pub const COLS: u8 = 3;

/// Length of one frame in bytes.
pub const FRAME_LEN: usize = 13;

const HEADER: [u8; 2] = [0xaa, 0x55];
const TRAILER: [u8; 2] = [0xf5, 0x5f];
const PAYLOAD_LEN: u8 = 6;
const TYPE_KEY: u8 = 0x16;

/// A press or release of one key, in the firmware's own numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Row, 0 to 5.
    pub row: u8,
    /// Column, 0 to 2.
    pub col: u8,
    /// True when the key went down, false when it was released.
    pub pressed: bool,
    /// Key code reported by the controller (a USB HID keyboard usage).
    pub code: u8,
}

/// A valid frame (length, complement, trailer and checksum are right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// A key press or release.
    Key(KeyEvent),
    /// A valid frame of a type this crate does not know, or a key frame outside the 6x3 grid.
    /// The six payload bytes are kept so that new message types can be explored.
    Unknown([u8; 6]),
}

/// Counters of what the decoder has seen so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Valid frames returned (key and unknown).
    pub frames: u64,
    /// Valid frames that were not key events.
    pub unknown_frames: u64,
    /// Bytes thrown away while looking for the next frame (noise, or a damaged frame).
    pub skipped_bytes: u64,
}

/// Streaming decoder: feed it whatever the serial port returns, in any chunk size.
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
    stats: Stats,
}

impl Decoder {
    /// A decoder with nothing buffered.
    pub fn new() -> Self {
        Self::default()
    }

    /// What has been seen so far.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Adds bytes and calls `on_frame` for every complete valid frame, in order.
    pub fn push(&mut self, bytes: &[u8], mut on_frame: impl FnMut(Frame)) {
        self.buf.extend_from_slice(bytes);
        let mut pos = 0;
        loop {
            // Look for the header.
            match find_header(&self.buf[pos..]) {
                None => {
                    // No header in view. A trailing 0xAA could still be the start of one.
                    let keep = usize::from(self.buf.last() == Some(&HEADER[0]) && self.buf.len() > pos);
                    let drop_to = self.buf.len() - keep;
                    self.stats.skipped_bytes += (drop_to - pos) as u64;
                    pos = drop_to;
                    break;
                }
                Some(off) => {
                    self.stats.skipped_bytes += off as u64;
                    pos += off;
                }
            }
            if self.buf.len() - pos < FRAME_LEN {
                break; // wait for the rest of the frame
            }
            match parse(&self.buf[pos..pos + FRAME_LEN]) {
                Some(frame) => {
                    self.stats.frames += 1;
                    if matches!(frame, Frame::Unknown(_)) {
                        self.stats.unknown_frames += 1;
                    }
                    on_frame(frame);
                    pos += FRAME_LEN;
                }
                None => {
                    // Not a frame after all: drop the header's first byte and look again.
                    self.stats.skipped_bytes += 1;
                    pos += 1;
                }
            }
        }
        self.buf.drain(..pos);
    }
}

fn find_header(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == HEADER)
}

/// Parses exactly one candidate frame (`data.len() == FRAME_LEN`); `None` if any check fails.
fn parse(data: &[u8]) -> Option<Frame> {
    debug_assert_eq!(data.len(), FRAME_LEN);
    if data[..2] != HEADER || data[11..13] != TRAILER {
        return None;
    }
    let (checksum, len, not_len) = (data[2], data[3], data[4]);
    if len != PAYLOAD_LEN || not_len != !len {
        return None;
    }
    let payload: [u8; 6] = data[5..11].try_into().ok()?;
    let sum = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    if checksum != sum {
        return None;
    }
    let [kind, state, row, col, zero, code] = payload;
    if kind == TYPE_KEY && state <= 1 && row < ROWS && col < COLS && zero == 0 {
        Some(Frame::Key(KeyEvent { row, col, pressed: state == 1, code }))
    } else {
        Some(Frame::Unknown(payload))
    }
}

/// Builds the frame the controller would send for a key event. Used by the tests and by tools that
/// replay events; the controller's own frames are what the decoder is checked against.
pub fn encode(event: KeyEvent) -> [u8; FRAME_LEN] {
    let payload = [TYPE_KEY, u8::from(event.pressed), event.row, event.col, 0, event.code];
    let sum = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    let mut f = [0u8; FRAME_LEN];
    f[..2].copy_from_slice(&HEADER);
    f[2] = sum;
    f[3] = PAYLOAD_LEN;
    f[4] = !PAYLOAD_LEN;
    f[5..11].copy_from_slice(&payload);
    f[11..13].copy_from_slice(&TRAILER);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(chunks: &[&[u8]]) -> (Vec<Frame>, Stats) {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        for c in chunks {
            d.push(c, |f| out.push(f));
        }
        (out, d.stats())
    }

    fn key(row: u8, col: u8, pressed: bool, code: u8) -> KeyEvent {
        KeyEvent { row, col, pressed, code }
    }

    #[test]
    fn real_frames_from_the_device() {
        // The first two frames of the capture: key (5,0) pressed, then released.
        let a = [0xaa, 0x55, 0x2a, 0x06, 0xf9, 0x16, 0x01, 0x05, 0x00, 0x00, 0x0e, 0xf5, 0x5f];
        let b = [0xaa, 0x55, 0x29, 0x06, 0xf9, 0x16, 0x00, 0x05, 0x00, 0x00, 0x0e, 0xf5, 0x5f];
        let (frames, stats) = decode_all(&[&a, &b]);
        assert_eq!(frames, vec![Frame::Key(key(5, 0, true, 0x0e)), Frame::Key(key(5, 0, false, 0x0e))]);
        assert_eq!(stats, Stats { frames: 2, unknown_frames: 0, skipped_bytes: 0 });
    }

    #[test]
    fn encode_matches_the_real_frame() {
        let a = [0xaa, 0x55, 0x2a, 0x06, 0xf9, 0x16, 0x01, 0x05, 0x00, 0x00, 0x0e, 0xf5, 0x5f];
        assert_eq!(encode(key(5, 0, true, 0x0e)), a);
    }

    #[test]
    fn byte_by_byte_gives_the_same_result() {
        let mut stream = Vec::new();
        for r in 0..ROWS {
            for c in 0..COLS {
                stream.extend_from_slice(&encode(key(r, c, true, r * 3 + c)));
                stream.extend_from_slice(&encode(key(r, c, false, r * 3 + c)));
            }
        }
        let whole = decode_all(&[&stream]);
        let pieces: Vec<&[u8]> = stream.chunks(1).collect();
        let bytewise = decode_all(&pieces);
        assert_eq!(whole.0.len(), 36);
        assert_eq!(whole, bytewise);
        let odd: Vec<&[u8]> = stream.chunks(7).collect();
        assert_eq!(whole, decode_all(&odd));
    }

    #[test]
    fn resynchronises_after_noise_and_counts_it() {
        let f1 = encode(key(1, 2, true, 0x22));
        let f2 = encode(key(1, 2, false, 0x22));
        let mut s = vec![0x00, 0xff, 0xaa, 0x00]; // noise, including a lone 0xAA
        s.extend_from_slice(&f1);
        s.extend_from_slice(&[0x11, 0x22, 0x33]);
        s.extend_from_slice(&f2);
        let (frames, stats) = decode_all(&[&s]);
        assert_eq!(frames, vec![Frame::Key(key(1, 2, true, 0x22)), Frame::Key(key(1, 2, false, 0x22))]);
        assert_eq!(stats.frames, 2);
        assert_eq!(stats.skipped_bytes, 7);
    }

    #[test]
    fn a_damaged_frame_is_dropped_and_the_next_one_still_decodes() {
        let good = encode(key(0, 0, true, 0x27));
        let mut bad = encode(key(3, 1, true, 0x08));
        bad[7] ^= 0x01; // a flipped bit in the row: the checksum no longer matches
        let mut s = Vec::new();
        s.extend_from_slice(&bad);
        s.extend_from_slice(&good);
        let (frames, stats) = decode_all(&[&s]);
        assert_eq!(frames, vec![Frame::Key(key(0, 0, true, 0x27))]);
        assert_eq!(stats.skipped_bytes, FRAME_LEN as u64);
    }

    #[test]
    fn a_frame_inside_a_damaged_header_is_found() {
        // A stray header start right before a real frame must not hide the real frame.
        let good = encode(key(2, 1, false, 0x05));
        let mut s = vec![0xaa, 0x55, 0x01];
        s.extend_from_slice(&good);
        let (frames, _) = decode_all(&[&s]);
        assert_eq!(frames, vec![Frame::Key(key(2, 1, false, 0x05))]);
    }

    #[test]
    fn rejects_wrong_length_complement_and_trailer() {
        let base = encode(key(2, 2, true, 0x06));
        for (idx, val) in [(3, 7u8), (4, 0xf8), (11, 0x00), (12, 0x00), (0, 0xab), (1, 0x54)] {
            let mut f = base;
            f[idx] = val;
            let (frames, _) = decode_all(&[&f]);
            assert!(frames.is_empty(), "byte {idx} = {val:#x} must invalidate the frame");
        }
    }

    #[test]
    fn unknown_types_and_out_of_grid_keys_are_kept_not_lost() {
        // A valid frame of another type.
        let payload = [0x17u8, 1, 2, 3, 4, 5];
        let mut f = encode(key(0, 0, true, 0));
        f[5..11].copy_from_slice(&payload);
        f[2] = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        let (frames, stats) = decode_all(&[&f]);
        assert_eq!(frames, vec![Frame::Unknown(payload)]);
        assert_eq!(stats.unknown_frames, 1);

        // A key frame with a row outside the grid.
        let mut g = encode(key(0, 0, true, 0x27));
        g[7] = 9;
        g[2] = g[5..11].iter().fold(0u8, |a, &b| a.wrapping_add(b));
        let (frames, _) = decode_all(&[&g]);
        assert!(matches!(frames[0], Frame::Unknown(p) if p[2] == 9));
    }

    #[test]
    fn a_trailing_header_byte_is_kept_for_the_next_chunk() {
        let f = encode(key(4, 2, true, 0x0d));
        let (frames, stats) = decode_all(&[&f[..1], &f[1..]]);
        assert_eq!(frames, vec![Frame::Key(key(4, 2, true, 0x0d))]);
        assert_eq!(stats.skipped_bytes, 0);
    }

    #[test]
    fn arbitrary_garbage_never_panics_or_invents_frames() {
        // Deterministic pseudo-random bytes (xorshift), no frame can appear by chance with a valid checksum
        // in this fixed sequence; the point is that nothing panics and the accounting adds up.
        let mut x: u32 = 0x1234_5678;
        let mut data = Vec::new();
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            data.push((x & 0xff) as u8);
        }
        let mut d = Decoder::new();
        let mut n = 0u64;
        for chunk in data.chunks(97) {
            d.push(chunk, |_| n += 1);
        }
        let s = d.stats();
        assert_eq!(s.frames, n);
        assert!(s.skipped_bytes + s.frames * FRAME_LEN as u64 <= data.len() as u64);
    }
}
