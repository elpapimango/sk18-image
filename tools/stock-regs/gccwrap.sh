#!/bin/bash
# Compiler wrapper for building sk18_regs.ko: drops the options of the kernel's per-task stack protector GCC
# plugin (not loaded here, see Makefile) and runs the vendor cross compiler.
args=()
for a in "$@"; do
	case "$a" in
	-fplugin-arg-arm_ssp_per_task_plugin-*) ;;
	*) args+=("$a") ;;
	esac
done
exec arm-openwrt-linux-gnueabi-gcc "${args[@]}"
