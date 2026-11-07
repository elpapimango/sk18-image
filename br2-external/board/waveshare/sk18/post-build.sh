#!/bin/sh
# Post-build: generate extlinux.conf and genimage.cfg for this build (one fresh PARTUUID each time).
# Arguments: $1 = TARGET_DIR (set by Buildroot). Environment: BR2_CONFIG, BINARIES_DIR, HOST_DIR,
# BR2_EXTERNAL_SK18_PATH.
set -eu

BOARD="${BR2_EXTERNAL_SK18_PATH}/board/waveshare/sk18"

linux_image() {
	if grep -Eq '^BR2_LINUX_KERNEL_UIMAGE=y$' "${BR2_CONFIG}"; then
		echo "uImage"
	elif grep -Eq '^BR2_LINUX_KERNEL_IMAGE=y$' "${BR2_CONFIG}"; then
		echo "Image"
	elif grep -Eq '^BR2_LINUX_KERNEL_IMAGEGZ=y$' "${BR2_CONFIG}"; then
		echo "Image.gz"
	else
		echo "zImage"
	fi
}

PARTUUID="$("${HOST_DIR}/bin/uuidgen")"

install -d "${TARGET_DIR}/boot/extlinux/"

sed -e "s/%LINUXIMAGE%/$(linux_image)/g" \
	-e "s/%PARTUUID%/${PARTUUID}/g" \
	"${BOARD}/extlinux.conf" > "${TARGET_DIR}/boot/extlinux/extlinux.conf"

sed "s/%PARTUUID%/${PARTUUID}/g" "${BOARD}/genimage.cfg" > "${BINARIES_DIR}/genimage.cfg"

# A second login on the USB serial gadget (ttyGS0) next to the UART console.
if ! grep -q '^ttyGS0::' "${TARGET_DIR}/etc/inittab"; then
	echo 'ttyGS0::respawn:/sbin/getty -L ttyGS0 115200 vt100 # USB gadget console' >> "${TARGET_DIR}/etc/inittab"
fi
