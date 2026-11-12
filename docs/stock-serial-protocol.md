# Stock firmware serial protocol (observed)

Independent description of what the stock SK-18 firmware (V2.31) speaks over its USB CDC-ACM port,
written from the vendor's published open-source host demo (`OpenSourceLicenseDemo_V1.0`, shipped
in the vendor zip) and verified against a real device. Used for the interim `deck-sk18` driver and as
a reference only; the final on-device client uses a different protocol (see D-006).

Status: **Confirmed** = reproduced on the device; **From demo** = taken from the vendor demo, not
yet tried.

## Transport

- USB CDC-ACM `1d6b:0104` ("Serial", Allwinner Technology Inc.), host side `/dev/ttyACM0`, 115200 8N1,
  no flow control. The device sends nothing until asked. (Confirmed)
- The vendor demo writes 1 MiB of `'0'` bytes before its first request (apparently to flush the
  device's parser). A bare `getInfo` frame without that prefix was answered normally. (Confirmed)

## Frame (little endian) (Confirmed)

| Field | Size | Notes |
|---|---|---|
| header | 4 | `A1 A5 5A 5E` |
| id | 4 | sender-chosen; a reply to a JSON request carries the request's id |
| cmd | 4 | `100` SHOW_JPG, `101` JSON, `102` end marker (demo enum) |
| size | 4 | length of data |
| size_crc | 4 | CRC-32 of the 4 `size` bytes |
| data | size | |
| data_crc | 4 | CRC-32 of data |

CRC-32 is the standard IEEE CRC-32 (init `0xFFFFFFFF`, reflected, final xor `0xFFFFFFFF`), identical
to zlib's `crc32`. The receiver resynchronizes on the header and drops frames whose size CRC or data
CRC does not match.

## JSON messages (cmd 101)

Request: `{"method":"<name>","parameters":{...}}` (`parameters` is optional).
Reply: `{"ack_method":"<name>","success":true,"result":{...}}`, or `success:false` with
`errorString`. The device also sends events `{"method":"keyStateChanged","parameters":{...}}`.

| Method | Parameters | Status |
|---|---|---|
| `getInfo` | none | Confirmed (see below) |
| `setBacklight` | `level` (int) | From demo |
| `setVolume` | `level` (int) | From demo |
| `playAudio` / `stopAudio` | `filePath` / none | From demo |
| `keyboardInput` | `inputString` (the device types it as HID keys) | From demo |
| `getFilesBySuffix`, `saveToFile`, `setFileCRC`, `deleteFiles` | file management on the device | From demo; **not used** by this project (they modify the device's storage) |
| event `keyStateChanged` | `col`, `row`, `pressed` | From demo |

## `getInfo` result (Confirmed on the device)

```json
{"deviceModel":"SK18","deviceVersion":"V2.31","deviceWidth":720,"deviceHeight":1280,
 "devicePanel":{"screenWidth":720,"screenHeight":1280,"rectCols":3,"rectRows":6,
   "rects":[{"col":0,"row":0,"x":63,"y":10,"width":158,"height":158,"isKey":true}, ...]}}
```

18 keys of 158x158 px, x = 63, 281, 499 (pitch 218) and y = 10 + 218 * row, on a 720x1280 screen.
The rects with `isKey` give the layout; the demo also draws non-key rects if present (none here).

## Display (cmd 100, From demo)

The host sends one JPEG of the whole `deviceWidth x deviceHeight` screen as the frame's data. The
device answers each frame with a cmd 100 frame (the demo treats this as "frame finished" and sends
the next one), so the host paints at the device's pace.

## Verified with the `deck-sk18` driver on a real SK-18 (V2.31, 2026-11-07)

- A full-screen JPEG (720x1280, quality 85) is shown after the host sends it as a `SHOW_JPG` frame;
  18 key colours painted correctly into the rectangles from `getInfo`.
- The driver sent the next frame after the device's `SHOW_JPG` echo and never needed its 1.5 s
  fallback timeout during the test.
- `keyStateChanged` (`col`, `row`, `pressed`) is sent for **touches on the on-screen keys**: 48 events
  for a press and a release on each key, in order, no duplicates, positions matching the touched keys.
- `setBacklight` with `level` 70 was accepted without an error reply (the visible brightness change
  was not measured).
- Repainting a key white while pressed and back on release kept up with finger speed.

## Replies recorded from a real SK-18 with stock V2.31 (2026-11-12, `tools/stock/stock_probe.py`)

The session is kept in `docs/captures/stock-v2.31-probe.jsonl` (and as the fixture of `client/sk18-link/tests/stock_conformance.rs`);
the USB descriptors in `stock-v2.31-lsusb.txt`. Every reply is a cmd 101 frame with the request's id.

| Request | Stock reply |
|---|---|
| `setBacklight` `{"level": 0..100}` | `{"ack_method":"setBacklight","success":true,"result":<level>}` (the level is echoed; 55.5 gave 0) |
| `setBacklight` without `level` | error `Missing 'level' parameter` |
| `setBacklight` with a string, null or bool | error `'level' must be a number` |
| `setBacklight` 300 or -1 | error `Backlight level must be between 0 and 100, got 300` |
| `setVolume` 0..7 | success, `result` the level; 50 or -1: `Volume level must be between 0 and 7, got 50` |
| `stopAudio` | success, no `result` |
| `playAudio` | `Missing 'filePath' parameter`, or `File does not exist: <path>` |
| `getFilesBySuffix` | `Missing 'suffixs' parameter`; with a list: `{"filePaths": [...]}` (an empty suffix list returned the device's theme files, `/data/theme/SK18/*.Theme` with a `crc`) |
| `keyboardInput` without `inputString` | `Missing 'inputString' parameter` |
| unknown method | error `Unknown method: X. Available methods: [deleteFiles, getFilesBySuffix, getInfo, keyboardInput, playAudio, saveToFile, setBacklight, setFileCRC, setVolume, stopAudio]` |
| `parameters` that is not an object | `{"success":false,"errorString":"Field 'parameters' must be an object"}` (no `ack_method`) |
| a picture that cannot be decoded | **no echo**; a JSON frame with id `0xFFFFFFFF`: `{"errorString":"Image decoding failed, the image size may be larger than 720x1280","requestId":<id>}` |
| a frame with a bad data CRC | no reply; the next good frame is answered normally |
| 4 KiB of `0` bytes before a frame | the frame is answered (the parser resynchronises) |

`getInfo` is as above (V2.31, key rectangles at x = 63 + 218 col, y = 10 + 218 row). **Touch events** (`docs/captures/stock-v2.31-touch.jsonl`, 182 events, every one of the 18 keys touched): `{"method":"keyStateChanged","parameters":{"col":c,"pressed":bool,"row":r}}`, cmd 101, strictly alternating press and release (a press about 170 ms before its release when tapped), rows 0 to 5 and columns 0 to 2. **Their frame ids count down by one from `0xFFFFFFFF`** (they are device-originated; the picture error frame takes the next number of the same counter), so they never collide with the host's ids. `sk18-link` does the same now and is tested against this recording.

Not tried, because they change the device:
`saveToFile`, `setFileCRC`, `deleteFiles`, a real `playAudio` and `keyboardInput` with text.

**USB behaviour of the stock card:** `1d6b:0104`, `bcdDevice 5.04`, strings `Allwinner Technology Inc.` / `Serial`, no serial number,
CDC ACM (interface association, control + data), self-powered, 500 mA. In three of four sessions the device disconnected and
re-enumerated once within seconds of the first requests after it had been plugged in (a later 60-second session was stable);
the Stream Deck XL on the same hub dropped once with it (hub reset), so a power or hub effect is possible and was not
investigated.

## Still to verify

Whether the device has physical buttons that produce events, the valid range and effect of
`setBacklight`, behaviour when the host stops sending frames (does the stock UI come back, and
when?), and how long a frame takes to reach the screen (latency).

## Device settings (open-source firmware only, `sk18-link`; not in the stock firmware)

Extra JSON methods so that the host can set up the device over the USB link. Requests are `{"method": ..., "parameters": ...}`; the answer is
the usual `{"ack_method": ..., "success": ..., "result": ... | "errorString": ...}`. The stock firmware answers them with an error (or not at all),
which is how a host tells the two firmwares apart.

| Method | Parameters | Result |
|---|---|---|
| `getSettings` | none | `{"wifi": {...}, "home_assistant": {...}}` |
| `setWifi` | `ssid`, `passphrase` (optional for the same network), `country`, `hidden` | like `getSettings` |
| `clearWifi` | none | like `getSettings` |
| `setHomeAssistant` | `enabled`, `url`, `event`, `token` (optional), `clear_token` | like `getSettings` |
| `testHomeAssistant` | `url`, `token` (both optional; the stored values fill in) | `{"message": "API running. (12 ms)"}` or an error text |

`wifi`: `{"configured", "ssid", "country", "hidden", "state"}` and, once connected, `"ip"` and `"signal_dbm"`; `state` is `connected`, `getting-address`,
`connecting`, `disconnected`, `not-running`, `no-adapter` or `not-configured`. `home_assistant`: `{"enabled", "url", "event", "token_set"}`. The Wi-Fi key
and the token are never reported. `testHomeAssistant` blocks the link for up to about 8 seconds (connection and read timeouts of 4 s each), so a host
should allow 10 s for the answer.
