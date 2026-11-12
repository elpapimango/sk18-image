#!/bin/sh
# TEMPORARY debugging hook for the stock SK-18 firmware. Not part of the product.
#
# Same mechanism as stock-lunch.sh (the stock rootfs copies /mnt/SDCARD/lunch.sh and runs it as root at boot),
# but for the touch controller: loads the read-only module sk18_regs.ko and, for four minutes, logs once a
# second the TPADC control/status registers (/proc/sk18_tp) and the touch interrupt count, so that they can be
# compared with the mainline driver while a finger is on the screen. Also saves the input devices and clocks.
# Remove lunch.sh and sk18_regs.ko from the card afterwards: the next boot then behaves as before.
D=/mnt/SDCARD

[ -f "$D/sk18_regs.ko" ] || exit 0
cp "$D/sk18_regs.ko" /tmp/sk18_regs.ko || exit 0
insmod /tmp/sk18_regs.ko > "$D/insmod_result.txt" 2>&1 || exit 0

(
	O=/tmp/tp_log.txt
	: > "$O"
	cat /proc/bus/input/devices > "$D/tp_input_devices.txt" 2>&1
	cat /proc/interrupts > "$D/tp_interrupts_start.txt" 2>&1
	i=0
	while [ $i -lt 240 ]; do
		echo "t=$i $(cat /proc/sk18_tp) irq=$(grep -E 'sunxi-ts|tpadc|rtp|sunxi_ts' /proc/interrupts | tr -s ' ' | tr '\n' ';')" >> "$O"
		i=$((i + 1))
		sleep 1
		if [ $((i % 30)) -eq 0 ]; then
			cp "$O" "$D/tp_log.txt"
			sync
		fi
	done
	cp "$O" "$D/tp_log.txt"
	cat /proc/interrupts > "$D/tp_interrupts_end.txt" 2>&1
	ls -l /dev/input/ > "$D/tp_dev_input.txt" 2>&1
	grep -i -E 'tpadc|rtp|ts' /sys/kernel/debug/clk/clk_summary > "$D/tp_clk.txt" 2>&1
	dmesg > "$D/tp_dmesg.txt" 2>&1
	sync
) &
exit 0
