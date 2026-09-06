#include "io.h"
#include "utils.h"
#include "mem.h"

void klog_ram_data() {
  char buf[21];
  size_t count = *MMAP_COUNT;
  
  if (!ki64toa(count, buf, sizeof(buf))) {
    kprintfail("Cannot convert MMAP_COUNT to string.", 0x0F);
  } else {
    char tmp[] = " memory map entries available.";
    char msg[21 + sizeof(tmp)];

    size_t i = 0;

    for (; buf[i]; i++)
        msg[i] = buf[i];

    for (size_t j = 0; tmp[j]; j++, i++)
        msg[i] = tmp[j];

    msg[i] = '\0';

    kprintinfo(msg, 0x0F);
  }
  
  size_t total = 0;

  for (size_t i = 0; i < count; i++) {
    if (MMAP_ENTRIES[i].type == E820_USABLE) {
      total += MMAP_ENTRIES[i].length_in_bytes;
    }
  }

  if (!ki64toa(total, buf, sizeof(buf))) {
    kprintfail("Cannot convert total available memory to string.", 0x0F);
  } else {
    char tmp[] = " bytes of RAM available.";
    char msg[21 + sizeof(tmp)];

    size_t i = 0;

    for (; buf[i]; i++)
        msg[i] = buf[i];

    for (size_t j = 0; tmp[j]; j++, i++)
        msg[i] = tmp[j];

    msg[i] = '\0';

    kprintinfo(msg, 0x0F);
  }
}

__attribute__((section(".text.entry")))
void kmain() {
  kprintsucc("Entered Kernel in long mode.", 0x0F);

  klog_ram_data();
  
  kprintinfo("Initializing PMM...", 0x0F);

  if (!kpmm_init()) {
    kprintferr("Failed to initialize PMM.", 0x0F);
    __asm__ volatile ("cli\nhlt");
  }
  
  kprintsucc("Initialized PMM.", 0x0F);

  __asm__ volatile ("cli\nhlt");
}
