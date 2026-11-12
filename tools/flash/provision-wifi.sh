#!/bin/bash
# Writes the Wi-Fi credentials onto the SD card of the SK-18 (mainline image) in the DietPi machine's card reader.
#
# The credentials are never part of the image or of this repository: they go to /etc/wpa_supplicant.conf on the
# card only, mode 0600. The passphrase is read without echo and is turned into the 256-bit WPA key (PBKDF2, 4096
# rounds, as wpa_passphrase does), so the plain passphrase is not stored. The SSID is written as hex so that any
# characters are safe.
#
# Run it yourself in a terminal on the DietPi machine (it asks for the SSID and passphrase):
#     bash provision-wifi.sh
# It only touches a card that
#  - is the 4 GB USB card reader, unmounted, not the system disk,
#  - has no stock-firmware partition labels (boot-resource, env, UDISK), and
#  - holds this project's mainline image with the Wi-Fi firmware (checked read-only first).
set -euo pipefail

WANT_SERIAL=3063422494200319   # the card reader's serial, NOT the card's
WANT_BYTES=4026531840
MNT=/mnt/sk18p1

die() { echo "ABORT: $*" >&2; exit 1; }

DEV=""
for d in /sys/block/sd*; do
	[ -e "$d" ] || continue
	n=$(basename "$d")
	s=$(udevadm info -q property -n "/dev/$n" 2>/dev/null | sed -n 's/^ID_SERIAL_SHORT=//p')
	[ "$s" = "$WANT_SERIAL" ] && DEV="/dev/$n"
done
[ -n "$DEV" ] || die "no card reader with serial $WANT_SERIAL"
PART="${DEV}1"

SIZE=$(sudo blockdev --getsize64 "$DEV")
TRAN=$(lsblk -dno TRAN "$DEV")
RM=$(lsblk -dno RM "$DEV")
MNTS=$(lsblk -no MOUNTPOINTS "$DEV" | tr -d ' \n')
ROOTDISK=$(lsblk -no PKNAME "$(findmnt -no SOURCE /)" 2>/dev/null || true)
[ "$SIZE" = "$WANT_BYTES" ] || die "size $SIZE != $WANT_BYTES (no card in the reader?)"
if [ "$TRAN" != "usb" ] || [ "$RM" != "1" ]; then die "not a removable USB disk"; fi
[ -z "$MNTS" ] || die "something is mounted: $MNTS"
[ "/dev/$ROOTDISK" != "$DEV" ] || die "this is the system disk"

LABELS=$(lsblk -no PARTLABEL "$DEV" | tr -d ' ' | tr '\n' ',')
case ",$LABELS" in
	*,boot-resource,*|*,UDISK,*|*,env,*|*,env-redund,*) die "this card carries a STOCK firmware layout: refusing" ;;
esac
[ "$(lsblk -no FSTYPE "$PART" | head -1)" = "ext4" ] || die "$PART is not ext4: not this project's image"

sudo mkdir -p "$MNT"
sudo mount -t ext4 -o ro "$PART" "$MNT" || die "cannot mount $PART"
ok=1
sudo test -f "$MNT/boot/extlinux/extlinux.conf" || ok=0
sudo test -f "$MNT/lib/firmware/rtlwifi/rtl8710bufw_SMIC.bin" || ok=0
sudo test -x "$MNT/usr/sbin/wpa_supplicant" || ok=0
sudo umount "$MNT"
[ "$ok" = 1 ] || die "the card does not hold this project's Wi-Fi image (extlinux, RTL8710BU firmware, wpa_supplicant)"

echo "Card: $DEV ($(lsblk -dno SIZE "$DEV")), partition $PART looks like the SK-18 mainline image with Wi-Fi support."
read -rp "Wi-Fi network name (SSID): " SSID
[ -n "$SSID" ] || die "empty SSID"
read -rp "Two-letter country code for the radio rules (for example US, DE, GB): " COUNTRY
[[ "$COUNTRY" =~ ^[A-Za-z]{2}$ ]] || die "the country code must be two letters"
COUNTRY=${COUNTRY^^}
read -rp "Is the network hidden? [y/N]: " HIDDEN
read -rsp "Passphrase (8 to 63 characters, not shown): " PASS; echo
read -rsp "Passphrase again: " PASS2; echo
[ "$PASS" = "$PASS2" ] || die "the passphrases differ"
if [ "${#PASS}" -lt 8 ] || [ "${#PASS}" -gt 63 ]; then die "a WPA passphrase has 8 to 63 characters"; fi
unset PASS2

TMP=$(mktemp)
trap 'rm -f "$TMP"; unset PASS' EXIT
chmod 600 "$TMP"
# The passphrase goes to python on stdin (not on a command line, where other users could see it).
printf '%s' "$PASS" | SSID="$SSID" COUNTRY="$COUNTRY" HIDDEN="$HIDDEN" python3 -c '
import hashlib, os, sys
ssid = os.environ["SSID"].encode()
psk = hashlib.pbkdf2_hmac("sha1", sys.stdin.buffer.read(), ssid, 4096, 32).hex()
lines = ["ctrl_interface=/run/wpa_supplicant", "update_config=0", "country=" + os.environ["COUNTRY"], "network={",
         "\tssid=" + ssid.hex(), "\tkey_mgmt=WPA-PSK", "\tpsk=" + psk]
if os.environ["HIDDEN"].lower().startswith("y"):
    lines.append("\tscan_ssid=1")
lines.append("}")
print("\n".join(lines))
' > "$TMP"

sudo mount -t ext4 -o rw "$PART" "$MNT" || die "cannot mount $PART read-write"
sudo install -m 0600 -o root -g root "$TMP" "$MNT/etc/wpa_supplicant.conf"
sync
echo "written: $(sudo ls -l "$MNT/etc/wpa_supplicant.conf" | awk '{print $1, $3, $4, $5 " bytes"}')"
sudo umount "$MNT"
echo "Done. The key (not the passphrase) is stored on the card only; nothing was written anywhere else."
