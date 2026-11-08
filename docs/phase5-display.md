# Display investigation (SK-18 screen, 720x1280)

Status labels: **Confirmed** (read from the original DTB, the vendor kernel source or the pinned Linux 7.3.1 source),
**Inferred**, **Unknown**.

## What the original system does (Confirmed)

Source: the board DTB node `lcd0@5461000` (`docs/phase2-image-analysis.md`) and the vendor kernel in
`~/Tina-Linux/lichee/linux-5.4/drivers/video/fbdev/sunxi/disp2`.

| Item | Value | Source |
|---|---|---|
| Interface | `lcd_if = 4` = `LCD_IF_DSI`: **MIPI-DSI**, video mode (`lcd_dsi_if = 0`), **2 lanes** (`lcd_dsi_lane = 2`) | vendor `include.h` enum, DTB |
| Pins | `dsi4lane` group, function `dsi`, PD0 to PD9 (the 4-lane pin group; the controller uses 2 lanes) | DTB pinctrl |
| Resolution / timing | 720x1280, pixel clock 70 MHz, htotal 900, hbp 100 (includes hsync 20), vtotal 1312, vbp 16 (includes vsync 4); about 59 Hz | DTB |
| Derived Linux timing | hdisplay 720, hsync_start 800, hsync_end 820, htotal 900; vdisplay 1280, vsync_start 1296, vsync_end 1300, vtotal 1312; clock 70000 kHz | computed from the above (front porch = total - active - back porch: 80 and 16) |
| Panel driver | `default_lcd` (`default_panel.c`); its `LCD_panel_init()` and `LCD_panel_exit()` are **empty**: the host sends **no DSI init commands** | vendor source |
| Power-on sequence | power, 30 ms; panel init (nothing), 50 ms; TCON enable, 100 ms; backlight on | vendor source |
| Panel GPIO | `lcd_gpio_0` = PE5, driven high at probe; never toggled by `default_panel.c` | DTB, vendor source (role Unknown: panel enable, reset or backlight enable) |
| Backlight | PWM channel 3 on **PG10**, 1 kHz, normal polarity, 0 to 255, default 50 | DTB |
| Controller addresses | DSI 0x05450000, TCON top 0x05460000, TCON LCD0 0x05461000 | DTB |

## What mainline Linux 7.3.1 provides (Confirmed)

I first reported that mainline had no T113 DSI support. **That was wrong**: the shared SoC file
(`arch/riscv/boot/dts/allwinner/sunxi-d1s-t113.dtsi`, included by `sun8i-t113s.dtsi`) already describes
`dsi@5450000` (`allwinner,sun20i-d1-mipi-dsi`, falling back to `allwinner,sun50i-a100-mipi-dsi`),
`phy@5451000` (D-PHY), `tcon-top@5460000`, `lcd-controller@5461000` and the display-engine mixers, at the same
addresses as the vendor DTB. The DSI driver matches the A100 fallback, which is why searching it for "d1"
found nothing. The ports are wired in the dtsi (`tcon_lcd0_out_dsi` to `dsi_in_tcon_lcd0`); the nodes are
`status = "disabled"`, so the board file enables them.

| Piece | Mainline state |
|---|---|
| DE2 mixer, TCON top, TCON LCD (D1 quirks), DSI, D-PHY | present; `DRM_SUN6I_DSI`, `DRM_SUN8I_MIXER`, `DRM_SUN8I_TCON_TOP` default on with `DRM_SUN4I=y` (already in `sunxi_defconfig`) |
| The panel | not in the kernel. `panel-simple` handles DSI panels through a table (`panel_desc_dsi`: mode, lanes, format, flags); a new entry is needed (small patch) |
| Backlight PWM | **no driver for the T113 PWM block**: the tree has only `pwm-sun4i` (A10 register layout) and no PWM node in the T113 dtsi. `pwm-gpio` (software PWM, hrtimer) and `gpio-backlight` exist |
| Touch (Goodix) | separate step |

## Plan

1. **Kernel patch** (`patches/linux/`): add `waveshare,sk18-panel` to `panel-simple.c` as a DSI panel with the
   mode above, 2 lanes, RGB888, `MIPI_DSI_MODE_VIDEO | MIPI_DSI_MODE_VIDEO_SYNC_PULSE`. Sync pulse follows from the
   non-zero hsync/vsync widths; if there is no picture, `VIDEO_BURST` is the first variant to try.
2. **Device tree**: enable `de`, `mixer0`, `tcon_top`, `tcon_lcd0`, `dsi`, `dphy`; panel child node under `&dsi`
   (`reg = <0>`, `enable-gpios` PE5, `power-supply` 3.3 V assumed, `backlight`); backlight as `pwm-backlight` on
   a `pwm-gpio` provider on PG10 (interim, software PWM at 1 kHz), later replaced by a real T113 PWM driver.
3. **Kernel config**: `DRM_FBDEV_EMULATION`, `FRAMEBUFFER_CONSOLE`, `PWM_GPIO`, `BACKLIGHT_PWM`, `LOGO` optional.
4. **Test**: the kernel log shows the DRM probe; the screen should show the console (fbcon) and the text of the boot.
   The SK-18 screen is the only judge of "picture", so the user looks at it.

## Unknowns that can make the first attempt fail

- Whether the panel needs DSI init commands that the vendor code sends somewhere I did not find (the vendor
  `default_panel.c` sends none, so it should not).
- The role of PE5 (enable, reset or backlight enable).
- DSI mode flags (sync pulse versus burst) and lane clock behaviour (continuous or not).
- The power supply of the panel (assumed always on, 3.3 V rail).

## First test on the board (2026-11-07): no picture, cause found

- **Observed:** the kernel with the display configuration boots (USB serial gadget as before), the screen stays dark,
  and `dmesg` shows `sun6i-mipi-dsi 5450000.dsi: Couldn't get the DSI mod clock` on every probe attempt;
  `devices_deferred` lists only `5450000.dsi`; `/sys/class/drm` has no card. The backlight device exists.
- **Cause (Confirmed in the 7.3.1 source):** on the D1/T113 the DSI "mod" clock is the clock gate provided by the
  TCON top (`CLK_TCON_TOP_DSI`, dtsi). The DSI driver requests it in `probe()`. The TCON top (`sun8i_tcon_top.c`)
  registers its clock gates in its component `bind()`, which runs only after the DRM master has all components,
  including the DSI (registered at the end of its probe). A deadlock; no upstream board enables the D1/T113 DSI.
- **Fix:** `patches/linux/0002-drm-sun4i-tcon-top-register-clocks-at-probe.patch` registers the clocks in `probe()` and
  turns the component callbacks into no-ops. Applies cleanly to 7.3.1.
- **Next possible failures (not yet seen):** D-PHY or DSI register differences on this SoC, panel mode flags
  (sync pulse versus burst), PE5's role.

## Second test on the board (2026-11-07): pipeline up, still no picture, DSI pads not muxed

- **Observed with patch 0002:** the DSI probes and the DRM device appears (`card0`, `card0-DSI-1`, `fb0`); the connector is
  `connected`/`enabled`/`dpms On` with mode 720x1280; the console framebuffer (XR24 720x1280) is on the CRTC;
  `tcon-lcd0` runs at 840 MHz (70 MHz x 24 bit / 2 lanes), the pixel clock at 210 MHz (divider 4), the DSI mod clock at
  150 MHz (the vendor DTB assigns the same 150 MHz); the panel enable GPIO (PE5) is high; the backlight is lit (50 %).
  The screen stays black. Only warning: `panel-simple-dsi: Unbalanced pm_runtime_enable!`.
- **Cause found in the sources:** the shared dtsi defines `dsi_4lane_pins` (PD0 to PD9, function `dsi`, drive strength 30)
  but nothing references it, and my board file did not either. The pads stayed at their reset state.
- **Change:** `pinctrl-0 = <&dsi_4lane_pins>` on `&dsi`. Compiled DTB checked. Not yet run on the board.
- **Still open if it stays black:** the polarity of PE5 (the vendor DTB gives it flag 1, probably active low; I use
  active high), DSI mode flags (burst or sync pulse), DSI init commands the stock kernel may send (the SDK's
  `default_panel.c` has none, but the shipped kernel could differ).
