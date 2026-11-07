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

## D-004 Open question: where does the OpenDeck client run
- **Observed:** the stock firmware makes the SK-18 a USB CDC-ACM peripheral controlled by a PC application; it has no OpenDeck client.
- **Open:** run the OpenDeck client on the device (needs Wi-Fi and display/input working), or bridge from a host. Pending the user's decision before Phase 8.
