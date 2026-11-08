# SK-18 key/touch events on UART1 (SoC side)

Status labels: **Confirmed** (verified against captured data), **Inferred**, **Unknown**.

## Summary

The SK-18's "touch" is **not** a touchscreen device. A separate controller (the same one that shows up on the host as the
USB HID keyboard `4250:426f` "SYK syk_keyboards") reports the 18 on-screen keys to the T113 over **UART1** as press/release
events, as discrete grid cells, not coordinates. Nothing on the T113 side reads a touch panel: the stock kernel has no touchscreen
input device, the SoC touch ADC is never programmed, and the Goodix node in the stock DTB is disabled (decision log D-021, D-022).

## Link (Confirmed)

| Item | Value | Source |
|---|---|---|
| UART | UART1, base `0x02500400`, pins PG6 (TX), PG7 (RX), PG8 (RTS), PG9 (CTS) | stock pinmux (`uart1` on PG6..PG9) |
| Stock device node | `/dev/ttyS1` (opened by `KeyboardDevice` through `QSerialPort`) | strings of the stock app |
| Mainline device node | **`/dev/ttyS0`**: UART0 is disabled, so UART1 is the first 8250 port; UART3 (console) is `ttyS3` through its alias | boot log of the mainline image |
| Baud rate | **115200**, 8 data bits; 8N1 assumed (framing errors would show; there were none) | captures at 115200 / 460800 / 921600 / 9600: only 115200 is clean |
| Direction | controller to SoC. Nothing was sent by the SoC in these captures | capture |

DTS: `&uart1` with `uart1_pg6_pins` and `uart1_pg8_rts_cts_pins` (the pin groups of the shared dtsi).

## Frame (Confirmed, 136 of 136 captured frames valid)

13 bytes, no length variation seen:

```
offset  0   1   2   3   4   5   6   7   8   9   10  11  12
        AA  55  CS  06  F9  16  S   R   C   00  K   F5  5F
```

| Field | Meaning |
|---|---|
| `AA 55` | start of frame |
| `CS` | checksum: the 8-bit sum of bytes 5..10 (type, S, R, C, 0, K). Holds for all frames |
| `06 F9` | payload length (6) and its bitwise complement |
| `16` | message type: key state. The only type seen |
| `S` | 1 = pressed, 0 = released |
| `R` | key row, 0..5 |
| `C` | key column, 0..2 |
| `00` | always 0 |
| `K` | key code, one fixed value per key (table below); they are USB HID keyboard usage codes |
| `F5 5F` | end of frame |

The grid is 3 columns x 6 rows = 18 keys, the same numbering as the firmware's serial protocol over USB (`keyStateChanged`, `docs/stock-serial-protocol.md`).

### Key codes (Confirmed)

| | col 0 | col 1 | col 2 |
|---|---|---|---|
| row 0 | `27` (0) | `1e` (1) | `1f` (2) |
| row 1 | `20` (3) | `21` (4) | `22` (5) |
| row 2 | `04` (a) | `05` (b) | `06` (c) |
| row 3 | `07` (d) | `08` (e) | `09` (f) |
| row 4 | `0b` (h) | `0c` (i) | `0d` (j) |
| row 5 | `0e` (k) | `0f` (l) | `10` (m) |

The letters in brackets are the HID keyboard usages (note `g`, `0a`, is skipped). That the controller also acts as the USB keyboard is **Inferred**
from the match with `4250:426f`; the codes are what that HID interface would send.

## Evidence

- Capture: 1768 bytes at 115200 while a person pressed the keys, taken on the mainline image `dbb79615…` with `cat /dev/ttyS0` after `stty 115200 raw`. Decoded with `work/` scripts (not committed): 136 valid frames, 0 bytes skipped, all 18 keys seen, press and release alternate perfectly for every key.
- Stock system, 4 minutes with the screen touched: `uart1` 843 interrupts; no touch-ADC activity; no touchscreen input device.

## Unknown

- Whether the SoC can send commands to the controller (key LEDs, brightness, mode). The stock app opens the port read/write (`QSerialPort`), but no SoC-to-controller traffic was captured.
- Whether there are other message types (long presses, errors). Only type `0x16` appeared in about 4 minutes of use.
- The mapping from (row, column) to what is drawn on the screen in each orientation (the screen is rotated in use; see D-009).
- Why the stock USB host interrupt count is so high (about 97,000 in 4 minutes).

## Consequence for the product

No touchscreen driver is needed in the kernel. The on-device client reads `/dev/ttyS0` at 115200, decodes these frames, and maps (row, column) to a key of the deck. It can feed them to OpenDeck or `controldeck-rs` directly, or expose them as an evdev device through `uinput` if other software needs it.
