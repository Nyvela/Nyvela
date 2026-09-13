#ifndef NYV_RAMFS_H
#define NYV_RAMFS_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

bool ramfs_init(void);
int ramfs_create(const char *path, bool is_dir);
int64_t ramfs_write(const char *path, const void *buf, uint64_t len, uint64_t offset);
int64_t ramfs_read(const char *path, void *buf, uint64_t len, uint64_t offset);
int64_t ramfs_list(const char *path, char *buf, uint64_t len);
int64_t ramfs_size(const char *path);

#endif // NYV_RAMFS_H
