# Builds sk18_regs.ko against the vendor Tina SDK kernel tree (Linux 5.4.61) with the vendor toolchain.
#   make KDIR=$HOME/Tina-Linux/lichee/linux-5.4 \
#        TOOLCHAIN=$HOME/Tina-Linux/prebuilt/gcc/linux-x86/arm/toolchain-sunxi-glibc/toolchain
# The module only matches a kernel with vermagic "5.4.61 SMP preempt mod_unload ARMv7 p2v8".
obj-m += sk18_regs.o

# The tree's GCC plugin (per-task stack protector) was built for the host gcc and does not load into the
# vendor gcc 6.4.1. The module needs neither the plugin nor a stack protector (it has no local buffers).
ccflags-y += -fno-stack-protector

KDIR ?= $(HOME)/Tina-Linux/lichee/linux-5.4
TOOLCHAIN ?= $(HOME)/Tina-Linux/prebuilt/gcc/linux-x86/arm/toolchain-sunxi-glibc/toolchain

all:
	PATH=$(TOOLCHAIN)/bin:$$PATH STAGING_DIR=$(TOOLCHAIN) \
	$(MAKE) -C $(KDIR) M=$(CURDIR) ARCH=arm CROSS_COMPILE=arm-openwrt-linux-gnueabi- \
		CC=$(CURDIR)/gccwrap.sh GCC_PLUGINS_CFLAGS= modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
