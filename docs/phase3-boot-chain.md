# Phase 3: boot-chain decision

Status labels: **Confirmed** (read from the image or from the pinned sources), **Inferred**, **Unknown**.

## Facts

- **Original chain (Confirmed):** the T113 boot ROM loads the vendor `boot0` (`eGON.BT0`) from 8 KiB of
  the SD card. `boot0` sets up DRAM and loads a vendor `sunxi-package` (U-Boot, OP-TEE, the board DTB)
  from byte 16,793,600. U-Boot reads `bootcmd=run setargs_mmc boot_normal` from a raw `env` partition,
  loads the Android boot image from the `boot` partition to `0x41000000`, and boots vendor Linux 5.4.61
  with the vendor DTB (`docs/phase2-image-analysis.md`).
- **DRAM (Confirmed):** the `boot0` header carries the DRAM parameters (decoded from the image): DDR3,
  792 MHz, ZQ `0x007b7bfb`, ODT off, `mr0=0x1c70`, `mr1=0x42`, `mr2=0x18`, `TPR0=0x004a2195`,
  `TPR11=0x00340000`, `TPR12=0x46`, `TPR13=0x34000100`, boot0 debug UART port 3.
- **Mainline U-Boot 2026.10 (Confirmed from the pinned sources):** `configs/mangopi_mq_r_defconfig`
  (T113-S3) sets `DRAM_CLK=792`, `DRAM_ZQ=8092667`, `DRAM_SUNXI_ODT_EN=0`, `TPR0=0x004a2195`,
  `TPR11=0x340000`, `TPR12=0x46`, `TPR13=0x34000100`, `CONS_INDEX=4` (UART3). These equal the SK-18's
  vendor values. The vendor header also has `TPR1=0x02423190`, `TPR2=0x0008b061`, `TPR3=0xb4787896`,
  `TPR6=0x48`, `para1=0x10d2`; the MQ-R defconfig leaves them at the Kconfig defaults (0 for TPR1 to 3)
  and boots with them, so the SK-18 should too.
- **Mainline Linux 7.3.1 (Confirmed):** `sun8i-t113s.dtsi` and the shared MQ-R board file exist. The
  display pipeline (DE2 mixer, TCON, MIPI-DSI, D-PHY) is described in the shared dtsi; an earlier version
  of this note said DSI was missing, which was wrong (see `docs/phase5-display.md`).
- **Not known:** where the UART3 pins (PB6/PB7) are exposed on the board, the DDR3 chip part number, the
  panel interface (the original DTB says interface type 4 with 2 lanes; probably DSI).

## Options

| Option | What it is | For | Against |
|---|---|---|---|
| A. Keep vendor boot0 + vendor U-Boot, replace kernel and rootfs | Mainline kernel in an Android boot image; DTB appended to the kernel (`ARM_APPENDED_DTB`) because the vendor U-Boot always passes the vendor DTB | Smallest change; DRAM known good | Vendor U-Boot fixes up its own DTB, not ours; kernel command line comes from the vendor environment; depends on an unmaintained vendor U-Boot |
| B. Mainline SPL and U-Boot | `u-boot-sunxi-with-spl.bin` at 8 KiB, extlinux on the root file system | Fully reproducible from upstream; one maintained code base; standard distro boot; our own DTB with no tricks | If the SPL fails, there is no output unless the UART3 header is found |
| C. Hybrid | Vendor boot0, mainline U-Boot proper | Keeps vendor DRAM init | Needs a vendor-format package; no upstream support |
| D. Stay on vendor until mainline is validated | Vendor everything | Safe | Does not meet the goal |

## Decision

**B (mainline SPL + U-Boot) is the target and is tried first**, with **A as the fallback** if the SPL
cannot be made to boot.

Why B first:
- The DRAM parameters are the same as a known-working mainline board, so the most dangerous step
  (DRAM bring-up) uses values that have been checked against the real device.
- Recovery is trivial and risk-free: the SD card holds all boot code, the original card is backed up,
  and nothing on the board's internal storage is touched (the board boots from SD).
- The missing UART header is mitigated by a second console: the kernel is built with a USB serial
  gadget (`ttyGS0`), which appears as `/dev/ttyACM*` on the host the SK-18 is plugged into, so a kernel
  that boots can be reached without opening the case.

Known limitation: a failure before the kernel starts (SPL, U-Boot, early kernel) gives no output until
a UART is attached. If the first image does not come up, the next step is finding the UART3 header
(PB6/PB7) on the board, not changing the design.

## Layout of the SD image (`board/waveshare/sk18/genimage.cfg`)

| Offset | Content |
|---|---|
| 8 KiB | `u-boot-sunxi-with-spl.bin` (SPL, then U-Boot proper at 40 KiB) |
| 1008 KiB | GPT |
| 1 MiB | one ext4 partition: root file system with `/boot/zImage`, `/boot/sun8i-t113s-waveshare-sk18.dtb`, `/boot/extlinux/extlinux.conf` |

U-Boot has no persistent environment (default environment in RAM), so there is nothing to corrupt.
Kernel command line: `console=ttyS3,115200 console=ttyGS0,115200 earlycon root=PARTUUID=... rootwait`.
Load addresses are the mainline sunxi defaults handled by distro boot.

## Test plan

1. SPL and U-Boot start (visible only with a UART; otherwise inferred from step 2).
2. Linux starts and mounts the root file system: a `/dev/ttyACM*` appears on the host with a login prompt.
3. USB host port: the Wi-Fi adapter (`0bda:1a2b`, then `0bda:b711` after the mode switch) enumerates.
4. Everything after that follows `docs/hardware-status.md` (to be created with the first booting image).

## Rollback

The original SD card (and the byte-exact image in `image/`) boots the original system. A new card is
written only after the target device is identified by serial number (D-007).
