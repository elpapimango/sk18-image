# Hardware status of the mainline image

Build `9fdda5c6…7e0f9` (`sdcard.img`, 2026-11-07): Buildroot `60a3423e95`, Linux 7.3.1, U-Boot 2026.10.
"Built" means it compiles and the image contents were checked; nothing here has been tested on the board yet.

| Hardware block | Original system | Mainline support | Required driver / DT node | Firmware | Test result | Remaining issue |
|---|---|---|---|---|---|---|
| DRAM init (SPL) | vendor boot0, DDR3 792 MHz | U-Boot 2026.10 `mangopi_mq_r` has the same parameters | `CONFIG_DRAM_*` | none | not tested | TPR1-3/6 left at defaults; verify memory stability |
| UART3 console | PB6/PB7, 115200 | yes (8250 DW) | `&uart3` | none | not tested | header location on the board unknown |
| SD card | mmc0, 4-bit, CD = PF6 low | yes | `&mmc0` | none | not tested | supply rail assumed 3.3 V |
| USB device port (OTG) | CDC-ACM gadget to the host | yes (musb) | `&usb_otg`, `&usbphy` | none | not tested | built-in `g_serial` for the first test |
| USB host port | ehci1/ohci1 | yes | `&ehci1`, `&ohci1` | none | not tested | VBUS control GPIO unknown (always on assumed) |
| Wi-Fi RTL8188GU (`0bda:b711` after switch) | rtlwifi module | `rtl8xxxu` binds on the DietPi (Linux 6.18) | `CONFIG_RTL8XXXU` | `rtlwifi/rtl8710bufw_SMIC.bin` | not in this image yet | mode switch (`0bda:1a2b` CD-ROM) needs a udev rule |
| Display 720x1280 | lcd0, interface type 4 (probably DSI, 2 lanes) | DE2 mixer and LCD TCON for D1 exist; no D1 compatible found in the MIPI-DSI driver | `de`, `tcon_lcd0`, `dsi`, panel | none | not in this image | main risk: panel interface and init sequence |
| Backlight | PWM channel 3, 1 kHz | PWM driver exists | `pwm`, `pwm-backlight` | none | not in this image | enable GPIO unknown |
| Touch | Goodix `gt9xxnew_ts`, I2C 2, 0x14 | `goodix` driver | `&i2c2`, node | none | not in this image | IRQ and reset pins to decode from the original DTB |
| Key image path (18 keys on screen) | stock firmware, serial protocol | n/a | userspace client | none | works on stock firmware (deck-sk18 driver) | needs display and touch first |
| Audio | codec, DMIC | partial | `codec` | none | not in this image | |
| ATSHA204A secure element | `/dev/atsha204a` | n/a | not used | none | not used | deliberately ignored |
