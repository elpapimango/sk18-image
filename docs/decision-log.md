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

## D-007 Test SD card on the DietPi machine
- **Observed (2026-11-07, read-only):** USB card reader "Generic MassStorageClass", serial `3063422494200319`, enumerated as `/dev/sda`: 7,864,320 sectors (4,026,531,840 bytes, the same size as the original image), removable, not mounted, MBR with one FAT32 partition (start sector 32, factory-style format). The DietPi machine boots from `/dev/mmcblk0` (115.5 GB), which is a different device.
- **Inferred:** a blank or factory-formatted test card; contents of the FAT partition were not inspected.
- **Rule:** nothing is written to it until a flash is explicitly requested. Before any write, re-run `lsblk -o NAME,SIZE,MODEL,SERIAL,TRAN,RM,MOUNTPOINTS` and require serial `3063422494200319`, size 4026531840 bytes, transport usb and no mountpoints. The device name (`sda`) can change between plugs; the serial cannot.
- **Rollback:** the card is replaceable; the original image and the extracted partitions stay untouched.

## D-008 Wi-Fi adapter: confirmed mainline support and firmware
- **Observed (2026-11-07, DietPi machine, Linux 6.18):** after `usb_modeswitch -v 0bda -p 1a2b -KR` the adapter re-enumerates as `0bda:b711` "802.11n WLAN Adapter" (Realtek). The mainline `rtl8xxxu` driver binds by itself: "RTL8710BU rev A (SMIC) romver 1, 1T1R", firmware `rtlwifi/rtl8710bufw_SMIC.bin` (revision 16.0) loads.
- **Consequence:** no out-of-tree driver is needed. The final image needs `rtl8xxxu` (plus cfg80211/mac80211), the `linux-firmware` file `rtlwifi/rtl8710bufw_SMIC.bin`, and a mode switch at plug-in time (`usb_modeswitch` plus a udev rule for `0bda:1a2b`), because the adapter first appears as a CD-ROM.
- **Unknown:** whether the T113 mainline kernel's USB host (EHCI/OHCI) works with this adapter; scan/association not tested yet.
- **Changed on the DietPi machine:** installed the `usb-modeswitch` package (`apt`), switched the dongle once. Rollback: unplug/replug restores CD-ROM mode; `apt remove usb-modeswitch`.

## D-009 SK-18 orientation (2026-11-07)
- **Observed:** the firmware reports a portrait grid of 3 columns x 6 rows. With three keys lit (firmware row/col: red (0,0), blue (5,0), green (0,2)) the user saw red top right, blue top left, green bottom right on the device as used, i.e. turned 90 degrees clockwise. Touches reported the same firmware numbering as the lit keys, so touch and display rotate together.
- **Changed (control-deck-rs, branch `feature/deck-sk18`):** an `Orientation` setting in `deck-sk18` rotates the key grid, the key images and the touch events. The mapping is `logical row = firmware column`, `logical column = rows - 1 - firmware row`. `gatewayd` defaults to 90 degrees clockwise; `CONTROL_DECK_SK18_ORIENTATION` overrides it.
- **Test:** the unit tests encode the three observations; the production gateway reports a 3x6 grid. Rollback: set `CONTROL_DECK_SK18_ORIENTATION=0`.

## D-010 SK-18 in the production gateway for OpenDeck (2026-11-07)
- **Observed:** `/etc/control-deck/config.toml` only knew the Stream Deck XL, so the OpenDeck plugin held no SK-18 keys.
- **Changed on the DietPi machine (through the management API, config revisions 1 to 4):** added binding `sk18` (manufacturer `waveshare`, product `sk18`), resource set `sk18-keys` (grid 3x6), and an `opendeck` profile assignment of `sk18-keys` to `opendeck` (exclusive). `gatewayd` replaced by the `feature/deck-sk18` build; udev rule `70-control-deck-sk18.rules` and a systemd drop-in `DeviceAllow=char-ttyACM rw` added.
- **Result:** `opendeck` holds all 18 keys and the device-level target; OpenDeck registered the device and set brightness 36.
- **Rollback:** `POST /api/v1/config/rollback/<index>` (history from `GET /api/v1/config/history`); the old binary is `/usr/local/bin/gatewayd.prev-20261107-210540`; config copy in `/var/backups/control-deck-20261107-210540`.
- **Update (config revision 5):** the `work` profile also assigns `sk18-keys` to `opendeck` (exclusive), so the SK-18 stays an OpenDeck deck in both profiles. Switching profiles was not exercised.

## D-011 First mainline image built (2026-11-07)
- **Observed:** `waveshare_sk18_defconfig` in `br2-external/` builds to completion with Buildroot `60a3423e95`: U-Boot 2026.10 (SPL + U-Boot, 487 KB), Linux 7.3.1 (`zImage` 6.1 MB), the board DTB (18.6 KB) and a 60 MB ext4 root, combined in `sdcard.img` (64 MB, SHA-256 `9fdda5c611db6bae6927cbd5b9bddfe1d1a2cd56a3e876c2454e851954d7e0f9`).
- **Checked in the image:** SPL header (`eGON.BT0`, `SPL` marker) at 8 KiB; uImage-wrapped U-Boot at 40 KiB; GPT with one partition at LBA 2048 whose GUID equals the `PARTUUID` in `/boot/extlinux/extlinux.conf`; `/boot` holds `zImage`, the DTB and `extlinux/`; `getty` on `ttyS3` and `ttyGS0`; `root` has an empty password (bring-up only).
- **Build problems fixed on the way:** missing `BR2_TARGET_UBOOT_NEEDS_GNUTLS` (U-Boot host tool `mkeficapsule`), and the DTS include path (`allwinner/` subdirectory); line endings pinned with `.gitattributes`.
- **Not yet known:** whether it boots. Failure before the kernel gives no output without the UART3 header.
- **Reproduce:** `br2-external/board/waveshare/sk18/README.md`. **Roll back:** the original card.

## D-012 Test card flashed with the first mainline image (2026-11-07)
- **Changed:** `sdcard.img` (SHA-256 `9fdda5c6…7e0f9`, 63,963,136 bytes) written to the card with serial `3063422494200319` (found by serial, not by device name; 4,026,531,840 bytes, USB, removable, nothing mounted, not the system disk) with `dd bs=1M conv=fsync`. Read back and compared: identical hash. Only the first 61 MiB were written; the rest of the card keeps its old contents (a FAT remnant that nothing reads).
- **Result on the card:** one ext4 partition, PARTUUID `0915bee9-7461-4b03-9e9d-3e6d87e23276`, equal to the one in `/boot/extlinux/extlinux.conf`.
- **Not yet done:** boot the SK-18 from this card.
- **What to expect and look for:** the screen stays dark (no display driver in this image; that is not a failure). The SK-18's USB hub and HID chip (`4250:426f`) still appear on the host. If the kernel boots, a new `g_serial` device (`0525:a4a7`, `/dev/ttyACM*`) appears with a login prompt; the stock `1d6b:0104` serial port does not appear.
- **Roll back:** put the original or backup card back in the SK-18.

## D-013 The mainline image boots on the SK-18 (2026-11-07)
- **Observed:** with the test card in the SK-18, about 6 seconds after the internal USB hub appeared, the host (DietPi machine) enumerated a new device `0525:a4a7`, product "Gadget Serial v2.4", manufacturer "Linux 7.3.1 with musb-hdrc", as `/dev/ttyACM0`. Logging in as `root` (empty password) on it gives a shell on `sk18`.
- **Measured over that shell:** Linux 7.3.1 `armv7l`, 2 CPUs (Cortex-A7, part 0xc07), command line as in `extlinux.conf`, root `ext4` on `mmcblk0p1` (rw), SD card 3.75 GiB high speed, **RAM 131072K (128 MiB)**, 98 MB free after boot, SoC temperature 27 C, no cpufreq driver (fixed clock), only kernel warning "GPT: Use GNU Parted to correct GPT errors" (the image has no backup GPT by design). EHCI/OHCI at `4200000`/`4200400` registered; musb UDC at `4100000`; `g_serial` ready.
- **Correction:** RAM is 128 MiB, not 512 MiB. The vendor DTB memory node (512 MiB) is a template; the T113-S3 has 128 MiB DDR3 in its package. Docs, defconfig comment and DTS comment fixed.
- **Meaning:** mainline SPL DRAM init (parameters from the MQ-R base, equal to the original boot0 header), U-Boot, the kernel, the board DTS (UART3, mmc0, USB OTG as peripheral), the SD card and the USB gadget all work on this board. The UART3 header was not needed.
- **Not working or not yet in the image:** display (screen stays dark), backlight, touch, Wi-Fi, audio, anything key-related. The stock serial protocol is gone while this card runs, so the gateway shows the SK-18 as detached.
- **Tools used:** `serial_sh.py`-style login over `/dev/ttyACM0` from the DietPi machine (read-only commands). Rollback: put the original or backup card back.

## D-014 Display image built and flashed (2026-11-07)
- **Changed:** the image now has the display pipeline (DE2 mixer, TCON top, TCON LCD0, MIPI-DSI, D-PHY), a `panel-simple` entry for the SK-18 panel (`patches/linux/0001-drm-panel-simple-add-waveshare-sk18-panel.patch`, applies cleanly to Linux 7.3.1), a software-PWM backlight on PG10, and the text console on the screen (`console=tty0`, `fbcon=rotate:3`, font 10x18). See `docs/phase5-display.md`.
- **Checked in the build:** kernel options on, patch present in the source, panel and six display nodes in the compiled DTB, new boot arguments in `extlinux.conf`.
- **Flashed:** `sdcard.img` SHA-256 `28dbfba4e1fbf45fcb0f3052844baa16b1ce6dbe177d848144c9b29375ed3f8c` to the test card (serial `3063422494200319`, found by serial; checks passed), read back and identical. New PARTUUID `ba26212a-aed0-4795-b8a4-f010ac5120fb`.
- **To look at on the board:** backlight on or off; any picture; whether the boot text is readable, and upside-down or sideways (`fbcon=rotate` value); whether it flickers (software PWM). The serial console on the DietPi machine shows the kernel DRM messages.
- **If there is no picture:** DSI mode flags (burst instead of sync pulse), the role of PE5, the panel supply.

## D-015 Display image with the TCON-top fix flashed to a new card (2026-11-07)
- **Observed on the board (previous card, image `28dbfba4…`):** boots, screen dark. `dmesg`: `sun6i-mipi-dsi 5450000.dsi: Couldn't get the DSI mod clock`, repeated; `devices_deferred` = `5450000.dsi`; no DRM card. Cause and fix: `docs/phase5-display.md`, `patches/linux/0002`.
- **Built:** Buildroot with both kernel patches, `sdcard.img` SHA-256 `28b3e4d9ecf6fb6140e51d50156c1f8e421de12ba94a536dd01d67ab0d69075a`. Checked before flashing: both patches present in the built tree, `zImage` and DTB inside the image identical to the build outputs, `PARTUUID` `294d0c07-a840-4b9a-8280-1a8731c64ef2` in `extlinux.conf` equals the GPT entry, SPL at 8 KiB and U-Boot at 40 KiB.
- **Flashed:** to a second, new SD card in the DietPi machine's reader (factory FAT32, 4,026,531,840 bytes, USB, removable, unmounted, not the system disk). Note: the serial `3063422494200319` that `flash.sh` matches is the card **reader's** serial, so it does not tell cards apart; the size, the factory layout and the unmounted state were what identified this card. Read back identical.
- **Side finding:** after a half-finished file transfer over the serial console (my script), the SK-18 console stopped answering although the USB gadget stayed enumerated. Not investigated further; a fresh boot from the new card replaces that state. The transfer approach is abandoned in favour of re-flashing the card.
- **Next:** the user puts this card into the SK-18; check the screen and the console messages. Roll back: any earlier card.
