/*
 * regdump: print ranges of 32-bit hardware registers through /dev/mem (read-only).
 *
 * usage: regdump ADDR LEN [ADDR LEN ...]       (numbers as in C: 0x05461000 0x1000)
 *
 * Output: lines of "0xADDRESS: w0 w1 w2 w3" (16 bytes per line, hexadecimal words). Used to compare the
 * display registers of the stock firmware with those of the mainline kernel on the SK-18.
 * Only aligned 32-bit reads are done. A page that cannot be mapped is reported and skipped.
 *
 * Build: arm-linux-gcc -static -O2 -Wall -o regdump regdump.c
 */
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>

int main(int argc, char **argv)
{
	long page = sysconf(_SC_PAGESIZE);
	int fd = open("/dev/mem", O_RDONLY | O_SYNC);

	if (fd < 0) {
		perror("open /dev/mem");
		return 1;
	}
	for (int i = 1; i + 1 < argc; i += 2) {
		unsigned long addr = strtoul(argv[i], NULL, 0) & ~3UL;
		unsigned long end = addr + strtoul(argv[i + 1], NULL, 0);

		for (unsigned long base = addr & ~(unsigned long)(page - 1); base < end; base += page) {
			volatile uint32_t *map = mmap(NULL, page, PROT_READ, MAP_SHARED, fd, base);

			if (map == MAP_FAILED) {
				printf("0x%08lx: cannot map\n", base);
				continue;
			}
			for (unsigned long a = base; a < base + page; a += 4) {
				if (a < addr || a >= end)
					continue;
				if ((a - addr) % 16 == 0)
					printf("%s0x%08lx:", a == addr ? "" : "\n", a);
				printf(" %08x", (unsigned)map[(a - base) / 4]);
			}
			printf("\n");
			munmap((void *)map, page);
		}
	}
	return 0;
}
