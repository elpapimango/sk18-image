//! `sk18-link`: the SK-18 as a deck for controldeck. Speaks the deck protocol on the USB gadget's serial port
//! (images in, key events out). See decision D-034.

use std::process::ExitCode;

const USAGE: &str = "\
usage: sk18-link [--port /dev/ttyGS0] [--keys /dev/ttyS0] [--fb /dev/fb0] [--backlight PATH] [--verbose]

  --port PATH        the USB gadget serial port that carries the deck protocol (default /dev/ttyGS0)
  --keys PATH        the serial port of the key controller, UART1 (default /dev/ttyS0)
  --fb PATH          the framebuffer (default /dev/fb0)
  --backlight PATH   the sysfs brightness file (default /sys/class/backlight/backlight/brightness)
  --verbose          print what happens to every picture and every connection";

#[cfg(unix)]
mod run {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use sk18_keys::{Decoder as KeyDecoder, Frame as KeyFrame};
    use sk18_link::device::{key_event, Device};
    use sk18_link::hw::LinuxHardware;
    use sk18_link::protocol::{Frame, FrameDecoder};
    use sk18_link::serial::{open_raw, recover, wait_readable};

    pub struct Options {
        pub port: PathBuf,
        pub keys: PathBuf,
        pub fb: String,
        pub backlight: String,
        pub verbose: bool,
    }

    /// The host connection shared by the thread that answers requests and the one that reports key presses.
    type Link = Arc<Mutex<Option<File>>>;

    /// Milliseconds since the first call, for the verbose log.
    fn ms() -> u128 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_millis()
    }

    fn send(link: &Link, frame: &Frame, verbose: bool) {
        if let Some(port) = link.lock().expect("link lock").as_mut() {
            let bytes = frame.encode();
            if verbose {
                eprintln!("sk18-link: [{} ms] reply id {} cmd {} ({} bytes): writing", ms(), frame.id, frame.cmd, bytes.len());
            }
            // A failed write means the host went away; the read side notices and reconnects.
            let r = port.write_all(&bytes);
            if verbose {
                eprintln!("sk18-link: [{} ms] reply id {} written: {:?}", ms(), frame.id, r.map_err(|e| e.to_string()));
            }
        }
    }

    /// Reports every key press and release from the UART1 controller to the host (when there is one).
    fn key_thread(keys: PathBuf, link: Link, verbose: bool) {
        loop {
            match sk18_keys::port::open_port(&keys) {
                Ok(mut port) => {
                    let mut decoder = KeyDecoder::new();
                    let mut buf = [0u8; 128];
                    while let Ok(n) = port.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        decoder.push(&buf[..n], |frame| {
                            if let KeyFrame::Key(k) = frame {
                                if verbose {
                                    eprintln!("sk18-link: key ({}, {}) {}", k.row, k.col, if k.pressed { "down" } else { "up" });
                                }
                                send(&link, &key_event(k.row, k.col, k.pressed), verbose);
                            }
                        });
                    }
                }
                Err(e) => eprintln!("sk18-link: key port {}: {e}", keys.display()),
            }
            thread::sleep(Duration::from_secs(2));
        }
    }

    pub fn main(o: Options) -> Result<(), String> {
        let hw = LinuxHardware::open(&o.fb, &o.backlight, o.verbose)?;
        let mut device = Device::new(hw);
        let link: Link = Arc::new(Mutex::new(None));

        {
            let (keys, link, verbose) = (o.keys.clone(), link.clone(), o.verbose);
            thread::spawn(move || key_thread(keys, link, verbose));
        }

        let mut waiting_reported = false;
        loop {
            let mut port = match open_raw(&o.port) {
                Ok(p) => p,
                Err(e) => {
                    if !waiting_reported {
                        eprintln!("sk18-link: waiting for {}: {e}", o.port.display());
                        waiting_reported = true;
                    }
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };
            waiting_reported = false;
            let writer = port.try_clone().map_err(|e| format!("{}: {e}", o.port.display()))?;
            *link.lock().expect("link lock") = Some(writer);
            if o.verbose {
                eprintln!("sk18-link: {} open", o.port.display());
            }

            let mut decoder = FrameDecoder::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                // If the kernel does not wake us for data it holds, find it here (see `recover`).
                match wait_readable(&port, 20) {
                    Ok(true) => {}
                    Ok(false) => {
                        if !recover(&port) {
                            continue;
                        }
                    }
                    Err(_) => break,
                }
                match port.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        decoder.push(&buf[..n]);
                        while let Some(frame) = decoder.next_frame() {
                            if o.verbose {
                                eprintln!("sk18-link: [{} ms] frame id {} cmd {} ({} bytes) received", ms(), frame.id, frame.cmd, frame.data.len());
                            }
                            for reply in device.handle(&frame) {
                                send(&link, &reply, o.verbose);
                            }
                        }
                    }
                }
            }
            *link.lock().expect("link lock") = None;
            if o.verbose {
                let (shown, failed) = device.counters();
                eprintln!("sk18-link: host disconnected ({shown} pictures shown, {failed} failed); reopening");
            }
            thread::sleep(Duration::from_millis(500));
        }
    }
}

#[cfg(unix)]
fn parse() -> Result<Option<run::Options>, String> {
    let mut o = run::Options {
        port: "/dev/ttyGS0".into(),
        keys: "/dev/ttyS0".into(),
        fb: "/dev/fb0".into(),
        backlight: "/sys/class/backlight/backlight/brightness".into(),
        verbose: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--help" | "-h" => return Ok(None),
            "--port" => o.port = value("--port")?.into(),
            "--keys" => o.keys = value("--keys")?.into(),
            "--fb" => o.fb = value("--fb")?,
            "--backlight" => o.backlight = value("--backlight")?,
            "--verbose" | "-v" => o.verbose = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Some(o))
}

#[cfg(unix)]
fn main() -> ExitCode {
    match parse() {
        Ok(None) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(o)) => match run::main(o) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("sk18-link: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("sk18-link: {e}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

#[cfg(not(unix))]
fn main() -> ExitCode {
    eprintln!("sk18-link runs on the SK-18 (Linux) only.\n{USAGE}");
    ExitCode::from(2)
}
