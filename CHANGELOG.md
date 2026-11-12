# Changelog

The version of a release is the version of the client workspace (`client/Cargo.toml`); the board reports it to the host as its
firmware version (`open-<version>`) and `/etc/sk18-release` on the card says which version and commit built the image.
A release is made by pushing the tag `v<version>`: the `Release` workflow builds the image and publishes it.

## [Unreleased]

## [0.1.0] - 2026-11-12

First release.

### Image
- Mainline U-Boot 2026.10 (SPL with the DRAM parameters of the vendor boot0) and Linux 7.3.1 built with Buildroot (pinned in
  `tools/build/BUILDROOT_COMMIT`), Bootlin ARMv7 hard-float glibc toolchain, ext4 root file system; one 64 MB SD card image,
  flashable with balenaEtcher (`sk18-image-0.1.0.img.xz`), Raspberry Pi Imager or `dd`.
- Display: DSI panel initialised, TCON-top and pad-mux fixes, 720x1280 framebuffer; PWM backlight driver for the T113.
- Boot animation (`sk18-splash`) and a quiet boot.
- Key input: the key controller on UART1 decoded by `sk18-keys` (3 rows by 6 columns when the panel is used on its side).
- Wi-Fi (RTL8188-class USB adapter, firmware blobs fetched hash-pinned at build time), `wpa_supplicant`, regulatory database;
  mDNS: the board is `sk18.local`.
- USB gadget: `1d6b:0104` "SK-18 Deck" with a HID function (the deck protocol, 512-byte reports) and a serial console
  (`ttyGS0`, login on the UART as well).

### Deck protocol
- `sk18-link` speaks the stock firmware's deck protocol (`getInfo`, `setBacklight`, whole-screen JPEG, `keyStateChanged`, ...)
  to control-deck-rs and OpenDeck, carried over the HID function (see `docs/hid-transport.md`) so that the host drivers need not
  know which firmware they talk to. Wi-Fi, Home Assistant and picture offset can be set from the controldeck web UI over the
  link. Stock methods this firmware does not implement answer with an error.
- The answers were compared with a real SK-18 running the stock firmware (V2.31) and made to match: `setBacklight` and `setVolume`
  results and error texts, `stopAudio`, `playAudio`, `getFilesBySuffix`, the unknown-method list, the error frame for a picture that
  cannot be decoded, and the ids of events (counting down from `0xFFFFFFFF`). A recorded stock session is replayed in the tests
  (`client/sk18-link/tests/stock_conformance.rs`); `docs/stock-serial-protocol.md` has the details.

### Known limits
- No audio, typing or file-management methods of the stock protocol (they answer like a stock device without audio or files, or with an error).
- The USB transport differs from the stock firmware: this image is a HID function, the stock firmware a serial port, so tools that open a serial port (the vendor demo) do not talk to this image.
- Not bit-for-bit reproducible: every build gets a fresh root partition UUID.
