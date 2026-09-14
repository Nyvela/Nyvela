#ifndef NYV_VFS_H
#define NYV_VFS_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#define VFS_NAME_MAX 63
#define VFS_PATH_MAX 256
#define VFS_MAX_FILE_SIZE (8ULL * 4096ULL)

#define VFS_OK 0
#define VFS_ERR_INVAL -1
#define VFS_ERR_NOTFOUND -2
#define VFS_ERR_EXISTS -3
#define VFS_ERR_NOTDIR -4
#define VFS_ERR_ISDIR -5
#define VFS_ERR_NOSPACE -6
#define VFS_ERR_BADPATH -7

bool vfs_init(void);

int vfs_create(const char *path, bool is_dir);

int64_t vfs_write(const char *path, const void *buf, uint64_t len, uint64_t offset);
int64_t vfs_read(const char *path, void *buf, uint64_t len, uint64_t offset);

int64_t vfs_list(const char *path, char *buf, uint64_t len);

int64_t vfs_size(const char *path);

#endif // NYV_VFS_H
