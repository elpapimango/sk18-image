# Hardware status of the mainline image

Build `9fdda5c6…7e0f9` (`sdcard.img`, 2026-11-07): Buildroot `60a3423e95`, Linux 7.3.1, U-Boot 2026.10.
"Built" means it compiles and the image contents were checked. "Works" means observed on the board on 2026-11-07 (D-013): it boots from the test SD card and runs a shell on the USB serial gadget.

| Hardware block | Original system | Mainline support | Required driver / DT node | Firmware | Test result | Remaining issue |
|---|---|---|---|---|---|---|
| DRAM init (SPL) | vendor boot0, DDR3 792 MHz | U-Boot 2026.10 `mangopi_mq_r` has the same parameters | `CONFIG_DRAM_*` | none | **works**: SPL ran, 128 MiB detected (131072K, 114 MB usable after CMA), stable for 3+ minutes | TPR1-3/6 left at defaults; run a memory stress test |
| UART3 console | PB6/PB7, 115200 | yes (8250 DW) | `&uart3` | none | not tested | header location on the board unknown |
| SD card | mmc0, 4-bit, CD = PF6 low | yes | `&mmc0` | none | **works**: 3.75 GiB SDHC detected at high speed, root mounted read-write, card-detect GPIO found | supply rail assumed 3.3 V |
| USB device port (OTG) | CDC-ACM gadget to the host | yes (musb) | `&usb_otg`, `&usbphy` | none | **works**: `g_serial` enumerates on the host as `0525:a4a7` ("Linux 7.3.1 with musb-hdrc"), login prompt on `ttyGS0` | stock `1d6b:0104` port is gone while this image runs |
| USB host port | ehci1/ohci1, VBUS on PG12 (regulator in the DTS) | yes | `&ehci1`, `&ohci1`, `reg_usb1_vbus` | none | **works**: the Wi-Fi dongle enumerates (`0bda:1a2b`, CD-ROM mode) | mode switch and driver for Wi-Fi still to do; stock shows about 97,000 host interrupts in 4 min (unexplained) |
| Wi-Fi RTL8188GU (`0bda:b711` after switch) | rtlwifi module | `rtl8xxxu` binds on the DietPi (Linux 6.18) | `CONFIG_RTL8XXXU` | `rtlwifi/rtl8710bufw_SMIC.bin` | not in this image yet | mode switch (`0bda:1a2b` CD-ROM) needs a udev rule |
| Display 720x1280 | lcd0, MIPI-DSI video mode, 2 lanes, 70 MHz, HX8394-family panel needing an init sequence | full pipeline in 7.3.1 + HX8394 panel entry (patch 0001) | `de`, `mixer0`, `tcon_top`, `tcon_lcd0`, `dsi`, `dphy`, panel, PE5 regulator | none | **works**: console text on the panel, right way up | see `docs/phase5-display.md` and D-020 |
| Backlight | PWM channel 3 on PG10, 1 kHz | no driver for the T113 PWM block in 7.3.1; `pwm-gpio` (software) exists | `pwm-gpio`, `pwm-backlight` | none | not in this image | interim software PWM; real driver later |
| Key/touch input | not a touchscreen: a separate controller reports the 18 keys over UART1 (PG6/PG7, 115200), frames `AA 55 CS 06 F9 16 S R C 00 K F5 5F` | none needed (userspace reads `/dev/ttyS0`) | `&uart1` | none | **works**: 136 of 136 frames decoded, all 18 keys | `docs/uart1-key-protocol.md`; SoC-to-controller commands unknown |
| Key image path (18 keys on screen) | stock firmware, serial protocol | n/a | userspace client | none | works on stock firmware (deck-sk18 driver) | needs display and touch first |
| Audio | codec, DMIC | partial | `codec` | none | not in this image | |
| ATSHA204A secure element | `/dev/atsha204a` | n/a | not used | none | not used | deliberately ignored |
