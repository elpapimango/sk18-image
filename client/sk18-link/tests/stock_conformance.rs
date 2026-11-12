//! Replays a session recorded from a real SK-18 with the stock firmware (V2.31, `tools/stock/stock_probe.py`, kept in
//! `tests/data/stock-v2.31-probe.jsonl`) against this firmware's protocol logic and compares every answer.
//!
//! Answers must be equal, apart from the differences listed in `DIFFERENCES`, each of which is deliberate.

use serde_json::{json, Value};
use sk18_link::device::{Device, Hardware};
use sk18_link::protocol::{Frame, CMD_JSON, CMD_SHOW_JPG};

const RECORDING: &str = include_str!("data/stock-v2.31-probe.jsonl");

/// Probe steps whose answers legitimately differ, and why.
const DIFFERENCES: &[(&str, &str)] = &[
    (
        "info",
        "version and the calibrated key rectangles are this firmware's own (D-036, D-038)",
    ),
    ("info again", "same"),
    ("getInfo after the bad frame", "same"),
    ("4 KiB of '0' before getInfo", "same"),
    (
        "files by suffix empty list",
        "the stock device lists its theme files; there are none here",
    ),
    (
        "backlight fraction",
        "the stock firmware answers 0 for 55.5 (apparently a bug); this one truncates",
    ),
    (
        "unknown method",
        "the list of available methods is each firmware's own (the text before it is compared)",
    ),
    (
        "getSettings (open firmware only)",
        "same: unknown here only without the settings backend",
    ),
];

struct Fake {
    fail_images: bool,
}

impl Hardware for Fake {
    fn set_backlight_percent(&mut self, _percent: u8) -> Result<(), String> {
        Ok(())
    }
    fn show_jpeg(&mut self, _jpeg: &[u8]) -> Result<(), String> {
        if self.fail_images {
            Err("cannot decode".into())
        } else {
            Ok(())
        }
    }
}

fn json_of(frame: &Frame) -> Value {
    assert_eq!(frame.cmd, CMD_JSON, "{frame:?}");
    serde_json::from_slice(&frame.data).unwrap()
}

/// Without what each firmware reports about itself.
fn without_identity(mut info: Value) -> Value {
    info["result"]["deviceVersion"] = json!("-");
    for rect in info["result"]["devicePanel"]["rects"].as_array_mut().unwrap() {
        rect["x"] = json!(0);
        rect["y"] = json!(0);
    }
    info
}

#[test]
fn this_firmware_answers_like_the_recorded_stock_firmware() {
    let mut checked = 0;
    let mut device = Device::new(Fake { fail_images: false });
    let mut failing = Device::new(Fake { fail_images: true });
    for line in RECORDING.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        let Some(label) = entry["label"].as_str() else {
            continue;
        };
        let stock: Vec<&Value> = entry["rx"].as_array().unwrap().iter().collect();
        let skip_exact = DIFFERENCES.iter().any(|(l, _)| *l == label);

        // What the probe sent: a request from `tx`, or the picture frame of the last step.
        let (request, picture) = if label == "empty JPEG (SOI+EOI)" {
            let id = stock[0]["json"]["requestId"].as_u64().unwrap() as u32;
            (
                Frame {
                    id,
                    cmd: CMD_SHOW_JPG,
                    data: vec![0xFF, 0xD8, 0xFF, 0xD9],
                },
                true,
            )
        } else if let Some(tx) = entry.get("tx") {
            let mut body = json!({ "method": tx["method"] });
            if !tx["parameters"].is_null() {
                body["parameters"] = tx["parameters"].clone();
            }
            (Frame::json(tx["id"].as_u64().unwrap() as u32, &body), false)
        } else {
            continue; // bad CRC, noise before a frame: the framing layer's business (protocol tests)
        };
        let ours = if picture {
            failing.handle(&request)
        } else {
            device.handle(&request)
        };

        assert_eq!(ours.len(), stock.len(), "{label}: number of answers");
        for (ours, stock) in ours.iter().zip(&stock) {
            assert_eq!(ours.id as u64, stock["id"].as_u64().unwrap(), "{label}: id");
            assert_eq!(ours.cmd as u64, stock["cmd"].as_u64().unwrap(), "{label}: cmd");
            let (ours, stock) = (json_of(ours), stock["json"].clone());
            match label {
                "info" | "info again" | "getInfo after the bad frame" | "4 KiB of '0' before getInfo" => {
                    assert_eq!(without_identity(ours), without_identity(stock), "{label}");
                }
                "unknown method" | "getSettings (open firmware only)" => {
                    let head = |v: &Value| {
                        v["errorString"]
                            .as_str()
                            .unwrap()
                            .split(" Available")
                            .next()
                            .unwrap()
                            .to_owned()
                    };
                    assert_eq!(head(&ours), head(&stock), "{label}");
                    assert_eq!(ours["success"], stock["success"], "{label}");
                }
                _ if skip_exact => {}
                _ => assert_eq!(ours, stock, "{label}"),
            }
        }
        checked += 1;
    }
    // every recorded step was replayed (the recording has 30 that carry a request or a picture)
    assert!(checked >= 28, "only {checked} steps were replayed");
}
