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

## D-016 DSI pad mux fix built and flashed (2026-11-07)
- **Observed:** with patch 0002 the display pipeline probes and the kernel reports a connected, enabled 720x1280 connector, but the screen is black (backlight on). Details in `docs/phase5-display.md`.
- **Changed:** `pinctrl-0 = <&dsi_4lane_pins>` on `&dsi` (PD0 to PD9, function `dsi`, drive strength 30), commit `4ab85d3`. Image SHA-256 `2de7a130d1ae3a63ee93e61e2b04fdac8cbdbba499d0d5aab354abe72300ff85`, PARTUUID `540e6ce1-a6a0-4e45-94a0-41b16c636a18`.
- **Flashed:** to the card in the DietPi machine's reader. Before writing I checked that it held one of our own images (PARTUUID `ba26212a-…`, the first display image), 4,026,531,840 bytes, USB, removable, unmounted, not the system disk. Read back identical. (The user now uses two cards: one in the SK-18, one in the reader.)
- **Next:** swap the cards; look at the screen.

## D-017 Stock-side display register dump, and a bug in the SK-18 device id (2026-11-08)
- **Observed:** with the stock card in the SK-18 and the module hook (`tools/stock-regs`), the stock kernel loaded `sk18_regs.ko` without problems and dumped the display registers twice (early, and 40 s later; they are almost identical). The vendor driver reports: 720x1280, 60.2 fps, `de_rate` 300 MHz, backlight 98, one layer. Dumps are in `work/dumps/` (not committed: measurements of a vendor system).
- **Observed (gateway journal):** the SK-18 came up on a different USB hub socket the second time (`1-1.3.1` instead of `1-1.1.1`). The user confirmed the cable had been moved (to another socket). The driver derived the device id from that path, so the gateway saw two devices, the `sk18` binding matched both ("matches several devices"), the profile was not applied, no key images were sent, and the stock firmware kept playing its idle animation. Not a reboot loop: one clean attach/detach per run.
- **Changed (control-deck-rs, commit `6141f5e`):** the id is now independent of the port: serial number if the unit reports one, otherwise `waveshare-sk18` (first unit) and `-2`, `-3` by USB position. Tests added (cable moved to another socket, several units). Deployed to the production `gatewayd` on the DietPi machine (binary backup `/usr/local/bin/gatewayd.v2-20261108-130511`); the restart also cleared the stale duplicate.
- **Restored:** the hook was removed from the stock card; every original file on its FAT partition is byte-identical to the backup taken before (`~/sk18-backups/stock-p1-before-20261108-123345.bin`).
- **Next:** dump the same registers on the mainline kernel (`tools/stock-regs/mainline-dump.cmds`) and compare.

## D-018 OpenDeck layout moved to the new device id (2026-11-08)
- **Observed:** after the stable-id change the gateway side was healthy (device attached as `waveshare-sk18`, `opendeck` held all 19 targets, profile active) but all 18 keys had the output `clear` and the SK-18 kept playing its idle animation. OpenDeck (running on this Windows machine) had created a new empty device `cd-waveshare-sk18` (0 of 18 keys); the user's layout (14 of 18 keys) was still under the old port-based id `cd-waveshare-sk18-usb-1-1_1_1-9e97`. OpenDeck keys layouts by device id only (`docs/opendeck-integration.md` of control-deck-rs, line 97).
- **Changed:** with OpenDeck quit, backed up both folders to `work/opendeck-backup-20261108-074704`, then copied the old `Default.json` over the new device's empty `Default.json` (hash-identical, 14 keys). OpenDeck restarted: icons showed up on the SK-18.
- **Lesson:** an id change costs the user their layout in OpenDeck. The new id is stable across cable and port changes, so this should not repeat.
- **Rollback:** restore the folder from the backup path above, or delete `%APPDATA%\opendeck\profiles\cd-waveshare-sk18*` while OpenDeck is quit.
- **Untested:** whether a freshly booting SK-18 shows the icons when the gateway attaches during the stock firmware's start-up (frames sent too early might be ignored). Test: power-cycle the SK-18 while the gateway and OpenDeck run.

## D-019 Cards, and a stricter flash script (2026-11-08)
- **Cards (user's labels):** 1 = original (kept untouched, never used again); 2 = backup of the original (stock firmware; the card the hook experiments were done on, restored byte-identical); 3 and 4 = the mainline test cards. Use 2 for the normal deck (stock + OpenDeck), 3 or 4 for mainline experiments.
- **Observed:** the SK-18 booted the stock firmware twice while the user believed a mainline card was inside (the cards had been swapped while powered, and a stock-layout card was in the SK-18). FEL mode (`1f3a:efe8`) appeared once when the card was not recognised.
- **Changed:** `tools/flash/flash-sdcard.sh` (was `flash.sh`) now refuses any card whose partition labels show the stock layout (`boot-resource`, `env`, `UDISK`) and any card with unknown multi-partition content; it accepts blank/factory FAT cards and cards that hold one of our earlier mainline images (listed by PARTUUID). First version wrongly treated the label `rootfs` as a stock marker (the mainline image uses it too) and refused a valid card: fixed. Remember that the serial it matches is the card reader's.
- **Flashed:** newest image (`2de7a130…`, DSI pad mux fix) to the first mainline card (PARTUUID before: `540e6ce1-…`, i.e. it already carried this build but had been booted, so its filesystem differed); read back identical. The second mainline card is next.

## D-020 Black screen: the panel was never initialised (2026-11-08)
- **Observed (register diff, stock vs mainline, `work/regs_diff.py`):** CCU display clocks, TCON top, D-PHY and the mixer chain are the same on both systems, so clocks, routing and the video path are not the problem. Two real differences: (1) the DSI **command buffer** (+0x300..) of the working stock system holds DCS packets and the mainline one is empty; (2) PE5 (`PE_DAT` bit 5) is **low** on stock and **high** on mainline.
- **Decoded:** the stock buffer starts `05 29 00 1c`, a DCS short write of **0x29 (display on)**; behind it is a long write `B2 00 80 64 0c 06 2f 00 00 00 00 c0 18` whose CRC (`5f bc`) overwrote the tail of an earlier **E0 (gamma)** write. The stock panel therefore receives a full HX8394 initialisation. The previous finding "the stock kernel sends no panel commands" stays true (its `default_lcd` init is empty); the commands come from the stock **U-Boot**, which initialises the panel (that is also the boot animation) and the kernel then takes over without touching it.
- **Found in the U-Boot image:** `rawboot.bin` offset `0x010941b4` holds the panel tables (next to the strings `lcd_compatible_index.txt` and `/soc/lcd0_0`): three variants (HX8394 `B9 FF 83 94`, HX8399 `B9 FF 83 99`, and an Ilitek-style set). The HX8394 table is the one whose last long write matches the register leftover, and it equals mainline's `waveshare,5.0-dsi-touch-a` sequence (same panel family, same 70 MHz pixel clock) up to the gamma table; it then differs slightly and ends with a 12-byte `B2`.
- **PE5:** configured as an output and driven low on the working stock system; the vendor DTB gives it flag 1. So PE5 is an **active-low panel power enable**. The earlier "held low for 45 s" test could not show anything because the panel had no initialisation yet.
- **Changed (not yet run on the board):** `patches/linux/0001` is replaced: instead of a `panel-simple` entry (no commands) the SK-18 is now an entry in `panel-himax-hx8394.c` with the exact stock byte sequence, the stock timing, then sleep-out, 120 ms, display-on (the driver's existing flow; the sun6i DSI host calls `drm_panel_enable()` before it starts the high-speed clock, so the commands go out in low-power mode, the same order as stock). DTS: a `regulator-fixed` on PE5 (active low) feeds `vcc-supply` and `iovcc-supply`; the `enable-gpios`/`power-supply` guesses are gone. Kernel fragment: `CONFIG_DRM_PANEL_HIMAX_HX8394=y` replaces `DRM_PANEL_SIMPLE`.
- **Decision on the sequence:** exact stock bytes (proven on this unit) rather than mainline's Waveshare sequence. They differ in the tail only: mainline sends `B1 00` and `BD 00` after `BD 01`; the stock table does not. If the screen stays dark, the first variant to try is mainline's tail.
- **Not explained by this:** TCON `CPU_IF` (+0x60, bit 22 vs the FIFO-flush bit 16), DCLK enable bits (+0x44), and the DSI trigger words (+0x160..+0x168, +0x1f0) differ. They are unlikely to matter in video mode but stay on the list if the init does not fix it.
- **Result (2026-11-08, image `a54da0ea…`, cards 3 and 4):** **the screen works.** The user booted card 3 and the console text is on the panel. So the cause of the black screen was the missing HX8394 initialisation (plus PE5 polarity); clocks, TCON, DSI timing and mode flags were already right. The exact stock byte sequence worked; mainline's tail variant (`B1 00`, `BD 00`) was not needed.
- **Open:** orientation/rotation (the panel is portrait; `fbcon=rotate:3` is on the command line, not yet judged), touch, backlight PWM (software PWM on PG10), USB host VBUS (PG12), Wi-Fi. The TCON/DSI differences listed above did not matter.

## D-021 Touch is not the Goodix on I2C2 (2026-11-08)
- **Tried:** the Goodix node from the stock DTB (I2C2, 0x14, INT PB6, reset PB7), image `9e44924c…`. Result on the board: the I2C bus works (`Goodix-TS 0-0014` bound), but the probe fails with `Failed to get irq GPIO` (-22), and an `i2cdetect` of the bus shows **only 0x64** (an ATSHA204A). Nothing at 0x14 or 0x5d.
- **Why the node was wrong (all from the stock side):** the stock DTB marks both `twi@2502000` and its child `ctp@14` `status = "disabled"`; PB6/PB7 are the UART3 console pins (the stock pinmux shows `uart3` on both, and the register dump has them at mux 7); the working stock system has **no** touch-related GPIO or EINT configured at all (only PF6 as the SD card-detect EINT; PG12 is `usb1-vbus`, PG13/PG14 are `bt_wake`/`bt_hostwake`); and `allwinner,sun8i-ts` in the DTB is the thermal sensor. The stock kernel image has no touch driver strings and the rootfs has no touch module.
- **Working theory (Inferred, not yet confirmed):** the touch panel is a USB HID touch device on the T113's internal USB host port, which needs no I2C/INT/reset pins. The stock system drives PG12 (host VBUS) high; the mainline DTS never did (`usb1_vbus-supply` pointed at an always-on 5 V rail without the PG12 switch), so the port was unpowered and nothing could enumerate.
- **Also noticed:** `sdc1` (PG0-PG5) and `uart1` (PG6-PG9) plus `bt_wake`/`bt_hostwake` on the stock system mean there is an SDIO Wi-Fi/Bluetooth module on the board, in addition to the supplied USB RTL8188GU adapter. To check when doing Wi-Fi.
- **Changed (not yet run on the board):** `reg_usb1_vbus` regulator on PG12 (active high, always on) feeds `usb1_vbus-supply`; the Goodix node is removed (I2C2 stays, for the ATSHA); kernel gets `HID_MULTITOUCH`, `HIDRAW`, `TOUCHSCREEN_USB_COMPOSITE`; image gets `usbutils` (lsusb).
- **Test:** boot, `lsusb` / `/sys/bus/usb/devices`; if a touch HID device appears, `evtest` it. If nothing enumerates, the next candidates are an SPI/UART touch MCU (stock `uart1` is claimed, but by the Bluetooth lines) and reading `/proc/bus/input/devices` and `/proc/interrupts` on the stock system with the hook method of D-017.
- **Runtime GPIO write did not take:** setting PG12 as an output worked (`devmem` on the PIO config register) but the data bit would not latch (read back 0) in two tries; the kernel has no GPIO sysfs. Cause not found; the proper regulator in the DTS replaces the experiment.
- **Correction and result of the USB test (image `29ad4b67…`):** with PG12 powered the only device on the internal host port is the Wi-Fi dongle in CD-ROM mode (`0bda:1a2b`), so VBUS works (needed for Wi-Fi), but there is no touch device on USB and no input device at all. The USB-touch theory is refuted.
- **Found:** the stock DTB enables `rtp@2009c00` (`allwinner,sun8i-ts`): the SoC's **resistive touch-panel ADC**, with sensitivity 15 and filter type 1. This explains the missing pins and the stock userspace (tslib `pthres`/`variance`/`dejitter`/`linear`). My earlier remark that `sun8i-ts` is the thermal sensor was wrong; it is the touch ADC.
- **Changed (not yet run on the board):** new driver `patches/linux/0003` (`drivers/input/touchscreen/sun20i-d1-ts.c`, binding `allwinner,sun20i-d1-ts`, register programming from the vendor GPL driver), DTS node `touchscreen@2009c00` (IRQ SPI 62, `CLK_BUS_TPADC`/`CLK_TPADC`/`RST_BUS_TPADC`, sensitivity 15, filter 1), `CONFIG_TOUCHSCREEN_SUN20I_D1=y`. Image `150f8aa2…`.
- **Test:** `dmesg | grep sun20i`, `/proc/bus/input/devices`, then `evtest` on the new event device while touching the screen. Raw X/Y are 0..4095; mapping to the 720x1280 screen (and the rotation) is a calibration step for userspace or `touchscreen-*` DT properties.

## D-022 The touch ADC is not used either: touch most likely comes over UART1 (2026-11-08)
- **Mainline result (image `150f8aa2…`, with the new TPADC driver):** the driver probes, `/dev/input/event0` exists, the controller registers hold exactly the vendor values and the interrupt is wired, but in six attempts with a finger on the glass the controller never reported a pen-down (status register 0, no events). One earlier run showed the FIFO filling and 31 interrupts; it could not be reproduced.
- **Stock-side capture (card 2, hook `tools/stock-regs/stock-lunch-tp.sh`, 4 minutes, user touching the screen throughout; logs in `~/sk18-dumps3` on the DietPi machine, not committed):**
  - The stock kernel has only four input devices: `sunxi-keyboard` (LRADC), `sunxi-gpadc0`, `sunxi-ir`, the audio jack. **No touchscreen device.**
  - The TPADC registers read their reset defaults for all 240 samples (`CTRL0=0x0f800000`, `FIFOC=0x00000f00`) and the `tpadc` / `bus-tpadc` clocks have no users. The stock system never uses the touch ADC. The `rtp@2009c00` node in the stock DTB is enabled but nothing drives it.
  - Interrupt counts over the run: `uart1` 843, `twi2` 132, `ehci_hcd:usb1` about 97,000, `mmc0` 4,318, display 14,874. Nothing from the touch ADC.
  - The stock application (`KeyboardDevice`) references `/dev/ttyS1` (via `QSerialPort::setBaudRate`), `/dev/atsha204a`, `/dev/ttyGS0`, `/dev/fb0`, `/dev/disp`, and no input-device or touchscreen path.
- **Reading (Inferred):** UART1 (`/dev/ttyS1`, pins PG6 TX, PG7 RX, PG8/PG9 RTS/CTS in the stock pin mux) carries the touch and/or key data from a separate controller; the I2C2 traffic is the ATSHA204A licence/authentication chip (`/dev/atsha204a`); the large USB host interrupt count is unexplained and needs a look (the supplied Wi-Fi dongle was not an obvious source: it sat in CD-ROM mode). This also fits the earlier observation that the SYK "keyboard" HID chip is separate from the T113.
- **Changed (not yet run on the board):** removed the TPADC driver patch (0003) and its node and config (it is correct code for a device that uses the block, but not this one; it stays in git history at `ad68f0e`); enabled `uart1` on PG6/PG7 (+PG8/PG9) in the DTS to capture the raw bytes.
- **Test:** `stty -F /dev/ttyS1 <baud> raw`, `cat /dev/ttyS1 | od -t x1` while touching; baud rate unknown (try 115200 first, then 921600, 460800, 9600). If the controller must be asked first, the stock application's strings and its traffic (on the stock card) are the next source.
- **Card 2 restored:** FAT partition of the stock card is bit-identical to the backup taken before the hook (sha256 `1c6f1792…`, whole partition compared). Backups: `~/sk18-backups/stock-p1-before-tp-20261108-175004.bin` on the DietPi machine.

## D-023 The key events come over UART1 at 115200; protocol decoded (2026-11-08)
- **Observed (mainline image `dbb79615…`, UART1 enabled):** this kernel names UART1 `ttyS0` (UART0 is disabled), so `/dev/ttyS1` does not exist; my first recorder listened to nothing. Recording `/dev/ttyS0` at four baud rates while the user touched the screen: **only 115200 gives clean data**; 9600, 460800 and 921600 give noise. 1768 bytes.
- **Decoded and verified:** 13-byte frames `AA 55 CS 06 F9 16 S R C 00 K F5 5F`: checksum = byte sum of the six payload bytes, length 6 and its complement, type `0x16`, S = press/release, R = row 0..5, C = column 0..2, K = a USB HID keyboard usage code per key. All 136 frames valid, 0 bytes skipped, all 18 keys seen, press and release alternate perfectly. Full description in `docs/uart1-key-protocol.md`.
- **Meaning:** the SK-18 reports **keys**, not coordinates; no kernel touchscreen driver is needed. The same controller is the USB HID keyboard seen on the host (`4250:426f`; Inferred from the matching key codes). This closes the question from D-005 (`/dev/ttyS1` was the right guess).
- **Still open:** commands from the SoC to the controller (if any), other message types, and the stock host-port interrupt storm. UART1's DTS node stays; the key reader belongs in the on-device client.
- **Next:** key reader in the client (Rust), mapped through the existing `Orientation` model; then Wi-Fi.

## D-024 The key reader is a Buildroot package, built with the image's toolchain (2026-11-08)
- **Done:** `client/sk18-keys` (Rust; decoder, orientation mapping, CLI; 21 tests incl. the real capture; clippy clean on the host and for `armv7-unknown-linux-gnueabihf`) is packaged as `br2-external/package/sk18-keys` and enabled in the defconfig. Buildroot uses its prebuilt Rust compiler (`host-rust-bin` 1.99.1; no from-source compiler build). The binary is a stripped 343 KB ARMv7 hard-float PIE linking libc and libgcc_s, installed as `/usr/bin/sk18-keys`. Image `c4628b30…`.
- **Packaging gotcha:** Buildroot vendors a Rust package's dependencies in the *download* step, which a local-source package (`SITE_METHOD = local`) never goes through, so the first build failed offline with "no matching package named rustix". The `.mk` now runs the same `cargo vendor --locked` command in a post-rsync hook. Consequence: the first build needs network access to crates.io (cached in `dl/br-cargo-home` afterwards); versions and checksums are pinned by the committed `Cargo.lock`.
- **Host-side builds:** keep `cargo` output out of `client/` (`CARGO_TARGET_DIR`), because the local-source rsync copies everything under `client/`.
- **Caught by the cross-check:** a borrow error in the Unix-only serial path, invisible to the Windows build; found by `cargo check --target armv7-unknown-linux-gnueabihf` before it reached the board.
- **Not yet verified on the board:** that the binary runs there and prints the events for real key presses.
