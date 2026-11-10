# The deck protocol over USB HID

Decision D-041. The open-source firmware can carry the deck protocol (`stock-serial-protocol.md`) on a HID function of the
USB gadget instead of a CDC-ACM serial port. The protocol above the transport is unchanged: the same frames (`A1 A5 5A 5E`
...), the same commands, the same CRCs. Only the byte stream underneath is different.

## Gadget

One device, `1d6b:0104`, manufacturer `SK18 open firmware`, product `SK-18 Deck`, no serial number (so the controldeck
device id stays `waveshare-sk18`), one configuration with two functions (`S01usb-gadget`):

| function | device | host | use |
| --- | --- | --- | --- |
| `hid.usb0` (interface 0) | `/dev/hidg0` | `/dev/hidrawN` | the deck protocol |
| `acm.usb0` | `/dev/ttyGS0` | `/dev/ttyACMn` | login console (getty) |

HID function: protocol 0, subclass 0 (no boot device), report length 512, endpoint interval 1 (every microframe; the
default of 4 would allow only 512 bytes per millisecond). The report descriptor is one vendor-defined application
collection (usage page `0xFF00`, usage 1) with a 512-byte input report and a 512-byte output report and **no report
ids**:

```
06 00 FF  09 01  A1 01  15 00  26 FF 00  75 08
96 00 02  09 01  81 02          (input, 512 x 8 bit)
96 00 02  09 02  91 02          (output, 512 x 8 bit)
C0
```

512 is the size of the musb FIFOs of the T113, so a report is one packet.

## Reports

Both directions use fixed 512-byte reports:

```
offset 0   u16 LE   n, the number of stream bytes in this report (0..510)
offset 2   n bytes  the next bytes of the protocol stream
...        zero padding up to 512 bytes
```

A byte stream longer than 510 bytes is cut into consecutive reports; the receiver appends the payloads. A report with a
length above 510 is dropped, and the frame decoder above resynchronises on the next start marker. A frame may start and
end anywhere inside a report; reports are not frames.

* **Device**: one `write(2)` to `/dev/hidg0` is one input report (always 512 bytes); one `read(2)` returns one output
  report. `sk18-link` selects this transport when its `--port` is a `hidg*` node (the default is `/dev/hidg0`) and
  the serial transport for anything else.
* **Host**: `hidraw` wants a leading report id byte for a write, `0x00` for a device without numbered reports, which the
  kernel strips; a read returns the 512 bytes without it. `deck-sk18` writes 513 bytes (`00` + report) and reads 512.

## Why

* The tty layer is gone: no line discipline, no lost wakeups (D-037), no termios. A report is delivered whole or not at all.
* The transport looks like the Stream Deck's, which is what the `streamdeck-xl-mimic` plan builds on.
* The kernel's `f_hid` receive queue is raised from 4 to 1024 requests (patch 0005), for the same reason as the
  `u_serial` patch 0004: the musb controller loses packets when the queue runs dry in the middle of a 58 KB burst.

## Host discovery

`deck-sk18` lists the `hidraw` nodes next to the serial ports and keeps one candidate per USB device. If a device has a
`hidraw` node it wins over the device's serial ports (the serial port of the HID firmware is only a console, and the stock
firmware has no HID function). The candidate's transport kind is `hid` instead of `serial`. The udev rule
`70-control-deck-sk18.rules` opens that `hidraw` node (manufacturer `SK18 open firmware`) to the `control-deck` group.
