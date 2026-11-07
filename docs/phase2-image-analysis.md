# Phase 2: original SD-card image analysis (Waveshare SK-18, T113-S3)

Status labels: **Confirmed** (read from the image), **Inferred** (reasoned, not verified on hardware), **Unknown**.

## Source and handling

| Item | Value |
|---|---|
| Image | `C:\Temp\sk18\image\waveshare_sk18.img` (WSL: `/mnt/c/Temp/sk18/image/`) |
| Size | 4,026,531,840 bytes (3.75 GiB), raw whole-disk image |
| SHA-256 | `cfcc04488158e411c0bdaedacb302f881b444a6af25630c113f80f3f63faf4e7` |
| Vendor zip | `MK Series Upper Computer Open Source_V1.0.zip`, 758,887,616 bytes |
| Zip SHA-256 | `1e9e43a58620b836cf532c31f5f3ab87c5a06490e1f9186b6544a6d6b747a29a` |
| Working copies | WSL `~/sk18-work/parts/*.bin`, plus `dtb/` and `app/` (not committed, they derive from proprietary firmware) |

The original image is only ever read. Partitions were copied with `dd bs=512 skip=<start> count=<sectors>` and checked against the source by re-hashing the same sector ranges (all seven files verified identical to the source ranges).

## Partition table (Confirmed)

GPT with protective MBR, 512-byte sectors, first usable LBA 41464.

| # | Name | Start LBA | Sectors | Size | Content |
|---|---|---|---|---|---|
| - | (raw area) | 0 | 41464 | 20.2 MiB | MBR/GPT, boot0 @ 8 KiB, U-Boot package @ 16,793,600 B |
| 1 | boot-resource | 41464 | 65536 | 32 MiB | FAT16 "Volumn" |
| 2 | env | 107000 | 504 | 252 KiB | U-Boot environment |
| 3 | env-redund | 107504 | 504 | 252 KiB | identical copy of env |
| 4 | boot | 108008 | 12600 | 6.2 MiB | Android boot image, kernel only |
| 5 | rootfs | 120608 | 2097152 | 1 GiB | ext4 (filesystem about 800 MiB) |
| 6 | UDISK | 2217760 | 5644512 | 2.7 GiB | not yet analysed |

## Boot chain (Confirmed)

1. BROM loads vendor **boot0** (`eGON.BT0`, header string `4.0`) from 8 KiB. It prints `HELLO! BOOT0 is starting!`, initializes DRAM (DDR2/DDR3 auto-scan) and reads the next stage from SD.
2. The next stage is a vendor **`sunxi-package`** at byte 16,793,600 holding the vendor U-Boot, OP-TEE (`linaro,optee-tz` in the DTB) and two DTBs: a generic `sun8iw20` one and a board DTB with `model = "SK18"`.
3. U-Boot reads the environment from the `env` partition and runs `bootcmd=run setargs_mmc boot_normal`:
   - `boot_normal=sunxi_flash read 41000000 ${boot_partition};bootm 41000000`
   - `setargs_mmc`: `earlycon=uart8250,mmio32,0x05000000 clk_ignore_unused loglevel=8 root=/dev/mmcblk0p5 init=/sbin/init cma=0M gpt=1`, with MAC variables left empty
   - `bootdelay=0`
4. The Android boot image (board name `t113-pro`) carries a gzip zImage (kernel at `0x40008000`) and a 12-byte ramdisk. Kernel: **Linux 5.4.61**, built by `arm-openwrt-linux-gnueabi-gcc` (Linaro 6.4.1), build #231, 2025-10-17.

**Kernel DTB (Confirmed):** the `boot` image has no DTB (header `dt_size`/second size are 0, and a scan for valid FDT headers in the image and in the decompressed kernel finds none). The `sunxi-package` lists items `u-boot`, `optee` and `dtb`. The `dtb` item is the 53,881-byte `SK18` board DTB at byte 18,120,704, which U-Boot hands to the kernel. The 17,174-byte `sun8iw20` DTB at byte 17,823,248 is U-Boot's own control tree (9 enabled nodes versus 47 in the board DTB). So the board DTB is the single source of truth for hardware facts.

**Unknown:** the exact kernel command line seen by Linux after U-Boot rewrites `bootargs` (the board DTB `chosen` node says `console=ttyS0`, the U-Boot environment overrides it).

## Root filesystem (Confirmed)

OpenWrt/Tina userland with procd, busybox ash, wpa_supplicant, dnsmasq, adbd, dbus, ntpd. Top-level directories include `arm-qt`, `arm-tslib`, `data`, `base`, `spec`, `overlay`, `rom`.

- `/etc/inittab` runs a login shell on `ttyS3` (and `tts/0`, `tty1`).
- Wi-Fi uses `wlan0` with `wpa_supplicant -Dnl80211`; kernel modules in `/lib/modules/5.4.61`, with an `rtlwifi` directory under `/lib/firmware`.
- `qt_app1` waits for `/mnt/SDCARD` (the FAT `boot-resource` partition), copies `overlayDir/*` over `/`, and may copy and run `lunch.sh` from the card. This is a vendor update/hook mechanism.
- `qt_app2` (procd) runs `/data/appLunch.sh`, which sources `/data/qt_env.sh` and runs `/data/KeyboardDevice`, logging to `/mnt/SDCARD/log.txt`.
- `/data` (inside the rootfs, not UDISK): `KeyboardDevice`, `xiaozhiAIControl`, `xiaozhiAISound`, `usb_modeswitch`, `setusbconfig`, `serial_uart.sh`, `serial_adb.sh`, `serial_remove.sh`, fonts, images, audio, video, theme, plus raw framebuffers `128x128.bin`, `240x296.bin` and `720x1280.bin`.
- `/data/serial_uart.sh` configures a USB CDC-ACM gadget through configfs (`1d6b:0104`, product "Virtual Serial Port", manufacturer "SYK").

## Original application (Confirmed, static only, not executed)

`/data/KeyboardDevice`: ELF 32-bit ARM EABI5, hard-float (`ld-linux-armhf.so.3`), dynamically linked, stripped. It needs Qt5 (Widgets, Gui, Core, Xml, Charts, SerialPort), `libasound`, and Allwinner multimedia libraries (`libvdecoder`, `libVE`, `libMemAdapter`, `libcdc_base`, `libuapi`, `libvideoplayerinterface`). Display: Qt `linuxfb` on `/dev/fb0`; touch through tslib on `/dev/input/event1`.

**Inferred:** the stock device acts as a USB serial peripheral driven by the PC "upper computer" application (framed protocol in the vendor zip's `OpenSourceLicenseDemo`). It is not an OpenDeck network client, so Wi-Fi and OpenDeck are new functionality (see decision D-004).

## Extra findings (Confirmed from strings and scripts)

- `boot-resource/config.json` holds device settings: theme `/data/theme/SK18/defaultTheme.Theme`, backlight 71, name "ScreenKey", volume 7, `enableXIAOZHIAI`, `isCanvasFlip`.
- `KeyboardDevice` references `/dev/ttyGS0` (USB gadget serial), `/dev/ttyS1`, `/dev/fb0`, `/dev/disp`, `/dev/g2d`, `/dev/ion`, and **`/dev/atsha204a`** (a Microchip ATSHA204A secure-element interface). Its JSON `method` handlers include `handleGetInfo`, `handleSetVolume`, `handlePlayAudio`, `handleStopAudio`, `handleSaveToFile`, `handleSetFileCRC`, `handleDeleteFiles` and `handleSetBacklight`. Commands include `SHOW_JPG`, `GET_DEVICE_SCREENMESSAGE` and `REQUEST_UPLOAD_KEY`.
- The ATSHA204A is probably used for licensing or authentication. The replacement must not depend on it or try to bypass it.

## Vendor SDK and Wi-Fi notes (Confirmed from `~/Tina-Linux`)

- The SDK has T113 boards `t113-evb1` and `t113-mq_r`, kernel `lichee/linux-5.4`, and `brandy-2.0` for boot0/U-Boot. No SK18 board files, `SYK` or `5inch_720x1280` strings are in it, so the SK-18 DTS/panel config exists only in the shipped image.
- The vendor kernel has Goodix touch drivers (`goodix.c`, `gt9xxnew`) and many `disp2` LCD panel files; the `lcd0` parameters in the SK18 DTB select the generic `default_lcd` driver.
- The vendor kernel ships the Realtek `rtl8189fs`, `rtl8723ds`, `rtl8821cs` and `rtl8xxxu` drivers; the stock image has an `rtlwifi` modules directory. RTL8188GU support in mainline is not confirmed: web sources describe `rtl8xxxu` as covering "RTL8710BU (aka RTL8188GU)" and mention a USB mode-switch (CD-ROM mode) quirk. The exact USB ID of the supplied adapter is needed to confirm.

## Board device tree (from the U-Boot package DTB, `model = "SK18"`)

Decompiled with `dtc -I dtb -O dts` (about 127 warnings, mostly vendor-style nodes that are not standard bindings). Compatible: `allwinner,t113`, `allwinner,r528`, `arm,sun8iw20p1`.

| Block | Finding | Status |
|---|---|---|
| CPU/RAM | 2x Cortex-A7; RAM at `0x40000000`. The vendor DTB memory node says 512 MiB, but that is a template: the running mainline kernel reports 128 MiB (131072K), which is what the T113-S3 has in its package | Corrected 2026-11-07 (measured on the board, D-013) |
| Console | UART3 on PB6/PB7 `okay`; UART0 `disabled`; `chosen` says `console=ttyS0` but inittab uses `ttyS3` | Confirmed, conflict to resolve on hardware |
| SD card | `sdmmc@4020000` (SDC0), 4-bit, high-speed, card-detect GPIO | Confirmed |
| USB | OTG (`usbc0`, ID and VBUS-detect GPIOs), EHCI0/OHCI0 enabled | Confirmed |
| Display | `lcd0`: `5inch_720x1280`, `lcd_if=4`, 2 lanes, dclk 70 MHz, PWM channel 3 backlight | Confirmed values; DSI interpretation Inferred |
| Touch | Goodix `gt9xxnew_ts`, I2C id 2, address 0x14 (reported max 800x480, which disagrees with 720x1280) | Confirmed, inconsistency to check |
| Audio | codec, DMIC, sound nodes | Confirmed present |
| Other | GPADC keyboard node, 2x TWI/UART/PWM/SPI, RTC | Confirmed present |

**Unknown:** exact pin assignments for LCD, touch IRQ/reset, backlight enable and USB VBUS; panel part number; whether the `mk10` / `mk20-plus` files in `boot-resource` select hardware variants.

## Hardware status table

| Block | Original system | Mainline status | Needed DT node / driver | Test result | Open issue |
|---|---|---|---|---|---|
| Serial | UART3 PB6/PB7 | `sun6i-a31-uart` yes | `uart3` pinctrl | not tested | confirm header and baud |
| DRAM/boot | vendor boot0 | mainline SPL supports T113-S3 | U-Boot defconfig | not tested | DRAM parameters |
| SD/MMC | SDC0 | yes | `mmc0` | not tested | card-detect GPIO |
| USB host | EHCI0/OHCI0 | yes | `usb` / `usbphy` | not tested | VBUS regulator |
| Wi-Fi RTL8188GU | `rtlwifi` modules | check `rtl8xxxu` for USB ID | firmware | not tested | USB ID unknown |
| Display | lcd0 720x1280 | mainline T113 display support limited | panel + `de` / `tcon` | not tested | verify bus type |
| Touch | GT9xx | `goodix` driver | I2C node | not tested | pins, resolution |
| Audio | codec | partial | codec node | not tested | |

## Next steps

1. Verify UDISK and the full-set hashes; analyse UDISK contents.
2. (Done) Kernel DTB located: it is the `SK18` board DTB from the U-Boot package.
3. Decide the boot-chain strategy (reuse vendor boot0/U-Boot with a mainline kernel first, or mainline SPL/U-Boot).
4. Establish serial console access on a test card.
