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

## Still to verify on the device

Reply to a `SHOW_JPG` frame, the format and timing of `keyStateChanged`, `setBacklight` range, and
whether touches or physical keys produce the key events.
