//! Mapping from the firmware's key numbering to the position the user sees.
//!
//! The controller numbers the keys for the screen held in portrait: 6 rows by 3 columns, row 0 at the top
//! and column 0 on the left of that portrait view. The SK-18 is used turned 90 degrees clockwise (a 3 x 6
//! grid): the observation was that firmware key (0,0) is at the top right, (5,0) at the top left and
//! (0,2) at the bottom right (decision log D-009).

use crate::frame::{COLS, ROWS};

/// How the device is turned relative to the firmware's portrait view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// As the firmware numbers them: 6 rows by 3 columns.
    Portrait,
    /// Turned 90 degrees clockwise: 3 rows by 6 columns (how the SK-18 is used).
    #[default]
    Cw90,
    /// Turned 180 degrees: 6 rows by 3 columns, upside down.
    Cw180,
    /// Turned 270 degrees clockwise: 3 rows by 6 columns.
    Cw270,
}

impl Orientation {
    /// Parses `0`, `90`, `180`, `270` (degrees clockwise) or `portrait` / `cw90` / `cw180` / `cw270`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "0" | "portrait" => Some(Self::Portrait),
            "90" | "cw90" => Some(Self::Cw90),
            "180" | "cw180" => Some(Self::Cw180),
            "270" | "cw270" => Some(Self::Cw270),
            _ => None,
        }
    }

    fn quarter_turns(self) -> u8 {
        match self {
            Self::Portrait => 0,
            Self::Cw90 => 1,
            Self::Cw180 => 2,
            Self::Cw270 => 3,
        }
    }

    /// Size of the logical grid as `(rows, columns)`.
    pub fn grid(self) -> (u8, u8) {
        if self.quarter_turns() % 2 == 0 {
            (ROWS, COLS)
        } else {
            (COLS, ROWS)
        }
    }

    /// Converts a firmware `(row, col)` into the logical `(row, col)`; `None` if it is outside the grid.
    pub fn to_logical(self, row: u8, col: u8) -> Option<(u8, u8)> {
        if row >= ROWS || col >= COLS {
            return None;
        }
        let (mut r, mut c, mut rows, mut cols) = (row, col, ROWS, COLS);
        for _ in 0..self.quarter_turns() {
            // One quarter turn clockwise: the left column becomes the top row.
            (r, c, rows, cols) = (c, rows - 1 - r, cols, rows);
        }
        Some((r, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_observed_corners_of_the_device_as_used() {
        let o = Orientation::Cw90;
        assert_eq!(o.grid(), (3, 6));
        assert_eq!(o.to_logical(0, 0), Some((0, 5)), "red: top right");
        assert_eq!(o.to_logical(5, 0), Some((0, 0)), "blue: top left");
        assert_eq!(o.to_logical(0, 2), Some((2, 5)), "green: bottom right");
        assert_eq!(
            o.to_logical(5, 2),
            Some((2, 0)),
            "the remaining corner: bottom left"
        );
    }

    #[test]
    fn portrait_is_the_identity() {
        for r in 0..ROWS {
            for c in 0..COLS {
                assert_eq!(Orientation::Portrait.to_logical(r, c), Some((r, c)));
            }
        }
    }

    #[test]
    fn every_orientation_is_a_bijection_onto_its_grid() {
        for o in [
            Orientation::Portrait,
            Orientation::Cw90,
            Orientation::Cw180,
            Orientation::Cw270,
        ] {
            let (rows, cols) = o.grid();
            let mut seen = std::collections::HashSet::new();
            for r in 0..ROWS {
                for c in 0..COLS {
                    let (lr, lc) = o.to_logical(r, c).unwrap();
                    assert!(
                        lr < rows && lc < cols,
                        "{o:?}: ({r},{c}) -> ({lr},{lc}) outside {rows}x{cols}"
                    );
                    assert!(seen.insert((lr, lc)), "{o:?}: two keys map to ({lr},{lc})");
                }
            }
            assert_eq!(seen.len(), 18);
        }
    }

    #[test]
    fn four_quarter_turns_make_a_full_turn_and_180_is_two_of_them() {
        // Applying Cw90 twice must equal Cw180 (re-expressed on the 6x3 grid).
        for r in 0..ROWS {
            for c in 0..COLS {
                let (r1, c1) = Orientation::Cw90.to_logical(r, c).unwrap();
                // A second quarter turn on a 3x6 grid: (r, c) -> (c, 3 - 1 - r)
                let (r2, c2) = (c1, COLS - 1 - r1);
                assert_eq!(Orientation::Cw180.to_logical(r, c), Some((r2, c2)));
            }
        }
    }

    #[test]
    fn outside_the_grid_is_rejected() {
        assert_eq!(Orientation::Cw90.to_logical(6, 0), None);
        assert_eq!(Orientation::Cw90.to_logical(0, 3), None);
    }

    #[test]
    fn parsing() {
        assert_eq!(Orientation::parse("90"), Some(Orientation::Cw90));
        assert_eq!(Orientation::parse(" CW270 "), Some(Orientation::Cw270));
        assert_eq!(Orientation::parse("0"), Some(Orientation::Portrait));
        assert_eq!(Orientation::parse("45"), None);
    }
}
