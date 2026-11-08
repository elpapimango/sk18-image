//! `sk18-keys`: prints the key events of the SK-18, live from the serial port or from a captured file.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use sk18_keys::{Decoder, Frame, KeyEvent, Orientation};

const USAGE: &str = "\
usage: sk18-keys [--port PATH] [--orientation 0|90|180|270] [--file PATH] [--help]

  --port PATH         serial port of the key controller (default /dev/ttyS0, which is UART1 on the mainline image)
  --orientation DEG   how the device is turned, clockwise from portrait (default 90, how the SK-18 is used)
  --file PATH         decode a capture instead of reading the port; a .hex file is text with one hex pair per
                      byte ('#' starts a comment), anything else is raw bytes

Prints one line per event:  press|release  fw(row,col)  key(row,col)  code 0xNN";

struct Options {
    port: PathBuf,
    orientation: Orientation,
    file: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Options>, String> {
    let mut o = Options { port: PathBuf::from("/dev/ttyS0"), orientation: Orientation::default(), file: None };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--help" | "-h" => return Ok(None),
            "--port" => o.port = PathBuf::from(value("--port")?),
            "--file" => o.file = Some(PathBuf::from(value("--file")?)),
            "--orientation" => {
                let v = value("--orientation")?;
                o.orientation = Orientation::parse(&v).ok_or_else(|| format!("unknown orientation {v:?}"))?;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Some(o))
}

/// Text hex (`aa 55 ...`, `#` comments) to bytes.
fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        for tok in line.split_whitespace() {
            out.push(u8::from_str_radix(tok, 16).map_err(|e| format!("bad hex byte {tok:?}: {e}"))?);
        }
    }
    Ok(out)
}

fn print_frame(out: &mut impl Write, orientation: Orientation, frame: Frame) -> io::Result<()> {
    match frame {
        Frame::Key(KeyEvent { row, col, pressed, code }) => {
            let (lr, lc) = orientation.to_logical(row, col).unwrap_or((u8::MAX, u8::MAX));
            writeln!(
                out,
                "{:<7} fw({row},{col})  key({lr},{lc})  code 0x{code:02x}",
                if pressed { "press" } else { "release" }
            )?;
        }
        Frame::Unknown(p) => writeln!(out, "unknown frame, payload {}", hex(&p))?,
    }
    out.flush()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// Where the decoded events go; remembers the first write error so the read loop can stop.
struct Sink<W: Write> {
    out: W,
    orientation: Orientation,
    error: Option<io::Error>,
}

impl<W: Write> Sink<W> {
    fn emit(&mut self, frame: Frame) {
        if self.error.is_none() {
            if let Err(e) = print_frame(&mut self.out, self.orientation, frame) {
                self.error = Some(e);
            }
        }
    }
}

fn run(o: &Options) -> Result<(), String> {
    let mut dec = Decoder::new();
    let mut sink = Sink { out: io::stdout().lock(), orientation: o.orientation, error: None };

    if let Some(path) = &o.file {
        let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let bytes = if path.extension().is_some_and(|e| e == "hex") {
            parse_hex(&String::from_utf8_lossy(&raw))?
        } else {
            raw
        };
        dec.push(&bytes, |f| sink.emit(f));
        let s = dec.stats();
        eprintln!("{} bytes, {} frames ({} unknown), {} bytes skipped", bytes.len(), s.frames, s.unknown_frames, s.skipped_bytes);
    } else {
        #[cfg(unix)]
        {
            use std::io::Read;
            let mut port = sk18_keys::port::open_port(&o.port).map_err(|e| format!("{}: {e}", o.port.display()))?;
            eprintln!("reading {} at {} baud, orientation {:?}", o.port.display(), sk18_keys::port::BAUD, o.orientation);
            let mut buf = [0u8; 256];
            loop {
                let n = port.read(&mut buf).map_err(|e| format!("{}: {e}", o.port.display()))?;
                if n == 0 {
                    break;
                }
                dec.push(&buf[..n], |f| sink.emit(f));
                if sink.error.is_some() {
                    break;
                }
            }
        }
        #[cfg(not(unix))]
        return Err("reading a serial port is only supported on Unix; use --file".into());
    }
    match sink.error {
        Some(e) if e.kind() != io::ErrorKind::BrokenPipe => Err(format!("stdout: {e}")),
        _ => Ok(()),
    }
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
                eprintln!("sk18-keys: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("sk18-keys: {e}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
