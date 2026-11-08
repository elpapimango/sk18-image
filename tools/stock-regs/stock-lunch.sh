#!/bin/sh
# TEMPORARY debugging hook for the stock SK-18 firmware. Not part of the product.
#
# The stock rootfs (/etc/init.d/qt_app1) copies /mnt/SDCARD/lunch.sh (the FAT partition of the SD card)
# to /data/lunch.sh and runs it as root at boot. This script loads tools/stock-regs/sk18_regs.ko (a
# read-only register dump module for the stock 5.4.61 kernel), and writes the display registers into
# files on that FAT partition so they can be compared with the mainline kernel.
# Remove lunch.sh and sk18_regs.ko from the card afterwards: the next boot then behaves as before.
D=/mnt/SDCARD

[ -f "$D/sk18_regs.ko" ] || exit 0
cp "$D/sk18_regs.ko" /tmp/sk18_regs.ko || exit 0
insmod /tmp/sk18_regs.ko > "$D/insmod_result.txt" 2>&1 || exit 0

cat /proc/sk18_regs > "$D/regs_a.txt" 2>&1
sync

# Later, when the application has started, again.
(
	sleep 40
	cat /proc/sk18_regs > "$D/regs_b.txt" 2>&1
	dmesg > "$D/dmesg_stock2.txt" 2>&1
	cat /sys/class/disp/disp/attr/sys > "$D/disp_sys_stock.txt" 2>&1
	sync
) &
exit 0
