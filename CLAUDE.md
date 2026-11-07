# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository state

The goal is a reproducible mainline-based Buildroot SD image for this device (see `docs/phase2-image-analysis.md` and `docs/decision-log.md`). Tracked content is currently documentation only; there is no build system yet. Work was done in WSL distro `BuildRoot` (`~/buildroot`, extracted partitions in `~/sk18-work/parts`); never write to the original image. The untracked `image/` directory (≈4.7 GB of binary artifacts for a Waveshare SK18 "ScreenKey" / MK-series device). There is no README, lint config, or test suite. Do not `git add` `image/` (multi-GB blobs); keep it untracked or use Git LFS.

`image/` contains:
- `waveshare_sk18.img` — 4 GB raw disk image of the device (GPT with protective MBR). Treat as read-only; inspect with REA/binary tools rather than mounting or writing to it.
- `MK Series Upper Computer Open Source_V1.0.zip` — vendor package. Its source is **inside the zip only** (nothing is extracted). Inspect without extracting, e.g. `unzip -l "<zip>"` and `unzip -p "<zip>" "<member>"`. If extraction is needed, extract into the scratchpad, not the repo. Contents:
  - `OpenSourceLicenseDemo_V1.0/` — Qt (qmake) host-side desktop app, plus a prebuilt `OpenSourceLicenseDemo_V1.0.exe`.
  - `OpenSourceLicenseFirware_V1.0/MK10_V1.0.img`, `MK20_V1.0.img` — ~850 MB firmware images for two device variants, and `PhoenixCardv4.2.7.7z.7z` (PhoenixCard, Allwinner SD-card flashing tool).

## Host app (inside the zip): build and run

Qt project with `QT += core gui serialport widgets`, `CONFIG += c++17`. Build from an extracted copy:

```
qmake OpenSourceLicenseDemo.pro && make        # or open the .pro in Qt Creator
```

It uses `QRegExpValidator`/`QRegExp`, so it targets Qt 5 (or Qt 6 with core5compat). No tests.

## Host ↔ device serial protocol (needs `mainwindow.cpp`, `CRC32.cpp`, `rectwidget.cpp` together)

- Transport: USB CDC serial, 115200 8N1, no flow control. `get_ports()` picks the device by USB VID/PID: `1d6b:0104` or `1234:5678`.
- Framing (little-endian, `packet_cmd_ack` builds / `parse_cmd` parses): `A1 A5 5A 5E` header, `u32 id`, `u32 cmd`, `u32 size`, `u32 crc32(size)`, payload, `u32 crc32(payload)`. `parse_cmd` buffers the stream, resyncs on the header, and drops frames whose size-CRC or payload-CRC fail.
- `cmd` values (`CMD_VALUE` in `mainwindow.h`): `100` = JPG data (screen/preview image), `101` = JSON, `102` = end. Control is done via JSON messages with a `method` field (e.g. `setFileCRC`) for file list/download/delete, audio start/stop, volume sliders, and key-matrix state.
- `RectWidget` renders the key/rectangle layout from JSON (`loadFromJson`) and highlights pressed keys (`setKeyPressed`); `CRC32.cpp` is a stepwise `Start_CheckCRC / Calculate_CRC / CheckCRC_Result` API.
- Received frames are dispatched through `MainWindow::_DataProcessor`, which is where most protocol behavior lives.
