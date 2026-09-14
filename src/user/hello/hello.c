#include <nyvela/user/syslib.h>

static char buf[512];

static void puts(const char *s) {
  sys_write(1, s, ustrlen(s));
}

void _start(void) {
  puts("hello from /bin/hello!\n");

  int64_t r = sys_fs_list("/", buf, sizeof(buf));

  if (r >= 0) {
    puts("root:\n");
    sys_write(1, buf, (uint64_t)r);
  }

  r = sys_fs_read("/readme.txt", buf, sizeof(buf) - 1, 0);

  if (r > 0) {
    buf[r] = '\0';
    puts("readme says: ");
    puts(buf);
    puts("\n");
  }

  sys_exit(0);
}
