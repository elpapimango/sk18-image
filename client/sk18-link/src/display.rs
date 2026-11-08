//! JPEG to framebuffer pixels (no operating system involved, so it is testable on any machine).

use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Decodes `jpeg`, which must be exactly `width` x `height`, into `out` as 32-bit XRGB8888 rows of `stride` bytes
/// (the kernel's framebuffer format: bytes B, G, R, X in memory). Row padding beyond `width * 4` is left as it is.
pub fn jpeg_to_xrgb(jpeg: &[u8], width: usize, height: usize, stride: usize, out: &mut Vec<u8>) -> Result<(), String> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(jpeg, options);
    let rgb = decoder.decode().map_err(|e| format!("cannot decode the JPEG: {e:?}"))?;
    let info = decoder.info().ok_or("the JPEG has no header")?;
    let (w, h) = (usize::from(info.width), usize::from(info.height));
    if (w, h) != (width, height) {
        return Err(format!("the picture is {w}x{h}, the screen is {width}x{height}"));
    }
    if rgb.len() != w * h * 3 || stride < w * 4 {
        return Err(format!("unexpected decoded size {} for {w}x{h} (stride {stride})", rgb.len()));
    }

    out.resize(stride * height, 0);
    for (src, dst) in rgb.chunks_exact(w * 3).zip(out.chunks_exact_mut(stride)) {
        for (s, d) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
            d.copy_from_slice(&[s[2], s[1], s[0], 0]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUADRANTS: &[u8] = include_bytes!("../tests/data/quadrants-720x1280.jpg");
    const W: usize = 720;
    const H: usize = 1280;

    /// RGB of a pixel of the XRGB buffer.
    fn px(buf: &[u8], stride: usize, x: usize, y: usize) -> [u8; 3] {
        let i = y * stride + x * 4;
        [buf[i + 2], buf[i + 1], buf[i]]
    }

    fn near(got: [u8; 3], want: [u8; 3]) -> bool {
        got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 16)
    }

    #[test]
    fn the_quadrants_come_out_in_the_right_places_and_channels() {
        let mut out = Vec::new();
        jpeg_to_xrgb(QUADRANTS, W, H, W * 4, &mut out).unwrap();
        assert_eq!(out.len(), W * H * 4);
        let at = |x, y| px(&out, W * 4, x, y);
        assert!(near(at(180, 320), [255, 0, 0]), "top left should be red, got {:?}", at(180, 320));
        assert!(near(at(540, 320), [0, 255, 0]), "top right should be green, got {:?}", at(540, 320));
        assert!(near(at(180, 960), [0, 0, 255]), "bottom left should be blue, got {:?}", at(180, 960));
        assert!(near(at(540, 960), [255, 255, 255]), "bottom right should be white, got {:?}", at(540, 960));
    }

    #[test]
    fn the_unused_byte_of_every_pixel_is_zero() {
        let mut out = Vec::new();
        jpeg_to_xrgb(QUADRANTS, W, H, W * 4, &mut out).unwrap();
        assert!(out.chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn a_stride_wider_than_the_row_keeps_the_padding_untouched() {
        let stride = W * 4 + 32;
        let mut out = vec![0x77u8; stride * H];
        jpeg_to_xrgb(QUADRANTS, W, H, stride, &mut out).unwrap();
        assert_eq!(out.len(), stride * H);
        for y in [0, 500, H - 1] {
            assert!(out[y * stride + W * 4..(y + 1) * stride].iter().all(|&b| b == 0x77), "row {y}");
        }
        assert!(near(px(&out, stride, 180, 320), [255, 0, 0]));
    }

    #[test]
    fn a_picture_of_the_wrong_size_is_refused_with_a_clear_message() {
        let mut out = Vec::new();
        let err = jpeg_to_xrgb(QUADRANTS, 1280, 720, 1280 * 4, &mut out).unwrap_err();
        assert!(err.contains("720x1280") && err.contains("1280x720"), "{err}");
    }

    #[test]
    fn garbage_and_truncated_pictures_are_errors_not_panics() {
        let mut out = Vec::new();
        assert!(jpeg_to_xrgb(&[], W, H, W * 4, &mut out).is_err());
        assert!(jpeg_to_xrgb(b"this is not a jpeg at all", W, H, W * 4, &mut out).is_err());
        let cut = &QUADRANTS[..QUADRANTS.len() / 3];
        // a truncated JPEG may decode partially or fail; it must not panic, and whatever happens must be consistent
        if let Ok(()) = jpeg_to_xrgb(cut, W, H, W * 4, &mut out) {
            assert_eq!(out.len(), W * H * 4);
        }
    }
}
