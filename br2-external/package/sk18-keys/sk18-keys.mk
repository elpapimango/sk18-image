################################################################################
#
# sk18-keys
#
################################################################################

SK18_KEYS_VERSION = 0.1.0
# Built from the sources in this repository (client/), not downloaded.
SK18_KEYS_SITE = $(BR2_EXTERNAL_SK18_PATH)/../client
SK18_KEYS_SITE_METHOD = local
SK18_KEYS_SUBDIR = sk18-keys
SK18_KEYS_LICENSE = GPL-2.0+

# Buildroot builds Rust packages offline from a vendored copy of their dependencies, and normally makes that
# copy in the download step. A local source tree is never downloaded, so do the same here: the same
# `cargo vendor` command as support/download/cargo-post-process, pinned by Cargo.lock (--locked), which needs
# network access to crates.io once (the downloads are cached in $(BR_CARGO_HOME)).
define SK18_KEYS_VENDOR_DEPENDENCIES
	mkdir -p $(@D)/.cargo $(BR_CARGO_HOME)
	cd $(@D) && \
		PATH="$(HOST_DIR)/bin:$$PATH" CARGO_HOME=$(BR_CARGO_HOME) \
		cargo vendor --manifest-path $(SK18_KEYS_SUBDIR)/Cargo.toml --locked VENDOR \
		> .cargo/config.toml
endef
SK18_KEYS_POST_RSYNC_HOOKS += SK18_KEYS_VENDOR_DEPENDENCIES

$(eval $(cargo-package))
