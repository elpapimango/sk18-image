################################################################################
#
# sk18-splash
#
################################################################################

SK18_SPLASH_VERSION = 0.1.0
# Built from the sources in this repository (client/), not downloaded.
SK18_SPLASH_SITE = $(BR2_EXTERNAL_SK18_PATH)/../client
SK18_SPLASH_SITE_METHOD = local
SK18_SPLASH_SUBDIR = sk18-splash
SK18_SPLASH_LICENSE = GPL-2.0+

# See sk18-keys.mk: Buildroot vendors Rust dependencies in the download step, which a local source tree skips.
define SK18_SPLASH_VENDOR_DEPENDENCIES
	mkdir -p $(@D)/.cargo $(BR_CARGO_HOME)
	cd $(@D) && \
		PATH="$(HOST_DIR)/bin:$$PATH" CARGO_HOME=$(BR_CARGO_HOME) \
		cargo vendor --manifest-path $(SK18_SPLASH_SUBDIR)/Cargo.toml --locked VENDOR \
		> .cargo/config.toml
endef
SK18_SPLASH_POST_RSYNC_HOOKS += SK18_SPLASH_VENDOR_DEPENDENCIES

$(eval $(cargo-package))
