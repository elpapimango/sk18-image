//! The real hardware: the framebuffer (`/dev/fb0`) and the backlight (sysfs).

use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::device::Hardware;
use crate::display::jpeg_to_xrgb;

/// Framebuffer and backlight of the SK-18.
pub struct LinuxHardware {
    fb: File,
    width: usize,
    height: usize,
    stride: usize,
    buf: Vec<u8>,
    backlight: PathBuf,
    verbose: bool,
}

impl LinuxHardware {
    /// Opens `fb` (for example `/dev/fb0`) and reads its geometry from sysfs; `backlight` is the sysfs `brightness`
    /// file. With `verbose`, the time each picture takes is printed.
    pub fn open(fb: &str, backlight: &str, verbose: bool) -> Result<Self, String> {
        let name = Path::new(fb).file_name().and_then(|n| n.to_str()).ok_or("bad framebuffer path")?;
        let info = |file: &str| -> Result<String, String> {
            let p = format!("/sys/class/graphics/{name}/{file}");
            fs::read_to_string(&p).map(|s| s.trim().to_string()).map_err(|e| format!("{p}: {e}"))
        };
        let size = info("virtual_size")?;
        let (w, h) = size.split_once(',').ok_or_else(|| format!("unexpected virtual_size {size:?}"))?;
        let width: usize = w.parse().map_err(|_| format!("bad width {w:?}"))?;
        let height: usize = h.parse().map_err(|_| format!("bad height {h:?}"))?;
        let bpp: usize = info("bits_per_pixel")?.parse().map_err(|_| "bad bits_per_pixel".to_string())?;
        let stride: usize = info("stride")?.parse().map_err(|_| "bad stride".to_string())?;
        if bpp != 32 || stride < width * 4 {
            return Err(format!("unsupported framebuffer: {width}x{height}, {bpp} bpp, stride {stride}"));
        }
        let fb = OpenOptions::new().write(true).open(fb).map_err(|e| format!("{fb}: {e}"))?;
        Ok(Self { fb, width, height, stride, buf: Vec::new(), backlight: PathBuf::from(backlight), verbose })
    }
}

impl Hardware for LinuxHardware {
    fn set_backlight_percent(&mut self, percent: u8) -> Result<(), String> {
        // The PWM backlight takes 0 to 255.
        let level = (u32::from(percent.min(100)) * 255 + 50) / 100;
        fs::write(&self.backlight, level.to_string()).map_err(|e| format!("{}: {e}", self.backlight.display()))
    }

    fn show_jpeg(&mut self, jpeg: &[u8]) -> Result<(), String> {
        let t0 = Instant::now();
        jpeg_to_xrgb(jpeg, self.width, self.height, self.stride, &mut self.buf)?;
        let t1 = Instant::now();
        self.fb.write_all_at(&self.buf, 0).map_err(|e| format!("framebuffer: {e}"))?;
        if self.verbose {
            eprintln!(
                "sk18-link: picture of {} bytes: decode {} ms, write {} ms",
                jpeg.len(),
                (t1 - t0).as_millis(),
                t1.elapsed().as_millis()
            );
        }
        Ok(())
    }
}
