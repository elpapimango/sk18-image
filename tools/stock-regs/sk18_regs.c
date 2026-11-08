// SPDX-License-Identifier: GPL-2.0
/*
 * sk18_regs: read-only register dump for the STOCK SK-18 kernel (Linux 5.4.61, no CONFIG_DEVMEM).
 *
 * Provides /proc/sk18_regs with the display-related hardware registers of the Allwinner T113-S3, so that
 * the register state of the working vendor display pipeline can be compared with the mainline kernel.
 * Only 32-bit reads are done; nothing is written. Debugging aid, not part of the product.
 */
#include <linux/io.h>
#include <linux/module.h>
#include <linux/proc_fs.h>
#include <linux/seq_file.h>

struct range {
	unsigned long base;
	unsigned long len;
	const char *name;
};

static const struct range ranges[] = {
	{ 0x05450000, 0x1000, "DSI" },
	{ 0x05451000, 0x1000, "D-PHY" },
	{ 0x05460000, 0x0100, "TCON TOP" },
	{ 0x05461000, 0x1000, "TCON LCD0" },
	{ 0x05000000, 0x0100, "DE top" },
	{ 0x05100000, 0x0100, "mixer0 global" },
	{ 0x05101000, 0x0100, "mixer0 blender" },
	{ 0x05102000, 0x0100, "mixer0 VI channel 0" },
	{ 0x05103000, 0x0100, "mixer0 UI channel 0" },
	{ 0x02001000, 0x1000, "CCU" },
	{ 0x02000000, 0x0400, "PIO" },
};

static int sk18_regs_show(struct seq_file *m, void *unused)
{
	unsigned int i;

	for (i = 0; i < ARRAY_SIZE(ranges); i++) {
		void __iomem *map = ioremap(ranges[i].base, ranges[i].len);
		unsigned long off;

		seq_printf(m, "## %s 0x%08lx\n", ranges[i].name, ranges[i].base);
		if (!map) {
			seq_puts(m, "cannot map\n");
			continue;
		}
		for (off = 0; off < ranges[i].len; off += 4) {
			if (off % 16 == 0)
				seq_printf(m, "%s0x%08lx:", off ? "\n" : "", ranges[i].base + off);
			seq_printf(m, " %08x", readl_relaxed(map + off));
		}
		seq_puts(m, "\n");
		iounmap(map);
	}
	return 0;
}

static int __init sk18_regs_init(void)
{
	if (!proc_create_single("sk18_regs", 0444, NULL, sk18_regs_show))
		return -ENOMEM;
	return 0;
}

static void __exit sk18_regs_exit(void)
{
	remove_proc_entry("sk18_regs", NULL);
}

module_init(sk18_regs_init);
module_exit(sk18_regs_exit);
MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Read-only display register dump for the stock SK-18 kernel");
