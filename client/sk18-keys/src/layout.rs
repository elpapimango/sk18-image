//! Where the 18 key windows are on the panel, in native (portrait, 720 x 1280) pixels.
//!
//! The stock firmware reports keys of 158 px at x = 63 + 218 * col, y = 10 + 218 * row. Photographs of a ruler
//! pattern (decision D-036) show the glass windows are centred about 12 px further along native x and 3 px further
//! along native y than that, and about 170 px wide. This firmware reports and draws the measured positions, so a
//! picture's key lands in the middle of its window.

/// Key columns in the native (portrait) buffer.
pub const COLS: usize = 3;
/// Key rows in the native (portrait) buffer.
pub const ROWS: usize = 6;
/// Distance between neighbouring keys.
pub const PITCH: usize = 218;
/// Side of the square the host draws for one key.
pub const KEY_SIZE: usize = 158;
/// Left edge of the first key's square.
pub const ORIGIN_X: usize = 63 + 12;
/// Top edge of the first key's square.
pub const ORIGIN_Y: usize = 10 + 3;
/// Side of the visible part of a key's glass (a little more than the square).
pub const WINDOW: usize = 170;

/// Top left corner of the key square at native `(col, row)`.
pub fn key_origin(col: usize, row: usize) -> (usize, usize) {
    (ORIGIN_X + PITCH * col, ORIGIN_Y + PITCH * row)
}

/// Centre of the key window at native `(col, row)`.
pub fn key_centre(col: usize, row: usize) -> (usize, usize) {
    let (x, y) = key_origin(col, row);
    (x + KEY_SIZE / 2, y + KEY_SIZE / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_square_is_inside_the_panel() {
        for row in 0..ROWS {
            for col in 0..COLS {
                let (x, y) = key_origin(col, row);
                assert!(x + KEY_SIZE <= 720 && y + KEY_SIZE <= 1280, "({col},{row})");
            }
        }
    }

    #[test]
    fn the_first_and_last_centres() {
        assert_eq!(key_centre(0, 0), (154, 92));
        assert_eq!(key_centre(2, 5), (154 + 436, 92 + 1090));
    }
}
