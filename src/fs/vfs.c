#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/fs/ramfs.h"
#include "../../include/nyvela/lib/utils.h"

bool vfs_init(void) {
  return ramfs_init();
}

static bool vfs_path_ok(const char *path) {
  if (!path || path[0] != '/') return false;
  if (kstrlen(path) > VFS_PATH_MAX) return false;

  return true;
}

int vfs_create(const char *path, bool is_dir) {
  if (!vfs_path_ok(path)) return VFS_ERR_BADPATH;

  return ramfs_create(path, is_dir);
}

int64_t vfs_write(const char *path, const void *buf, uint64_t len, uint64_t offset) {
  if (!vfs_path_ok(path)) return VFS_ERR_BADPATH;

  return ramfs_write(path, buf, len, offset);
}

int64_t vfs_read(const char *path, void *buf, uint64_t len, uint64_t offset) {
  if (!vfs_path_ok(path)) return VFS_ERR_BADPATH;

  return ramfs_read(path, buf, len, offset);
}

int64_t vfs_list(const char *path, char *buf, uint64_t len) {
  if (!vfs_path_ok(path)) return VFS_ERR_BADPATH;

  return ramfs_list(path, buf, len);
}

int64_t vfs_size(const char *path) {
  if (!vfs_path_ok(path)) return VFS_ERR_BADPATH;

  return ramfs_size(path);
}
