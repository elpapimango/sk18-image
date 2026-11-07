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
- **Inferred:** the odd `bcdUSB=0x01` and the missing serial driver explain the Windows trouble. `adb` over USB may give a shell on the stock firmware (not verified).
- **Next:** once the device is connected to another computer, check `lsusb` / Device Manager for the VID:PID it shows, and whether `adb devices` lists it. Ask before running anything on the device.
