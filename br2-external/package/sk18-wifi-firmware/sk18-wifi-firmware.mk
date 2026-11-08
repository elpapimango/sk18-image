################################################################################
#
# sk18-wifi-firmware
#
################################################################################

# Same release as package/linux-firmware, but only the files the SK-18 needs.
SK18_WIFI_FIRMWARE_VERSION = 20261019
SK18_WIFI_FIRMWARE_RAW = https://gitlab.com/kernel-firmware/linux-firmware/-/raw/$(SK18_WIFI_FIRMWARE_VERSION)
SK18_WIFI_FIRMWARE_SITE = $(SK18_WIFI_FIRMWARE_RAW)/rtlwifi
SK18_WIFI_FIRMWARE_SOURCE = rtl8710bufw_SMIC.bin
SK18_WIFI_FIRMWARE_EXTRA_DOWNLOADS = \
	$(SK18_WIFI_FIRMWARE_SITE)/rtl8710bufw_UMC.bin \
	$(SK18_WIFI_FIRMWARE_RAW)/LICENSES/LICENCE.rtlwifi_firmware.txt
SK18_WIFI_FIRMWARE_LICENSE = Redistributable, see LICENCE.rtlwifi_firmware.txt
SK18_WIFI_FIRMWARE_LICENSE_FILES = LICENCE.rtlwifi_firmware.txt
SK18_WIFI_FIRMWARE_REDISTRIBUTE = YES

# The downloads are plain files, not archives: copy them into the build directory.
define SK18_WIFI_FIRMWARE_EXTRACT_CMDS
	cp $(SK18_WIFI_FIRMWARE_DL_DIR)/rtl8710bufw_SMIC.bin \
		$(SK18_WIFI_FIRMWARE_DL_DIR)/rtl8710bufw_UMC.bin \
		$(SK18_WIFI_FIRMWARE_DL_DIR)/LICENCE.rtlwifi_firmware.txt $(@D)/
endef

define SK18_WIFI_FIRMWARE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0644 $(@D)/rtl8710bufw_SMIC.bin \
		$(TARGET_DIR)/lib/firmware/rtlwifi/rtl8710bufw_SMIC.bin
	$(INSTALL) -D -m 0644 $(@D)/rtl8710bufw_UMC.bin \
		$(TARGET_DIR)/lib/firmware/rtlwifi/rtl8710bufw_UMC.bin
	$(INSTALL) -D -m 0644 $(@D)/LICENCE.rtlwifi_firmware.txt \
		$(TARGET_DIR)/usr/share/doc/sk18-wifi-firmware/LICENCE.rtlwifi_firmware.txt
endef

$(eval $(generic-package))
