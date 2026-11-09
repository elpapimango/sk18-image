//! The device side of the deck protocol: what the SK-18 answers to the host, as pure logic over a [`Hardware`].
//!
//! Behaviour matched to the stock firmware (docs/stock-serial-protocol.md) and to what the host driver
//! (`deck-sk18` in controldeck-rs) expects:
//!
//!  - `getInfo` is answered with the model, the screen size and the 3 x 6 key rectangles.
//!  - `setBacklight` takes a percentage.
//!  - every image frame (`cmd 100`) is answered with an empty `cmd 100` frame carrying the same id, which is the
//!    host's cue to send the next image. It is sent even if the image could not be shown, so a bad frame never
//!    stalls the host.
//!  - key presses are sent as `keyStateChanged` events.
//!  - the stock methods this firmware does not implement (audio, typing, file management) get a polite error.

use serde_json::{json, Value};

use crate::protocol::{Frame, CMD_JSON, CMD_SHOW_JPG};

/// Model name the host driver checks for (it must start with `SK18`).
pub const MODEL: &str = "SK18";
/// Firmware version reported to the host.
pub const VERSION: &str = "open-0.1.0";
/// Screen width in pixels.
pub const WIDTH: u32 = 720;
/// Screen height in pixels.
pub const HEIGHT: u32 = 1280;
/// Key columns and rows as the firmware numbers them (portrait).
pub const COLS: u32 = 3;
/// Key rows (portrait).
pub const ROWS: u32 = 6;

/// What the device can do to the outside world.
pub trait Hardware {
    /// Sets the backlight to `percent` (0 to 100).
    fn set_backlight_percent(&mut self, percent: u8) -> Result<(), String>;
    /// Decodes `jpeg` (one picture of the whole screen) and puts it on the panel.
    fn show_jpeg(&mut self, jpeg: &[u8]) -> Result<(), String>;
}

/// The protocol state machine.
pub struct Device<H> {
    hw: H,
    shown: u64,
    failed: u64,
}

/// The `getInfo` result: model, screen and the key rectangles (158 px squares, pitch 218 px).
pub fn info() -> Value {
    let mut rects = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            let (x, y) = sk18_keys::layout::key_origin(col as usize, row as usize);
            rects.push(json!({
                "col": col, "row": row, "isKey": true,
                "x": x, "y": y, "width": sk18_keys::layout::KEY_SIZE, "height": sk18_keys::layout::KEY_SIZE
            }));
        }
    }
    json!({
        "deviceModel": MODEL, "deviceVersion": VERSION, "deviceWidth": WIDTH, "deviceHeight": HEIGHT,
        "devicePanel": { "screenWidth": WIDTH, "screenHeight": HEIGHT, "rectCols": COLS, "rectRows": ROWS, "rects": rects }
    })
}

/// A `keyStateChanged` event for the key at firmware position `(row, col)`.
pub fn key_event(row: u8, col: u8, pressed: bool) -> Frame {
    Frame::json(
        0,
        &json!({ "method": "keyStateChanged", "parameters": { "col": col, "row": row, "pressed": pressed } }),
    )
}

fn ack(id: u32, method: &str, result: Value) -> Frame {
    Frame::json(id, &json!({ "ack_method": method, "success": true, "result": result }))
}

fn nack(id: u32, method: &str, error: &str) -> Frame {
    Frame::json(id, &json!({ "ack_method": method, "success": false, "errorString": error }))
}

impl<H: Hardware> Device<H> {
    /// A device on top of `hw`.
    pub fn new(hw: H) -> Self {
        Self { hw, shown: 0, failed: 0 }
    }

    /// The hardware, for inspection in tests.
    pub fn hardware(&self) -> &H {
        &self.hw
    }

    /// Images shown, and images that could not be shown.
    pub fn counters(&self) -> (u64, u64) {
        (self.shown, self.failed)
    }

    /// Handles one frame from the host and returns the frames to send back.
    pub fn handle(&mut self, frame: &Frame) -> Vec<Frame> {
        match frame.cmd {
            CMD_SHOW_JPG => {
                match self.hw.show_jpeg(&frame.data) {
                    Ok(()) => self.shown += 1,
                    Err(e) => {
                        self.failed += 1;
                        eprintln!("sk18-link: image not shown: {e}");
                    }
                }
                vec![Frame::image_shown(frame.id)]
            }
            CMD_JSON => self.handle_json(frame),
            _ => Vec::new(),
        }
    }

    fn handle_json(&mut self, frame: &Frame) -> Vec<Frame> {
        let Ok(Value::Object(request)) = serde_json::from_slice::<Value>(&frame.data) else {
            return Vec::new(); // not a request: nothing to answer
        };
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return Vec::new(); // an acknowledgement or event from the host: nothing to answer
        };
        let params = request.get("parameters").cloned().unwrap_or(Value::Null);
        let reply = match method {
            "getInfo" => ack(frame.id, method, info()),
            "setBacklight" => match params.get("level").and_then(Value::as_i64) {
                Some(level) => {
                    let percent = level.clamp(0, 100) as u8;
                    match self.hw.set_backlight_percent(percent) {
                        Ok(()) => ack(frame.id, method, json!({})),
                        Err(e) => nack(frame.id, method, &e),
                    }
                }
                None => nack(frame.id, method, "missing integer parameter \"level\""),
            },
            other => nack(frame.id, other, "not supported by this firmware"),
        };
        vec![reply]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FrameDecoder, FRAME_OVERHEAD};

    #[derive(Default)]
    struct Fake {
        backlight: Vec<u8>,
        images: Vec<usize>,
        fail_images: bool,
        fail_backlight: bool,
    }

    impl Hardware for Fake {
        fn set_backlight_percent(&mut self, percent: u8) -> Result<(), String> {
            if self.fail_backlight {
                return Err("no backlight".into());
            }
            self.backlight.push(percent);
            Ok(())
        }
        fn show_jpeg(&mut self, jpeg: &[u8]) -> Result<(), String> {
            if self.fail_images {
                return Err("bad picture".into());
            }
            self.images.push(jpeg.len());
            Ok(())
        }
    }

    fn json_of(frame: &Frame) -> Value {
        assert_eq!(frame.cmd, CMD_JSON);
        serde_json::from_slice(&frame.data).unwrap()
    }

    fn request(id: u32, body: Value) -> Frame {
        Frame::json(id, &body)
    }

    #[test]
    fn get_info_describes_the_sk18_like_the_stock_firmware() {
        let mut d = Device::new(Fake::default());
        let out = d.handle(&request(5, json!({ "method": "getInfo" })));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, 5, "a reply carries the request's id");
        let v = json_of(&out[0]);
        assert_eq!(v["ack_method"], "getInfo");
        assert_eq!(v["success"], true);
        let r = &v["result"];
        assert_eq!(r["deviceModel"], "SK18");
        assert_eq!((r["deviceWidth"].as_u64(), r["deviceHeight"].as_u64()), (Some(720), Some(1280)));
        let panel = &r["devicePanel"];
        assert_eq!((panel["rectCols"].as_u64(), panel["rectRows"].as_u64()), (Some(3), Some(6)));
        let rects = panel["rects"].as_array().unwrap();
        assert_eq!(rects.len(), 18);
        // the first, a middle and the last key rectangle (158 px squares, pitch 218 px)
        assert_eq!(rects[0], json!({"col":0,"row":0,"isKey":true,"x":61,"y":20,"width":158,"height":158}));
        assert_eq!(rects[7], json!({"col":1,"row":2,"isKey":true,"x":279,"y":456,"width":158,"height":158}));
        assert_eq!(rects[17], json!({"col":2,"row":5,"isKey":true,"x":497,"y":1110,"width":158,"height":158}));
    }

    #[test]
    fn the_reported_rectangles_stay_inside_the_screen() {
        for r in info()["devicePanel"]["rects"].as_array().unwrap() {
            let (x, y) = (r["x"].as_u64().unwrap(), r["y"].as_u64().unwrap());
            assert!(x + 158 <= 720 && y + 158 <= 1280, "{r}");
        }
    }

    #[test]
    fn every_image_is_answered_with_an_empty_frame_with_the_same_id() {
        let mut d = Device::new(Fake::default());
        let out = d.handle(&Frame { id: 77, cmd: CMD_SHOW_JPG, data: vec![1, 2, 3] });
        assert_eq!(out, vec![Frame::image_shown(77)]);
        assert_eq!(d.hardware().images, vec![3]);
        assert_eq!(d.counters(), (1, 0));
    }

    #[test]
    fn a_picture_that_cannot_be_shown_is_still_answered_so_the_host_does_not_stall() {
        let mut d = Device::new(Fake { fail_images: true, ..Fake::default() });
        let out = d.handle(&Frame { id: 8, cmd: CMD_SHOW_JPG, data: vec![0xFF] });
        assert_eq!(out, vec![Frame::image_shown(8)]);
        assert_eq!(d.counters(), (0, 1));
    }

    #[test]
    fn the_backlight_level_is_a_clamped_percentage() {
        let mut d = Device::new(Fake::default());
        for (level, want) in [(70, 70u8), (0, 0), (100, 100), (250, 100), (-5, 0)] {
            let out = d.handle(&request(1, json!({ "method": "setBacklight", "parameters": { "level": level } })));
            assert_eq!(json_of(&out[0])["success"], true);
            assert_eq!(*d.hardware().backlight.last().unwrap(), want, "level {level}");
        }
    }

    #[test]
    fn a_backlight_request_without_a_level_or_with_a_failing_backlight_is_refused() {
        let mut d = Device::new(Fake::default());
        let out = d.handle(&request(1, json!({ "method": "setBacklight", "parameters": {} })));
        assert_eq!(json_of(&out[0])["success"], false);
        let out = d.handle(&request(2, json!({ "method": "setBacklight" })));
        assert_eq!(json_of(&out[0])["success"], false);

        let mut d = Device::new(Fake { fail_backlight: true, ..Fake::default() });
        let out = d.handle(&request(3, json!({ "method": "setBacklight", "parameters": { "level": 50 } })));
        let v = json_of(&out[0]);
        assert_eq!((v["success"].as_bool(), v["errorString"].as_str()), (Some(false), Some("no backlight")));
    }

    #[test]
    fn the_stock_methods_this_firmware_does_not_have_get_an_error_not_silence() {
        let mut d = Device::new(Fake::default());
        for m in ["setVolume", "playAudio", "stopAudio", "keyboardInput", "getFilesBySuffix", "saveToFile", "deleteFiles", "setFileCRC"] {
            let out = d.handle(&request(9, json!({ "method": m, "parameters": {} })));
            let v = json_of(&out[0]);
            assert_eq!(v["ack_method"], m);
            assert_eq!(v["success"], false);
            assert_eq!(out[0].id, 9);
        }
    }

    #[test]
    fn things_that_are_not_requests_are_ignored() {
        let mut d = Device::new(Fake::default());
        assert!(d.handle(&Frame { id: 1, cmd: CMD_JSON, data: b"not json".to_vec() }).is_empty());
        assert!(d.handle(&Frame { id: 1, cmd: CMD_JSON, data: b"[1,2]".to_vec() }).is_empty());
        // an acknowledgement from the host has no "method": nothing to answer
        assert!(d.handle(&request(1, json!({ "ack_method": "x", "success": true }))).is_empty());
        assert!(d.handle(&Frame { id: 1, cmd: 999, data: vec![] }).is_empty());
    }

    #[test]
    fn key_events_use_col_row_and_the_documented_shape() {
        let f = key_event(5, 0, true);
        assert_eq!((f.id, f.cmd), (0, CMD_JSON));
        assert_eq!(
            json_of(&f),
            json!({ "method": "keyStateChanged", "parameters": { "col": 0, "row": 5, "pressed": true } })
        );
        assert_eq!(json_of(&key_event(0, 2, false))["parameters"]["pressed"], false);
    }

    #[test]
    fn replies_survive_the_wire_and_the_decoder() {
        let mut d = Device::new(Fake::default());
        let out = d.handle(&request(11, json!({ "method": "getInfo" })));
        let wire = out[0].encode();
        assert!(wire.len() > FRAME_OVERHEAD);
        let mut dec = FrameDecoder::new();
        dec.push(&wire);
        assert_eq!(dec.next_frame().unwrap(), out[0]);
    }
}
