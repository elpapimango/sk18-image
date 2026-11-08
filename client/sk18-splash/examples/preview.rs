//! Writes the frames of the animation as the user sees them (rotated back from the panel's native orientation)
//! to PPM files, for checking the look without a device:  cargo run --example preview -- OUT_DIR [90]

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use sk18_keys::Orientation;
use sk18_splash::{logical_size, render, to_native, Surface};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: preview OUT_DIR [orientation]"));
    let o = args.next().and_then(|a| Orientation::parse(&a)).unwrap_or_default();
    fs::create_dir_all(&out).unwrap();

    let (w, h) = (720usize, 1280usize);
    let (lw, lh) = logical_size(o, w, h);
    for t in [0u32, 250, 550, 800, 1050, 1300, 1700, 1900, 2000] {
        let mut native = vec![0u8; w * h * 4];
        let mut s = Surface { data: &mut native, width: w, height: h, stride: w * 4 };
        render(&mut s, o, t);

        let mut ppm = format!("P6\n{lw} {lh}\n255\n").into_bytes();
        for y in 0..lh {
            for x in 0..lw {
                let (nx, ny) = to_native(o, w, h, x, y);
                let i = (ny * w + nx) * 4;
                ppm.extend_from_slice(&[native[i + 2], native[i + 1], native[i]]);
            }
        }
        let path = out.join(format!("frame_{t:04}.ppm"));
        fs::File::create(&path).unwrap().write_all(&ppm).unwrap();
    }
    println!("wrote 9 frames of {lw}x{lh} to {}", out.display());
}
