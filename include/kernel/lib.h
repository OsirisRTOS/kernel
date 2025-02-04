#ifndef KERNEL_H
#define KERNEL_H

#include "stdint.h"
#include "stdbool.h"
#include "stdarg.h"

/**
 * TODO: Make this dynamic.
 */
#define MAX_THREADS 32

/**
 * The memory map entry type.
 *
 * This structure shall be compatible with the multiboot_memory_map_t struct at
 * Link: https://www.gnu.org/software/grub/manual/multiboot/multiboot.html
 */
typedef struct __attribute__((packed)) MemMapEntry {
  uint32_t size;
  uint64_t addr;
  uint64_t length;
  uint32_t ty;
} MemMapEntry;

typedef struct BootInfo {
  const char *implementer;
  const char *variant;
  struct MemMapEntry mmap[8];
  uintptr_t mmap_len;
} BootInfo;

void kernel_init(const struct BootInfo *boot_info);

void syscall_dummy(const void *svc_args);

void syscall_reschedule(const void *_svc_args);

#endif /* KERNEL_H */
