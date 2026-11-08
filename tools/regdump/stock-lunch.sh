#!/bin/sh
# TEMPORARY debugging hook for the stock SK-18 firmware. Not part of the product.
#
# The stock rootfs (/etc/init.d/qt_app1) copies /mnt/SDCARD/lunch.sh (the FAT partition of the SD card)
# to /data/lunch.sh and runs it as root at boot. This script dumps the display registers and clock/pin
# state into files on that FAT partition so they can be compared with the mainline kernel.
# It needs a static `regdump` binary (tools/regdump/regdump.c) next to it on the FAT partition.
# Remove lunch.sh and regdump again afterwards: the next boot then behaves as before.
D=/mnt/SDCARD
R=/tmp/regdump

[ -f "$D/regdump" ] || exit 0
cp "$D/regdump" "$R" && chmod +x "$R" || exit 0

dump() {
	out="$1"
	{
		echo "# $(date) uptime: $(cat /proc/uptime)"
		echo "## DSI 0x05450000"; "$R" 0x05450000 0x1000
		echo "## D-PHY 0x05451000"; "$R" 0x05451000 0x1000
		echo "## TCON TOP 0x05460000"; "$R" 0x05460000 0x100
		echo "## TCON LCD0 0x05461000"; "$R" 0x05461000 0x1000
		echo "## DE top 0x05000000"; "$R" 0x05000000 0x100
		echo "## mixer0 global 0x05100000"; "$R" 0x05100000 0x100
		echo "## mixer0 blender 0x05101000"; "$R" 0x05101000 0x100
		echo "## mixer0 overlay 0x05103000"; "$R" 0x05103000 0x100
		echo "## CCU 0x02001000"; "$R" 0x02001000 0x1000
		echo "## PIO 0x02000000 (B..G config/data/drive/pull)"; "$R" 0x02000000 0x400
	} > "$out" 2>&1
}

dump "$D/regdump_a.txt"
sync

# Later, when the application has started, again plus the kernel's own view.
(
	sleep 40
	dump "$D/regdump_b.txt"
	dmesg > "$D/dmesg_stock.txt" 2>&1
	cat /proc/cmdline > "$D/cmdline_stock.txt" 2>&1
	mount -t debugfs none /sys/kernel/debug 2>/dev/null
	cat /sys/kernel/debug/clk/clk_summary > "$D/clk_summary_stock.txt" 2>&1
	cat /sys/kernel/debug/pinctrl/*/pinmux-pins > "$D/pinmux_stock.txt" 2>&1
	cat /sys/kernel/debug/gpio > "$D/gpio_stock.txt" 2>&1
	ls /sys/class/disp /sys/class/graphics > "$D/disp_class_stock.txt" 2>&1
	sync
) &
exit 0
