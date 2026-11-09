//! Framing of the SK-18 deck protocol (documented in `docs/stock-serial-protocol.md`).
//!
//! ```text
//! A1 A5 5A 5E | id u32 | cmd u32 | size u32 | crc32(size bytes) u32 | data[size] | crc32(data) u32
//! ```
//!
//! All integers are little endian; the CRC is the standard IEEE CRC-32 (the same as zlib's). The decoder
//! resynchronises on the header and drops frames whose size CRC or data CRC is wrong, as the stock firmware does.

/// Start-of-frame marker.
pub const HEADER: [u8; 4] = [0xA1, 0xA5, 0x5A, 0x5E];
/// `cmd` of a frame whose data is one JPEG of the whole screen (also the device's "image shown" reply, with no data).
pub const CMD_SHOW_JPG: u32 = 100;
/// `cmd` of a frame whose data is one JSON object.
pub const CMD_JSON: u32 = 101;
/// Bytes before `data`: header, id, cmd, size and the size CRC.
const PREFIX_LEN: usize = 20;
/// Bytes a frame adds around its data.
pub const FRAME_OVERHEAD: usize = PREFIX_LEN + 4;
/// Largest `data` accepted; a bigger size is treated as line noise.
pub const MAX_DATA_LEN: usize = 2 * 1024 * 1024;

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            bit += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = crc_table();

/// Standard IEEE CRC-32.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = u32::MAX;
    for &b in data {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

/// One protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Sender-chosen id; a reply carries the id of the request it answers.
    pub id: u32,
    /// [`CMD_SHOW_JPG`] or [`CMD_JSON`].
    pub cmd: u32,
    /// The payload.
    pub data: Vec<u8>,
}

impl Frame {
    /// A JSON frame.
    pub fn json(id: u32, body: &serde_json::Value) -> Self {
        Self { id, cmd: CMD_JSON, data: body.to_string().into_bytes() }
    }

    /// The reply to an image frame: the same id, no data.
    pub fn image_shown(id: u32) -> Self {
        Self { id, cmd: CMD_SHOW_JPG, data: Vec::new() }
    }

    /// The bytes to put on the wire.
    pub fn encode(&self) -> Vec<u8> {
        let size = (self.data.len() as u32).to_le_bytes();
        let mut out = Vec::with_capacity(FRAME_OVERHEAD + self.data.len());
        out.extend_from_slice(&HEADER);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.cmd.to_le_bytes());
        out.extend_from_slice(&size);
        out.extend_from_slice(&crc32(&size).to_le_bytes());
        out.extend_from_slice(&self.data);
        out.extend_from_slice(&crc32(&self.data).to_le_bytes());
        out
    }
}

/// Incremental decoder: feed it bytes as they arrive and take complete frames out.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
    dropped: u64,
}

fn find_header(data: &[u8]) -> Option<usize> {
    data.windows(HEADER.len()).position(|w| w == HEADER)
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

impl FrameDecoder {
    /// An empty decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends received bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Candidate frames thrown away because of a bad CRC or an implausible size.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The next complete, valid frame, or `None` when more bytes are needed.
    /// Bytes held back waiting for the rest of a frame.
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    pub fn next_frame(&mut self) -> Option<Frame> {
        loop {
            let Some(start) = find_header(&self.buffer) else {
                // Keep what could be the start of a header at the end.
                let keep = HEADER.len() - 1;
                if self.buffer.len() > keep {
                    self.buffer.drain(..self.buffer.len() - keep);
                }
                return None;
            };
            if start > 0 {
                self.buffer.drain(..start);
            }
            if self.buffer.len() < PREFIX_LEN {
                return None;
            }
            let size_bytes = &self.buffer[12..16];
            let size = u32_at(&self.buffer, 12) as usize;
            if u32_at(&self.buffer, 16) != crc32(size_bytes) || size > MAX_DATA_LEN {
                // Not a frame: skip this header and look for the next one.
                self.dropped += 1;
                self.buffer.drain(..HEADER.len());
                continue;
            }
            let total = FRAME_OVERHEAD + size;
            if self.buffer.len() < total {
                return None;
            }
            let data = &self.buffer[PREFIX_LEN..PREFIX_LEN + size];
            if u32_at(&self.buffer, PREFIX_LEN + size) != crc32(data) {
                self.dropped += 1;
                self.buffer.drain(..HEADER.len());
                continue;
            }
            let frame = Frame { id: u32_at(&self.buffer, 4), cmd: u32_at(&self.buffer, 8), data: data.to_vec() };
            self.buffer.drain(..total);
            return Some(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_is_the_standard_one() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn a_frame_round_trips() {
        let f = Frame { id: 7, cmd: CMD_JSON, data: br#"{"method":"getInfo"}"#.to_vec() };
        let mut d = FrameDecoder::new();
        d.push(&f.encode());
        assert_eq!(d.next_frame(), Some(f));
        assert_eq!(d.next_frame(), None);
        assert_eq!(d.dropped(), 0);
    }

    #[test]
    fn the_layout_on_the_wire_is_the_documented_one() {
        let f = Frame { id: 1, cmd: CMD_SHOW_JPG, data: vec![0xAB, 0xCD] };
        let w = f.encode();
        assert_eq!(&w[..4], &[0xA1, 0xA5, 0x5A, 0x5E]);
        assert_eq!(&w[4..8], &1u32.to_le_bytes());
        assert_eq!(&w[8..12], &100u32.to_le_bytes());
        assert_eq!(&w[12..16], &2u32.to_le_bytes());
        assert_eq!(&w[16..20], &crc32(&2u32.to_le_bytes()).to_le_bytes());
        assert_eq!(&w[20..22], &[0xAB, 0xCD]);
        assert_eq!(&w[22..26], &crc32(&[0xAB, 0xCD]).to_le_bytes());
        assert_eq!(w.len(), FRAME_OVERHEAD + 2);
    }

    #[test]
    fn the_reply_to_an_image_has_the_same_id_and_no_data() {
        let r = Frame::image_shown(42);
        assert_eq!((r.id, r.cmd, r.data.len()), (42, CMD_SHOW_JPG, 0));
        assert_eq!(r.encode().len(), FRAME_OVERHEAD);
    }

    #[test]
    fn any_chunking_gives_the_same_frames() {
        let frames: Vec<Frame> = (0..5)
            .map(|i| Frame { id: i, cmd: if i % 2 == 0 { CMD_JSON } else { CMD_SHOW_JPG }, data: vec![i as u8; 1 + i as usize * 300] })
            .collect();
        let wire: Vec<u8> = frames.iter().flat_map(Frame::encode).collect();
        for chunk in [1usize, 2, 7, 19, 64, 4096] {
            let mut d = FrameDecoder::new();
            let mut got = Vec::new();
            for piece in wire.chunks(chunk) {
                d.push(piece);
                while let Some(f) = d.next_frame() {
                    got.push(f);
                }
            }
            assert_eq!(got, frames, "chunk size {chunk}");
        }
    }

    #[test]
    fn noise_in_front_and_between_frames_is_skipped() {
        let a = Frame { id: 1, cmd: CMD_JSON, data: b"{}".to_vec() };
        let b = Frame { id: 2, cmd: CMD_JSON, data: b"[]".to_vec() };
        let mut wire = vec![0u8; 1000]; // the host's start-up padding of zero bytes
        wire.extend_from_slice(&a.encode());
        wire.extend_from_slice(&[0xA1, 0xA5, 0x00, 0x13, 0x37]); // a header-looking stub
        wire.extend_from_slice(&b.encode());
        let mut d = FrameDecoder::new();
        d.push(&wire);
        assert_eq!(d.next_frame(), Some(a));
        assert_eq!(d.next_frame(), Some(b));
        assert_eq!(d.next_frame(), None);
    }

    #[test]
    fn a_bad_size_crc_or_data_crc_drops_the_frame_and_finds_the_next() {
        let good = Frame { id: 9, cmd: CMD_JSON, data: b"{\"x\":1}".to_vec() };
        let mut bad_size = Frame { id: 1, cmd: CMD_JSON, data: b"abc".to_vec() }.encode();
        bad_size[16] ^= 0xFF;
        let mut bad_data = Frame { id: 2, cmd: CMD_JSON, data: b"abc".to_vec() }.encode();
        let n = bad_data.len();
        bad_data[n - 1] ^= 0x01;
        let mut d = FrameDecoder::new();
        d.push(&bad_size);
        d.push(&bad_data);
        d.push(&good.encode());
        assert_eq!(d.next_frame(), Some(good));
        assert!(d.dropped() >= 2);
    }

    #[test]
    fn an_absurd_size_is_noise_not_an_allocation() {
        let mut w = Frame { id: 1, cmd: CMD_JSON, data: vec![] }.encode();
        let huge = (MAX_DATA_LEN as u32 + 1).to_le_bytes();
        w[12..16].copy_from_slice(&huge);
        w[16..20].copy_from_slice(&crc32(&huge).to_le_bytes()); // a valid size CRC for a huge size
        let mut d = FrameDecoder::new();
        d.push(&w);
        assert_eq!(d.next_frame(), None);
        assert!(d.dropped() >= 1);
    }

    #[test]
    fn a_header_split_across_pushes_is_not_lost() {
        let f = Frame { id: 3, cmd: CMD_JSON, data: b"{}".to_vec() };
        let w = f.encode();
        let mut d = FrameDecoder::new();
        d.push(&w[..2]);
        assert_eq!(d.next_frame(), None);
        d.push(&w[2..]);
        assert_eq!(d.next_frame(), Some(f));
    }

    #[test]
    fn garbage_never_panics() {
        let mut x: u32 = 0xC0FF_EE11;
        let mut data = Vec::new();
        for _ in 0..50_000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            data.push(x as u8);
        }
        let mut d = FrameDecoder::new();
        for c in data.chunks(333) {
            d.push(c);
            while d.next_frame().is_some() {}
        }
    }
}
