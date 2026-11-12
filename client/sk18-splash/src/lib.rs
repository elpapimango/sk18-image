//! Boot animation of the Waveshare SK-18, drawn into a 32-bit framebuffer.
//!
//! The device is used turned 90 degrees clockwise, so the panel's native (portrait, 720 x 1280) buffer holds a
//! landscape picture sideways. Everything is therefore laid out on a *logical* canvas, the way the user sees it
//! (a grid of 3 rows by 6 columns of keys), and each rectangle is mapped to the native buffer when it is drawn.
//!
//! Timeline (milliseconds, 2 s in total):
//! ```text
//!    0 ..  500  every key in its own colour (a rainbow sweep over the grid)
//!  500 .. 1100  the colours fade to black while S, K, 1, 8 appear on the four middle keys of the middle row
//! 1100 .. 1600  the letters stay
//! 1600 .. 2000  the letters fade to black
//! ```

use sk18_keys::{layout, Orientation};

/// Total length of the animation.
pub const TOTAL_MS: u32 = 2000;
const COLOURS_END: u32 = 500;
const FADE_IN_END: u32 = 1100;
const HOLD_END: u32 = 1600;

const WORD: [u8; 4] = *b"SK18";
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;

/// 5 x 7 block letters; a `#` is a solid square of the letter.
fn glyph(ch: u8) -> [&'static str; GLYPH_H] {
    match ch {
        b'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        b'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        b'1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
        b'8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        _ => ["....."; GLYPH_H],
    }
}

/// A 32-bit framebuffer in the panel's native orientation. Pixels are written as `0x00RRGGBB` (the kernel's
/// XRGB8888, little endian: bytes B, G, R, X in memory).
pub struct Surface<'a> {
    /// The pixel memory, at least `stride * height` bytes.
    pub data: &'a mut [u8],
    /// Width in pixels (720 on the SK-18).
    pub width: usize,
    /// Height in pixels (1280 on the SK-18).
    pub height: usize,
    /// Bytes per row.
    pub stride: usize,
}

/// An RGB colour.
pub type Rgb = [u8; 3];

/// 0 to 1 and back, with soft ends: `t` outside `[a, b]` is clamped.
pub fn smoothstep(a: u32, b: u32, t: u32) -> f32 {
    if t <= a {
        return 0.0;
    }
    if t >= b {
        return 1.0;
    }
    let x = (t - a) as f32 / (b - a) as f32;
    x * x * (3.0 - 2.0 * x)
}

/// Fully saturated colour of hue `deg` (0 to 360).
pub fn hue(deg: f32) -> Rgb {
    let h = (deg % 360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [
        (r * 255.0 + 0.5) as u8,
        (g * 255.0 + 0.5) as u8,
        (b * 255.0 + 0.5) as u8,
    ]
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t + 0.5) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Colour of the key with the given index in reading order (0 = top left as seen, 17 = bottom right):
/// a sweep from red through yellow, green and blue to magenta, 300 degrees over the whole grid.
pub fn key_colour(index: usize, keys: usize) -> Rgb {
    hue(300.0 * index as f32 / (keys.max(2) - 1) as f32)
}

/// The brightness of a key's background and the strength of the letters at time `t`.
fn levels(t: u32) -> (f32, f32) {
    let bg = 1.0 - smoothstep(COLOURS_END, FADE_IN_END, t);
    let letter_in = smoothstep(COLOURS_END, FADE_IN_END, t);
    let letter_out = 1.0 - smoothstep(HOLD_END, TOTAL_MS, t);
    (bg, letter_in * letter_out)
}

/// Size of the picture the user sees, as `(width, height)`, for a native buffer of `width` x `height`.
pub fn logical_size(orientation: Orientation, width: usize, height: usize) -> (usize, usize) {
    match orientation {
        Orientation::Portrait | Orientation::Cw180 => (width, height),
        Orientation::Cw90 | Orientation::Cw270 => (height, width),
    }
}

/// Maps a point of the logical canvas to the native buffer. `(nw, nh)` is the native size.
pub fn to_native(orientation: Orientation, nw: usize, nh: usize, x: usize, y: usize) -> (usize, usize) {
    match orientation {
        Orientation::Portrait => (x, y),
        Orientation::Cw90 => (y, nh - 1 - x),
        Orientation::Cw180 => (nw - 1 - x, nh - 1 - y),
        Orientation::Cw270 => (nw - 1 - y, x),
    }
}

/// The inverse of [`to_native`]: the logical point shown at native `(x, y)`.
pub fn to_logical_point(
    orientation: Orientation,
    nw: usize,
    nh: usize,
    x: usize,
    y: usize,
) -> (usize, usize) {
    match orientation {
        Orientation::Portrait => (x, y),
        Orientation::Cw90 => (nh - 1 - y, x),
        Orientation::Cw180 => (nw - 1 - x, nh - 1 - y),
        Orientation::Cw270 => (y, nw - 1 - x),
    }
}

impl Surface<'_> {
    fn fill_logical(&mut self, o: Orientation, x0: usize, y0: usize, x1: usize, y1: usize, c: Rgb) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (ax, ay) = to_native(o, self.width, self.height, x0, y0);
        let (bx, by) = to_native(o, self.width, self.height, x1 - 1, y1 - 1);
        let (nx0, nx1) = (ax.min(bx), ax.max(bx));
        let (ny0, ny1) = (ay.min(by), ay.max(by));
        let px = [c[2], c[1], c[0], 0];
        for y in ny0..=ny1.min(self.height - 1) {
            let row = &mut self.data[y * self.stride..y * self.stride + self.width * 4];
            for x in nx0..=nx1.min(self.width - 1) {
                row[x * 4..x * 4 + 4].copy_from_slice(&px);
            }
        }
    }
}

/// Side of one square of a letter, in logical pixels, for a key of `cell_w` x `cell_h`: large enough that a letter
/// fills most of its key (a letter is 5 x 7 squares) with a margin of about one square.
pub fn block_size(cell_w: usize, cell_h: usize) -> usize {
    (cell_w / (GLYPH_W + 1)).min(cell_h / (GLYPH_H + 1)).max(1)
}

/// The letter shown on the key at `(r, c)` of a `rows` x `cols` grid, if any. On the landscape grid of the SK-18
/// (3 x 6) the word runs along the middle row, centred: S K 1 8 on columns 1 to 4. On a grid too narrow for that
/// (the portrait ones, 6 x 3) it runs down the middle column instead.
fn letter_at(rows: usize, cols: usize, r: usize, c: usize) -> Option<u8> {
    let n = WORD.len();
    if cols >= n {
        let start = (cols - n) / 2;
        (r == rows / 2 && c >= start && c < start + n).then(|| WORD[c - start])
    } else if rows >= n {
        let start = (rows - n) / 2;
        (c == cols / 2 && r >= start && r < start + n).then(|| WORD[r - start])
    } else {
        None
    }
}

/// Draws the frame for time `t` (milliseconds since the start) over the whole surface.
pub fn render(s: &mut Surface<'_>, o: Orientation, t: u32) {
    let (rows, cols) = o.grid();
    let (rows, cols) = (usize::from(rows), usize::from(cols));
    let (lw, lh) = logical_size(o, s.width, s.height);
    let (bg_level, letter_level) = levels(t);

    for r in 0..rows {
        for c in 0..cols {
            // The key window this cell of the picture belongs to (found through the uniform tiling, so any
            // orientation works), and its measured place on the panel.
            let (ux, uy) = ((c * 2 + 1) * lw / (cols * 2), (r * 2 + 1) * lh / (rows * 2));
            let (nx, ny) = to_native(o, s.width, s.height, ux, uy);
            let (kc, kr) = (
                (nx * layout::COLS / s.width).min(layout::COLS - 1),
                (ny * layout::ROWS / s.height).min(layout::ROWS - 1),
            );
            let (kcx, kcy) = layout::key_centre(kc, kr);
            // The colour fills the space up to the middle between neighbouring keys (and to the screen's edge).
            let span = |centre: usize, k: usize, count: usize, size: usize| {
                let lo = if k == 0 { 0 } else { centre - layout::PITCH / 2 };
                let hi = if k + 1 == count {
                    size
                } else {
                    centre + layout::PITCH / 2
                };
                (lo, hi)
            };
            let (nx0, nx1) = span(kcx, kc, layout::COLS, s.width);
            let (ny0, ny1) = span(kcy, kr, layout::ROWS, s.height);
            let (ax, ay) = to_logical_point(o, s.width, s.height, nx0, ny0);
            let (bx, by) = to_logical_point(o, s.width, s.height, nx1 - 1, ny1 - 1);
            let (x0, x1) = (ax.min(bx), ax.max(bx) + 1);
            let (y0, y1) = (ay.min(by), ay.max(by) + 1);
            let bg = lerp([0, 0, 0], key_colour(r * cols + c, rows * cols), bg_level);
            s.fill_logical(o, x0, y0, x1, y1, bg);

            if let Some(letter) = letter_at(rows, cols, r, c).filter(|_| letter_level > 0.0) {
                let ink = lerp(bg, [255, 255, 255], letter_level);
                let block = block_size(layout::WINDOW, layout::WINDOW);
                let (cx, cy) = to_logical_point(o, s.width, s.height, kcx, kcy);
                let (gx, gy) = (
                    cx.saturating_sub(GLYPH_W * block / 2),
                    cy.saturating_sub(GLYPH_H * block / 2),
                );
                for (gr, line) in glyph(letter).iter().enumerate() {
                    for (gc, ch) in line.bytes().enumerate() {
                        if ch == b'#' {
                            let (bx, by) = (gx + gc * block, gy + gr * block);
                            s.fill_logical(o, bx, by, bx + block, by + block, ink);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 720;
    const H: usize = 1280;

    fn buffer() -> Vec<u8> {
        vec![0xAA; W * H * 4]
    }

    fn frame(o: Orientation, t: u32) -> Vec<u8> {
        let mut data = buffer();
        let mut s = Surface {
            data: &mut data,
            width: W,
            height: H,
            stride: W * 4,
        };
        render(&mut s, o, t);
        data
    }

    /// RGB of a native pixel.
    fn px(data: &[u8], x: usize, y: usize) -> Rgb {
        let i = (y * W + x) * 4;
        [data[i + 2], data[i + 1], data[i]]
    }

    /// RGB of a point of the logical canvas.
    fn at(data: &[u8], o: Orientation, x: usize, y: usize) -> Rgb {
        let (lw, lh) = logical_size(o, W, H);
        assert!(x < lw && y < lh);
        let (nx, ny) = to_native(o, W, H, x, y);
        px(data, nx, ny)
    }

    /// Top left of the glyph on the logical cell `(r, c)` of the Cw90 grid, and its block size, worked out from the
    /// measured key windows: logical row `r` is native column `r`, logical column `c` is native row `5 - c`.
    fn glyph_origin(o: Orientation, r: usize, c: usize) -> (usize, usize, usize) {
        let (cx, cy) = layout::key_centre(r, layout::ROWS - 1 - c);
        let (lx, ly) = to_logical_point(o, W, H, cx, cy);
        let block = block_size(layout::WINDOW, layout::WINDOW);
        (lx - 5 * block / 2, ly - 7 * block / 2, block)
    }

    #[test]
    fn easing_has_flat_ends_and_is_monotonic() {
        assert_eq!(smoothstep(500, 1100, 0), 0.0);
        assert_eq!(smoothstep(500, 1100, 500), 0.0);
        assert_eq!(smoothstep(500, 1100, 1100), 1.0);
        let mut last = 0.0;
        for t in 500..=1100 {
            let v = smoothstep(500, 1100, t);
            assert!(v >= last);
            last = v;
        }
        assert!((smoothstep(500, 1100, 800) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn hue_hits_the_primaries() {
        assert_eq!(hue(0.0), [255, 0, 0]);
        assert_eq!(hue(120.0), [0, 255, 0]);
        assert_eq!(hue(240.0), [0, 0, 255]);
        assert_eq!(hue(60.0), [255, 255, 0]);
    }

    #[test]
    fn at_the_start_all_eighteen_keys_have_different_colours() {
        let o = Orientation::Cw90;
        let d = frame(o, 0);
        let (lw, lh) = logical_size(o, W, H);
        let (rows, cols) = o.grid();
        let mut seen = std::collections::HashSet::new();
        for r in 0..usize::from(rows) {
            for c in 0..usize::from(cols) {
                // the centre of the cell, away from the letters' area is not needed at t = 0
                let colour = at(
                    &d,
                    o,
                    (c * 2 + 1) * lw / (usize::from(cols) * 2),
                    (r * 2 + 1) * lh / (usize::from(rows) * 2),
                );
                assert!(colour != [0, 0, 0], "cell ({r},{c}) is black");
                seen.insert(colour);
            }
        }
        assert_eq!(seen.len(), 18);
    }

    #[test]
    fn the_sweep_starts_red_and_ends_magenta() {
        let o = Orientation::Cw90;
        let d = frame(o, 100);
        let (lw, lh) = logical_size(o, W, H);
        assert_eq!(at(&d, o, 10, 10), [255, 0, 0], "top left of what the user sees");
        let end = at(&d, o, lw - 10, lh - 10);
        assert_eq!(end, hue(300.0), "bottom right");
    }

    #[test]
    fn the_logical_top_left_is_the_native_bottom_left() {
        // Firmware key (5,0) is the logical (0,0) (decision D-009 / the key reader tests): it must be the first
        // key of the sweep, red, and it sits at the bottom left of the native portrait buffer.
        let d = frame(Orientation::Cw90, 100);
        assert_eq!(Orientation::Cw90.to_logical(5, 0), Some((0, 0)));
        assert_eq!(px(&d, 10, H - 10), [255, 0, 0]);
        // and the logical bottom right (key 17) is the native top right
        assert_eq!(px(&d, W - 10, 10), hue(300.0));
    }

    #[test]
    fn the_whole_buffer_is_covered_and_black_at_the_end() {
        let d = frame(Orientation::Cw90, TOTAL_MS);
        for y in 0..H {
            for x in 0..W {
                assert_eq!(px(&d, x, y), [0, 0, 0], "pixel ({x},{y}) is not black at the end");
            }
        }
        // t = 0 also covers everything (the 0xAA fill is gone)
        let d = frame(Orientation::Cw90, 0);
        assert!(d.chunks(4).all(|p| p[0] != 0xAA || p[1] != 0xAA || p[2] != 0xAA));
    }

    #[test]
    fn during_the_hold_only_the_four_letters_are_lit() {
        let o = Orientation::Cw90;
        let d = frame(o, 1300);
        let (lw, lh) = logical_size(o, W, H);
        let (cols, rows) = (6, 3);
        let mut white = 0usize;
        let mut other = 0usize;
        for y in 0..lh {
            for x in 0..lw {
                match at(&d, o, x, y) {
                    [255, 255, 255] => white += 1,
                    [0, 0, 0] => {}
                    _ => other += 1,
                }
            }
        }
        assert_eq!(other, 0, "nothing but black and white while holding");
        assert!(white > 0);
        // and every lit pixel is in the middle row, columns 1 to 4
        for y in 0..lh {
            for x in 0..lw {
                if at(&d, o, x, y) == [255, 255, 255] {
                    let (r, c) = (y * rows / lh, x * cols / lw);
                    assert_eq!(r, 1, "lit pixel outside the middle row at ({x},{y})");
                    assert!(
                        (1..=4).contains(&c),
                        "lit pixel outside columns 1..4 at ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn each_letter_is_drawn_from_its_bitmap_upright() {
        // Read the letters back from the logical canvas and compare with the bitmaps.
        let o = Orientation::Cw90;
        let d = frame(o, 1300);
        for (i, ch) in b"SK18".iter().enumerate() {
            let (gx, gy, block) = glyph_origin(o, 1, 1 + i);
            for (gr, line) in glyph(*ch).iter().enumerate() {
                for (gc, b) in line.bytes().enumerate() {
                    let centre = at(&d, o, gx + gc * block + block / 2, gy + gr * block + block / 2);
                    let want = if b == b'#' { [255, 255, 255] } else { [0, 0, 0] };
                    assert_eq!(centre, want, "letter {} row {gr} col {gc}", *ch as char);
                }
            }
        }
    }

    #[test]
    fn halfway_through_the_fade_the_keys_are_dimmed_and_the_letters_half_lit() {
        let o = Orientation::Cw90;
        let d = frame(o, 800); // the middle of 500..1100
                               // a key that is not a letter key: half of its colour (smoothstep(0.5) = 0.5)
        let want = lerp([0, 0, 0], key_colour(0, 18), 0.5);
        assert_eq!(at(&d, o, 10, 10), want);
        // the letter blocks are half way between the (dimmed) key colour and white
        let (gx, gy, block) = glyph_origin(o, 1, 1);
        // 'S' row 0, column 1 is a solid block
        let got = at(&d, o, gx + block + block / 2, gy + block / 2);
        let bg = lerp([0, 0, 0], key_colour(6 + 1, 18), 0.5);
        assert_eq!(got, lerp(bg, [255, 255, 255], 0.5));
    }

    #[test]
    fn every_orientation_covers_the_buffer_without_panicking() {
        for o in [
            Orientation::Portrait,
            Orientation::Cw90,
            Orientation::Cw180,
            Orientation::Cw270,
        ] {
            for t in [0, 499, 500, 800, 1100, 1300, 1600, 1800, 2000, 5000] {
                let d = frame(o, t);
                assert!(d.chunks(4).all(|p| p[3] == 0xAA || p[3] == 0), "{o:?} at {t}");
                // no pixel keeps the 0xAA fill in all three colour bytes
                assert!(
                    d.chunks(4)
                        .all(|p| !(p[0] == 0xAA && p[1] == 0xAA && p[2] == 0xAA)),
                    "{o:?} at {t}"
                );
            }
        }
    }

    #[test]
    fn a_stride_larger_than_the_row_is_respected() {
        let stride = W * 4 + 64;
        let mut data = vec![0x55u8; stride * H];
        let mut s = Surface {
            data: &mut data,
            width: W,
            height: H,
            stride,
        };
        render(&mut s, Orientation::Cw90, 0);
        // the padding at the end of each row is untouched
        for y in 0..H {
            assert!(
                data[y * stride + W * 4..(y + 1) * stride]
                    .iter()
                    .all(|&b| b == 0x55),
                "row {y} padding"
            );
        }
    }
}
