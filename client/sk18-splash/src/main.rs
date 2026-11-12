//! `sk18-splash`: plays the boot animation on the framebuffer, turning the backlight on after the first frame.

use std::process::ExitCode;

use sk18_keys::Orientation;

const USAGE: &str = "\
usage: sk18-splash [--fb /dev/fb0] [--backlight PATH] [--brightness 0-255] [--orientation 0|90|180|270]

Draws the 2 second boot animation on the framebuffer. The backlight is switched on (to --brightness, default 128)
only after the first frame is on the screen, so the panel stays dark until there is something to show, and it is
left on at the end (the screen is black; the next program takes over).";

struct Options {
    fb: String,
    backlight: String,
    brightness: u32,
    orientation: Orientation,
}

fn parse_args() -> Result<Option<Options>, String> {
    let mut o = Options {
        fb: "/dev/fb0".into(),
        backlight: "/sys/class/backlight/backlight/brightness".into(),
        brightness: 128,
        orientation: Orientation::default(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--help" | "-h" => return Ok(None),
            "--fb" => o.fb = value("--fb")?,
            "--backlight" => o.backlight = value("--backlight")?,
            "--brightness" => {
                let v = value("--brightness")?;
                o.brightness = v
                    .parse::<u32>()
                    .ok()
                    .filter(|b| *b <= 255)
                    .ok_or_else(|| format!("bad brightness {v:?}"))?;
            }
            "--orientation" => {
                let v = value("--orientation")?;
                o.orientation = Orientation::parse(&v).ok_or_else(|| format!("unknown orientation {v:?}"))?;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Some(o))
}

#[cfg(unix)]
fn run(o: &Options) -> Result<(), String> {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::FileExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use sk18_splash::{render, Surface, TOTAL_MS};

    const FRAME: Duration = Duration::from_millis(33);

    // Size and layout of the framebuffer, from sysfs (no ioctl needed).
    let name = Path::new(&o.fb)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad framebuffer path")?;

    // The display driver may still be finishing its probe when init starts this program: wait up to 5 s for the
    // framebuffer, its sysfs entries and the backlight. After that the real error comes from the reads below.
    let ready = || {
        Path::new(&o.fb).exists()
            && Path::new(&format!("/sys/class/graphics/{name}/virtual_size")).exists()
            && Path::new(&o.backlight).exists()
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }

    let info = |file: &str| -> Result<String, String> {
        let p = format!("/sys/class/graphics/{name}/{file}");
        fs::read_to_string(&p)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("{p}: {e}"))
    };
    let size = info("virtual_size")?;
    let (w, h) = size
        .split_once(',')
        .ok_or_else(|| format!("unexpected virtual_size {size:?}"))?;
    let (width, height): (usize, usize) = (
        w.parse().map_err(|_| format!("bad width {w:?}"))?,
        h.parse().map_err(|_| format!("bad height {h:?}"))?,
    );
    let bpp: usize = info("bits_per_pixel")?
        .parse()
        .map_err(|_| "bad bits_per_pixel".to_string())?;
    let stride: usize = info("stride")?.parse().map_err(|_| "bad stride".to_string())?;
    if bpp != 32 || stride < width * 4 {
        return Err(format!(
            "unsupported framebuffer: {width}x{height}, {bpp} bpp, stride {stride}"
        ));
    }

    let fb = OpenOptions::new()
        .write(true)
        .open(&o.fb)
        .map_err(|e| format!("{}: {e}", o.fb))?;
    let mut buf = vec![0u8; stride * height];
    let mut draw = |t: u32| -> Result<(), String> {
        let mut s = Surface {
            data: &mut buf,
            width,
            height,
            stride,
        };
        render(&mut s, o.orientation, t);
        fb.write_all_at(&buf, 0).map_err(|e| format!("{}: {e}", o.fb))
    };

    // First frame, then the backlight, then the clock starts: the first 500 ms are not eaten by the set-up.
    draw(0)?;
    fs::write(&o.backlight, o.brightness.to_string()).map_err(|e| format!("{}: {e}", o.backlight))?;
    let start = Instant::now();
    loop {
        let t = start.elapsed().as_millis().min(u128::from(TOTAL_MS)) as u32;
        draw(t)?;
        if t >= TOTAL_MS {
            return Ok(());
        }
        std::thread::sleep(FRAME);
    }
}

#[cfg(not(unix))]
fn run(_: &Options) -> Result<(), String> {
    Err("the framebuffer output is only supported on Unix".into())
}

fn main() -> ExitCode {
    match parse_args() {
        Ok(None) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(o)) => match run(&o) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("sk18-splash: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("sk18-splash: {e}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
