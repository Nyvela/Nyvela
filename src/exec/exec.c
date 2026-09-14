#include "../../include/nyvela/exec/exec.h"
#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/lib/utils.h"

int64_t exec_load(const char *path, uint64_t base, uint64_t max) {
  int64_t sz = vfs_size(path);

  if (sz < 0) return sz;
  if (sz == 0) return VFS_ERR_INVAL;
  if ((uint64_t)sz > max) return VFS_ERR_NOSPACE;

  memset((void *)base, 0, (size_t)max);

  int64_t r = vfs_read(path, (void *)base, (uint64_t)sz, 0);

  if (r != sz) return VFS_ERR_INVAL;

  return sz;
}
