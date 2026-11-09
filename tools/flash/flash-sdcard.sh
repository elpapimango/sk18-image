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
KNOWN="0915bee9-7461-4b03-9e9d-3e6d87e23276 ba26212a-aed0-4795-b8a4-f010ac5120fb 294d0c07-a840-4b9a-8280-1a8731c64ef2 540e6ce1-a6a0-4e45-94a0-41b16c636a18 4e50dc54-8778-4535-96c8-ae4cbe10204f ca28bcff-985f-428f-a81a-233d780e0a99 9c370377-2e0c-4b6d-b228-bb82f205314d 00887ae7-2a62-487f-bfef-9a0ec0642128 3600f3f1-eb36-4c79-bff0-89dcf13dec5d f3c07547-22b6-4d77-a220-8f0019cf6317 66802615-3ab9-41e7-aa0f-fb57ff8c1328 c3cd6cf4-b65d-41a5-b5ad-df37e51cb4d6 64d1be8f-4545-4c01-98c1-2e1f6818a712 41310905-4a26-4aa6-be3f-434021805ccc d4b5c067-407c-446a-a361-d0d629a22535 175da818-a5bb-4e5b-accd-75f34c717900 0eb391f4-6b35-41be-9912-87cfe2f68e7b 5ccec91c-d332-4555-a83c-cec5bb57885e f656ba13-9b5e-4318-844a-9d2dc3faec17 0ae319be-81ac-4420-9458-f8619d1bc048 ad73547c-8178-455c-a962-cd00d9c3ed5b b7d2feb5-96b5-457f-a151-31fbf367af55 f3b27a9a-ea5b-431c-a5cf-5db406b7d1c5 668c4254-82e1-4df7-ac54-443a5f4a0571"

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
if [ "$TRAN" != "usb" ] || [ "$RM" != "1" ]; then die "not a removable USB disk"; fi
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

# Wi-Fi credentials already on this card (from provision-wifi.sh) are carried over to the new image, so that
# reflashing does not mean typing the passphrase again. They are held in a private temporary file on this machine
# only for the duration of the flash and removed afterwards.
KEEP=""
trap '[ -z "$KEEP" ] || rm -f "$KEEP"' EXIT
if [ "$KIND" != "blank or factory-formatted" ]; then
  sudo mkdir -p /mnt/sk18p1
  if sudo mount -t ext4 -o ro "${DEV}1" /mnt/sk18p1 2>/dev/null; then
    if sudo grep -q '^network=' /mnt/sk18p1/etc/wpa_supplicant.conf 2>/dev/null; then
      KEEP=$(mktemp); chmod 600 "$KEEP"
      sudo cat /mnt/sk18p1/etc/wpa_supplicant.conf | tee "$KEEP" > /dev/null
      echo "Wi-Fi credentials found on the card: they will be copied to the new image"
    else
      echo "no Wi-Fi credentials on the card (run provision-wifi.sh after flashing)"
    fi
    sudo umount /mnt/sk18p1
  fi
fi

echo "=== writing $IMG_BYTES bytes to $DEV"
sudo dd if="$IMG" of="$DEV" bs=1M conv=fsync status=progress 2>&1 | tail -2
sync; sudo blockdev --flushbufs "$DEV"
READ_SHA=$(sudo dd if="$DEV" bs=1M iflag=direct count=$(( (IMG_BYTES + 1048575) / 1048576 )) status=none | head -c "$IMG_BYTES" | sha256sum | cut -d' ' -f1)
echo "image : $WANT_SHA"
echo "card  : $READ_SHA"
if [ "$READ_SHA" = "$WANT_SHA" ]; then echo "VERIFIED: the card matches the image"; else die "READ-BACK MISMATCH"; fi
sudo partprobe "$DEV" 2>/dev/null; sleep 1
lsblk -o NAME,SIZE,FSTYPE,PARTUUID "$DEV"
if [ -n "$KEEP" ]; then
  # This changes the card after the read-back check above: the check covers the image as written, and the only
  # difference afterwards is this one file.
  sudo mount -t ext4 -o rw "${DEV}1" /mnt/sk18p1 || die "cannot mount the new image to restore the Wi-Fi credentials"
  sudo install -m 0600 -o root -g root "$KEEP" /mnt/sk18p1/etc/wpa_supplicant.conf
  sync
  sudo umount /mnt/sk18p1
  echo "Wi-Fi credentials restored on the new image (mode 0600)"
fi
