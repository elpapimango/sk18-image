#!/bin/bash
# Writes /tmp/sk18-sdcard.img to the SD card in the DietPi machine's reader, but only if
#  - the image has the expected hash,
#  - the device is the 4 GB USB card reader, unmounted, not the system disk, and
#  - the card is NOT a stock-firmware card: it must be blank/factory FAT, or hold one of our earlier mainline images.
set -u
IMG=/tmp/sk18-sdcard.img
WANT_SHA=${1:?usage: flash2.sh <sha256 of /tmp/sk18-sdcard.img>}
WANT_SERIAL=3063422494200319   # the card reader's serial, NOT the card's
WANT_BYTES=4026531840
# PARTUUIDs of the ext4 partitions of the mainline images flashed so far
KNOWN="0915bee9-7461-4b03-9e9d-3e6d87e23276 ba26212a-aed0-4795-b8a4-f010ac5120fb 294d0c07-a840-4b9a-8280-1a8731c64ef2 540e6ce1-a6a0-4e45-94a0-41b16c636a18 4e50dc54-8778-4535-96c8-ae4cbe10204f ca28bcff-985f-428f-a81a-233d780e0a99"

die() { echo "ABORT: $*"; exit 1; }

GOT_SHA=$(sha256sum "$IMG" | cut -d' ' -f1)
[ "$GOT_SHA" = "$WANT_SHA" ] || die "image hash $GOT_SHA != $WANT_SHA"
echo "image ok: $GOT_SHA"
IMG_BYTES=$(stat -c %s "$IMG")

DEV=""
for d in /sys/block/sd*; do
  n=$(basename "$d")
  s=$(udevadm info -q property -n "/dev/$n" 2>/dev/null | sed -n 's/^ID_SERIAL_SHORT=//p')
  [ "$s" = "$WANT_SERIAL" ] && DEV="/dev/$n"
done
[ -n "$DEV" ] || die "no reader with serial $WANT_SERIAL"

lsblk -o NAME,SIZE,TRAN,RM,FSTYPE,PARTLABEL,PARTUUID,MOUNTPOINTS "$DEV"
SIZE=$(sudo blockdev --getsize64 "$DEV")
TRAN=$(lsblk -dno TRAN "$DEV")
RM=$(lsblk -dno RM "$DEV")
MNT=$(lsblk -no MOUNTPOINTS "$DEV" | tr -d ' \n')
ROOTDISK=$(lsblk -no PKNAME "$(findmnt -no SOURCE /)" 2>/dev/null)
[ "$SIZE" = "$WANT_BYTES" ] || die "size $SIZE != $WANT_BYTES (no card in the reader?)"
[ "$TRAN" = "usb" ] && [ "$RM" = "1" ] || die "not a removable USB disk"
[ -z "$MNT" ] || die "something is mounted: $MNT"
[ "/dev/$ROOTDISK" != "$DEV" ] || die "this is the system disk"
[ "$IMG_BYTES" -le "$SIZE" ] || die "image larger than the card"

# what is on the card now?
LABELS=$(lsblk -no PARTLABEL "$DEV" | tr -d ' ' | tr '\n' ',')
UUIDS=$(lsblk -no PARTUUID "$DEV" | tr '\n' ' ')
echo "partition labels on the card: ${LABELS:-none}"
case ",$LABELS" in
  # "rootfs" alone is not a marker: the mainline image names its partition that too.
  *,boot-resource,*|*,UDISK,*|*,env,*|*,env-redund,*) die "this card carries a STOCK firmware layout (boot-resource/env/UDISK): refusing" ;;
esac
KIND="blank or factory-formatted"
for u in $UUIDS; do for k in $KNOWN; do [ "$u" = "$k" ] && KIND="one of our earlier mainline images ($u)"; done; done
# anything with partitions that is neither factory FAT nor ours: stop
NPART=$(lsblk -no TYPE "$DEV" | grep -c part)
if [ "$KIND" = "blank or factory-formatted" ] && [ "$NPART" -gt 1 ]; then die "unknown content: $NPART partitions, none of them ours"; fi
echo "card content: $KIND"

echo "=== writing $IMG_BYTES bytes to $DEV"
sudo dd if="$IMG" of="$DEV" bs=1M conv=fsync status=progress 2>&1 | tail -2
sync; sudo blockdev --flushbufs "$DEV"
READ_SHA=$(sudo dd if="$DEV" bs=1M iflag=direct count=$(( (IMG_BYTES + 1048575) / 1048576 )) status=none | head -c "$IMG_BYTES" | sha256sum | cut -d' ' -f1)
echo "image : $WANT_SHA"
echo "card  : $READ_SHA"
[ "$READ_SHA" = "$WANT_SHA" ] && echo "VERIFIED: the card matches the image" || die "READ-BACK MISMATCH"
sudo partprobe "$DEV" 2>/dev/null; sleep 1
lsblk -o NAME,SIZE,FSTYPE,PARTUUID "$DEV"
