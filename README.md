# sk18-image

A reproducible SD card image for the **Waveshare SK-18 "ScreenKey"** (Allwinner T113-S3, 720×1280 touch panel used on its side as
a 3×6 key grid): mainline U-Boot and Linux built with Buildroot, plus the Rust programs that run on the board. Everything
that ends up on the card is in this repository.

The board then works as a macro deck: a **deck** for [control-deck-rs](../control-deck-rs) (and through it OpenDeck), or directly
with OpenDeck through [opendeck-sk18](../opendeck-sk18). It speaks the same USB protocol as the stock firmware (carried over a
USB HID function, see [docs/hid-transport.md](docs/hid-transport.md)), so the host drivers do not need to know which firmware
they talk to.

## Flash a release (balenaEtcher or any other tool)

1. Download `sk18-image-<version>.img.xz` from the releases page (or build it, below).
2. Open **balenaEtcher**, choose *Flash from file* and pick the `.img.xz` (no need to unpack it), choose the SD card, *Flash*.
   Raspberry Pi Imager (*Use custom*), `dd` and others work as well; the checksum of the unpacked image is in `SHA256SUMS`.
   The card needs 1 GB or more; the image is about 64 MB.
3. Put the card into the SK-18, plug it into a USB port. The boot animation plays, then the board shows up as a USB device
   (`1d6b:0104`, "SK-18 Deck": a HID function for the deck protocol and a serial console).

Wi-Fi and Home Assistant are set from the controldeck web UI (device page, *Settings* tab) over the USB link, not on the card.

## Build

Needs Linux (on Windows: WSL) with Buildroot's host requirements and network access for the first build.

```sh
git clone https://gitlab.com/buildroot.org/buildroot.git ~/buildroot   # the version used is recorded in docs/decision-log.md
tools/build/build.sh                      # ~/sk18-build/images/sdcard.img
tools/build/release.sh 0.2.0              # dist/sk18-image-0.2.0.img.xz + SHA256SUMS for Etcher
```

`build.sh --clean-clients` rebuilds the Rust programs after a change under `client/`, `--clean-kernel` the kernel after a patch.

## Layout

| Path | What |
| --- | --- |
| `br2-external/` | Buildroot external tree: `configs/waveshare_sk18_defconfig`, the board files (device tree, kernel and U-Boot fragments, init scripts, udev rules), kernel patches, packages |
| `client/` | The programs on the board (Cargo workspace): `sk18-keys` (key decoder), `sk18-link` (deck protocol), `sk18-splash` (boot animation) |
| `tools/build/` | `build.sh`, `release.sh` |
| `tools/flash/` | Guarded flasher for the maintainer's card reader on the DietPi (`flash-sdcard.sh`), `provision-wifi.sh` |
| `docs/` | Decision log (D-001 onwards), hardware status, protocol notes |

No proprietary binaries, credentials or large images are kept in git. The stock firmware image and the vendor package the
project started from stay outside the repository (`image/`, git-ignored).

Working notes for contributors and for Claude Code are in [CLAUDE.md](CLAUDE.md).

## Licence

The original code in this repository (the programs under `client/`, the scripts, the Buildroot package and board files, the
documentation) is licensed under either of the [Apache License 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT), at your
option. The kernel patches in `br2-external/patches/` and the device tree are derived from Linux and stay under
GPL-2.0 (with the Linux syscall note where the file says so). Buildroot and the packages it downloads have their own licences.
This is an independent community project; it is not affiliated with or endorsed by Waveshare.
