# Waveshare SK-18 board support (Buildroot external tree)

Mainline U-Boot and Linux for the Waveshare SK-18 (Allwinner T113-S3). The reasoning is in
`docs/phase3-boot-chain.md`; the hardware facts and their sources are in
`docs/phase2-image-analysis.md`.

## Pinned versions

| Component | Version | Where it is pinned |
|---|---|---|
| Buildroot | commit `60a3423e958dc4266565c6bdfbb619dd95839314` (2026-11-06, reports 2026.11-git) | this README; use that commit |
| Linux | 7.3.1 | `configs/waveshare_sk18_defconfig` (hash in Buildroot) |
| U-Boot | 2026.10 | `configs/waveshare_sk18_defconfig` (hash in Buildroot) |
| Toolchain | Bootlin ARMv7 hard-float glibc, stable | `configs/waveshare_sk18_defconfig` |

`BR2_DOWNLOAD_FORCE_CHECK_HASHES=y`: a package without a matching hash fails the build.

## Build

Run in WSL (or any Linux) with a PATH that has no spaces (Buildroot refuses one that does):

```sh
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
cd ~/buildroot                      # at the pinned commit
make O=$HOME/sk18-build BR2_EXTERNAL=/path/to/sk18/br2-external waveshare_sk18_defconfig
make O=$HOME/sk18-build -j"$(nproc)"
```

Result: `$HOME/sk18-build/images/sdcard.img` (U-Boot with SPL at 8 KiB, GPT, one ext4 root file system).
Intermediate commands: `make O=... linux-menuconfig`, `uboot-menuconfig`, `savedefconfig`,
`linux-update-defconfig`, `uboot-update-defconfig`.

## Boot and console

- UART3 (PB6/PB7), 115200 8N1, is the console of the SPL, U-Boot and Linux (`ttyS3`). Where the pins
  are on the board is not yet known.
- The kernel also has a USB serial gadget: with the SK-18 connected to a host, `/dev/ttyACM*` shows a
  login prompt (`ttyGS0`). The root account has an empty password in this bring-up image.

## Flashing

Never write to a device you have not identified. Use the procedure in `docs/decision-log.md` (D-007):
check `lsblk -o NAME,SIZE,MODEL,SERIAL,TRAN,RM,MOUNTPOINTS`, require the card's serial number and size
and that nothing is mounted, then write with `dd` to that exact device and verify by reading back.

## Files

| File | Purpose |
|---|---|
| `configs/waveshare_sk18_defconfig` | Buildroot configuration |
| `board/waveshare/sk18/linux.fragment` | kernel options on top of `sunxi_defconfig` |
| `board/waveshare/sk18/uboot.fragment` | U-Boot options on top of `mangopi_mq_r_defconfig` |
| `board/waveshare/sk18/sun8i-t113s-waveshare-sk18.dts` | board device tree |
| `board/waveshare/sk18/genimage.cfg`, `extlinux.conf`, `post-build.sh` | image layout and boot configuration |
