//! The decoder against real bytes read from the SK-18's UART1 (tests/data/capture-115200.hex).

use std::collections::{HashMap, HashSet};

use sk18_keys::{Decoder, Frame, KeyEvent, Orientation};

fn capture() -> Vec<u8> {
    include_str!("data/capture-115200.hex")
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .flat_map(str::split_whitespace)
        .map(|t| u8::from_str_radix(t, 16).expect("hex byte"))
        .collect()
}

fn decode(chunk: usize) -> (Vec<KeyEvent>, sk18_keys::Stats) {
    let data = capture();
    let mut dec = Decoder::new();
    let mut events = Vec::new();
    for c in data.chunks(chunk) {
        dec.push(c, |f| match f {
            Frame::Key(k) => events.push(k),
            Frame::Unknown(p) => panic!("unexpected frame type in the capture: {p:02x?}"),
        });
    }
    (events, dec.stats())
}

#[test]
fn the_whole_capture_decodes_without_loss() {
    assert_eq!(capture().len(), 1768);
    let (events, stats) = decode(1768);
    assert_eq!(events.len(), 136);
    assert_eq!(stats.frames, 136);
    assert_eq!(stats.skipped_bytes, 0);
    assert_eq!(stats.unknown_frames, 0);
}

#[test]
fn the_chunk_size_does_not_matter() {
    let whole = decode(1768).0;
    for chunk in [1, 2, 3, 7, 13, 64, 255] {
        assert_eq!(decode(chunk).0, whole, "chunk size {chunk}");
    }
}

#[test]
fn all_eighteen_keys_with_their_codes() {
    // Row-major from the firmware's view: (row, col) -> HID usage, from docs/uart1-key-protocol.md.
    let expected: HashMap<(u8, u8), u8> = [
        ((0, 0), 0x27), ((0, 1), 0x1e), ((0, 2), 0x1f),
        ((1, 0), 0x20), ((1, 1), 0x21), ((1, 2), 0x22),
        ((2, 0), 0x04), ((2, 1), 0x05), ((2, 2), 0x06),
        ((3, 0), 0x07), ((3, 1), 0x08), ((3, 2), 0x09),
        ((4, 0), 0x0b), ((4, 1), 0x0c), ((4, 2), 0x0d),
        ((5, 0), 0x0e), ((5, 1), 0x0f), ((5, 2), 0x10),
    ]
    .into_iter()
    .collect();
    let (events, _) = decode(64);
    let mut seen = HashSet::new();
    for e in &events {
        assert_eq!(expected.get(&(e.row, e.col)), Some(&e.code), "key ({},{})", e.row, e.col);
        seen.insert((e.row, e.col));
    }
    assert_eq!(seen.len(), 18);
}

#[test]
fn presses_and_releases_alternate_for_every_key() {
    let (events, _) = decode(64);
    let mut down: HashMap<(u8, u8), bool> = HashMap::new();
    for e in &events {
        let was = down.insert((e.row, e.col), e.pressed).unwrap_or(false);
        assert_ne!(was, e.pressed, "key ({},{}) repeated a state", e.row, e.col);
    }
    assert!(down.values().all(|&d| !d), "every key must end released");
}

#[test]
fn the_logical_grid_of_the_device_as_used_is_fully_covered() {
    let (events, _) = decode(64);
    let o = Orientation::Cw90;
    let (rows, cols) = o.grid();
    let seen: HashSet<(u8, u8)> = events.iter().map(|e| o.to_logical(e.row, e.col).unwrap()).collect();
    assert_eq!(seen.len(), usize::from(rows) * usize::from(cols));
}
