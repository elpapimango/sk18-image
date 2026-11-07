# Decision log

Each entry: observed, inferred, changed, why, reproduce, test, roll back.

## D-001 Work from extracted partition copies, never the original image
- **Observed:** the original image is 4 GiB and is the only hardware reference.
- **Changed:** `waveshare_sk18.img` is only read. Partitions are copied with `dd` into WSL ext4 (`~/sk18-work/parts`) and checked by SHA-256 against the source ranges. `image/` is git-ignored.
- **Reproduce:** `sfdisk -d` for offsets, then `dd if=<img> of=<part>.bin bs=512 skip=<start> count=<sectors>`.
- **Roll back:** delete `~/sk18-work`; the original is untouched.

## D-002 Install analysis tools in WSL
- **Observed:** `dtc`, `fdtdump`, `mkimage`, `binwalk`, `7z`, `bison`, `flex`, `swig`, `shellcheck` were missing.
- **Changed:** `apt install device-tree-compiler u-boot-tools binwalk p7zip-full bison flex swig shellcheck` (root, with user approval).
- **Roll back:** `apt remove` the same packages.

## D-003 Buildroot version baseline (not yet pinned)
- **Observed:** `~/buildroot` is master at `60a3423e95` (2026.08-1646, reports 2026.11-git); it offers U-Boot 2026.10 and Linux 7.3.1 as the latest versions. It has no T113 defconfig.
- **Inferred:** a pinned release or commit and kernel version will be chosen in Phase 3/4 after checking T113-S3 support in that U-Boot and kernel.
- **Changed:** nothing yet.

## D-004 The OpenDeck client runs on the device
- **Observed:** the stock firmware makes the SK-18 a USB CDC-ACM peripheral controlled by a PC application; it has no network client.
- **Decision (user, 2026-11-07):** the final image runs a client on the SK-18 itself. It connects over Wi-Fi to OpenDeck or to the user's `controldeck-rs` app, shows images on the keys and sends commands, like a Stream Deck.
- **Consequences:** Wi-Fi (RTL8188GU), the display pipeline (including per-key images) and touch/key input are required for the final image, not optional. The client protocol is specified by OpenDeck / `controldeck-rs`, not by the stock serial protocol; the stock protocol is only useful for understanding the hardware.
- **Open:** the exact OpenDeck transport and the `controldeck-rs` interface (to be provided by the user).

## D-005 UART location is unknown; use the stock firmware's USB modes as a probe
- **Observed:** the console is UART3 (PB6/PB7) in the board DTB, but the header location is unknown. The user's Windows PC had driver problems with the device; the user will try another computer.
- **Observed in firmware:** `/data/serial_uart.sh` and `/data/serial_adb.sh` expose a CDC-ACM gadget (`1d6b:0104`, `bcdUSB=0x01`); `/data/setusbconfig` can also switch to `adb` (vendor ID `0x18d1`), `mtp` and `rndis`. `/etc/init.d/adbd` starts `adbd`.
- **Correction (2026-11-07):** the live device reports `bcdUSB 2.00`, manufacturer "Allwinner Technology Inc.", product "Serial" (not the strings in `serial_uart.sh`), so my earlier guess that `bcdUSB=0x01` caused the Windows problem was wrong. The cause on Windows is still unknown.
- **Observed on the DietPi machine (192.168.0.190, Linux 6.18 aarch64, Raspberry Pi class):** connecting the SK-18 adds an internal 4-port hub (`05e3:0610`) with two devices: the T113 CDC-ACM gadget `1d6b:0104` (`/dev/ttyACM0`, high speed, self-powered, 500 mA) and a separate HID device `4250:426f` "SYK syk_keyboards" (`usbhid`, full speed) with four HID interfaces: a boot keyboard (EP1 IN, 8 B), a vendor interface (EP2 IN / EP3 OUT, 32 B), a mouse/system/consumer interface (EP4 IN, 32 B) and a second vendor interface (EP5 IN / EP6 OUT, 32 B). `adb` is not installed there and the device is not in ADB mode.
- **Inferred:** key presses reach a PC through the separate HID chip, not through the T113. How (or whether) the T113 sees those keys is unknown; candidates are `/dev/ttyS1` or the touchscreen (`/dev/input/event1`). This must be settled before designing input handling.
- **Observed (passive read, 2026-11-07):** reading `/dev/ttyACM0` for 25 s (115200 raw, nothing written) returned no bytes, so the stock firmware only answers requests. The HID reports were not read (`/dev/hidraw*` need root).
- **Observed (`getInfo`, 2026-11-07):** one read-only request was answered with valid CRCs; see `docs/stock-serial-protocol.md` (SK18, V2.31, 720x1280, 18 keys in 3x6).
- **Observed (Wi-Fi adapter):** the supplied adapter shows as `0bda:1a2b` "RTL8188GU 802.11n WLAN Adapter (Driver CDROM Mode)", manufacturer Realtek, one Mass Storage interface. It is in CD-ROM mode and has to be mode-switched (for example with `usb_modeswitch`, not installed on the DietPi machine) before a Wi-Fi driver can bind; its post-switch USB ID is not yet known.
- **Rule:** development of B uses only non-destructive stock methods (device info, show image, backlight, key events); the stock file-save and file-delete methods are never used.

## D-006 How the on-device client reaches `controldeck-rs`
- **Observed:** `controldeck-rs` (`C:\Users\mholmert\Workspace\control-deck-rs`, also on the DietPi machine) is a gateway: `DeviceProvider` drivers (currently Elgato, over USB HID) feed `gatewayd`, which serves the OpenDeck plugin through `/ws/client` (JSON over WebSocket: `hello`, `claim`, `set_outputs` with `image`/`fill`/`clear`/`brightness`, and `input` events; token auth; one client slot). OpenDeck device plugins run on the OpenDeck host, so a program on the SK-18 cannot be an OpenDeck plugin itself.
- **Inferred:** the SK-18 needs a device-side connection to the gateway, i.e. the gateway treats it as a network-attached deck. `/ws/client` does not fit (it is the consumer side).
- **Options:** (A) add a new "remote device" endpoint and `DeviceProvider` to `controldeck-rs` that the SK-18 client connects to; (B) as an interim step, write a `deck-sk18` driver that uses the stock firmware's USB serial protocol so the gateway side can be developed and tested before the Linux port is finished.
- **Open:** user to choose between A, B or both.
