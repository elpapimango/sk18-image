# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A reproducible, mainline-based Buildroot SD image (U-Boot 2026.10, Linux 7.3.1) for the Waveshare SK-18 "ScreenKey" (Allwinner T113-S3, 720x1280 panel used turned 90° as a 3-row x 6-column key grid), plus the on-device Rust client. The device works as a macro deck with the user's `controldeck-rs` (`C:\Users\mholmert\Workspace\control-deck-rs`, running on the DietPi at 192.168.0.190) and with OpenDeck. Why and how: `docs/decision-log.md` (D-001 onwards, newest last; read the latest entries first), `docs/hardware-status.md`.

Working rules that have held throughout: evidence first, record every decision in the decision log, commit in git, never modify the original image or card, positively identify an SD target before writing, no proprietary binaries, credentials or large images in git, pin versions, quote paths.

## Layout

- `br2-external/`: Buildroot external tree (`configs/waveshare_sk18_defconfig`, `board/waveshare/sk18/` with DTS, kernel/U-Boot fragments, `overlay/` init scripts and udev rules, `patches/linux/*.patch`, packages `sk18-keys`, `sk18-link`, `sk18-splash`, `sk18-wifi-firmware`).
- `client/`: Cargo workspace for the board (`sk18-keys` UART1 key decoder and shared key geometry, `sk18-link` daemon that speaks the stock deck serial protocol to the host over the USB gadget, `sk18-splash` boot animation). `Cargo.lock` is committed; the serial code is Unix only.
- `tools/flash/`: `flash-sdcard.sh <sha256>` (guarded flasher, runs on the DietPi; add each new image's ext4 PARTUUID to its KNOWN list), `provision-wifi.sh`.
- `docs/`: decision log, hardware status, protocol notes (`stock-serial-protocol.md`, `hid-transport.md`, `uart1-key-protocol.md`), phase write-ups.
- `work/`: scratch scripts and built images (git-ignored). `image/`: original card image and vendor zip, git-ignored, read-only, never `git add` it.

## Build

Buildroot lives in WSL distro `BuildRoot` (`~/buildroot`, output `~/sk18-build`). From Windows: `wsl -d BuildRoot bash /mnt/c/Temp/sk18/work/rebuild_all.sh` (dirclean of the three client packages, then make) or `work/rebuild_kernel.sh` (after a kernel patch change); progress in `~/sk18-build/build.status`. The first build needs network (cargo vendor, crates.io). Client crates: `cargo test` and `cargo clippy --all-targets -- -D warnings` in `client/`; the board is 32-bit hard-float Cortex-A7 (`armv7-unknown-linux-gnueabihf`).

## Deploying and testing on hardware

- The board is one USB device with a HID function (hidraw on the host = deck protocol, D-041) and one ACM port (getty console) and no network unless Wi-Fi credentials are on the card. Anything that drops the USB gadget (unbinding the UDC) must run as a detached `setsid sh script &`.
- Images are flashed from the DietPi's card reader; the user swaps the card and replugs. `scp` the image to the DietPi, then `bash flash-sdcard.sh <sha256>`.
- controldeck-rs is built and tested on the DietPi (`~/control-deck-rs-sk18`; Windows `cargo check` skips the Unix-only gatewayd code), installed as `/usr/local/bin/gatewayd` for the `control-deck-gatewayd` service (keep a timestamped backup, roll back if it does not become active). Its config is `/etc/control-deck/config.toml`.
- On Windows prefer script files over inline quoting for ssh/scp/PowerShell commands.

## Host app in the vendor zip (reference only)

`image/MK Series Upper Computer Open Source_V1.0.zip` holds the vendor Qt host app and the MK10/MK20 firmware images. Inspect without extracting (`unzip -l`, `unzip -p`); extract only into a scratch directory. Protocol summary (full version in `docs/stock-serial-protocol.md`): USB CDC 115200 8N1, VID/PID `1d6b:0104` or `1234:5678`; frame `A1 A5 5A 5E | u32 id | u32 cmd | u32 size | u32 crc32(size) | payload | u32 crc32(payload)` little endian; cmd 100 = whole-screen JPEG (acknowledged by an empty cmd-100 frame with the same id), 101 = JSON (`method` field: `getInfo`, `setBacklight`, `keyStateChanged`, ...), 102 = end.
