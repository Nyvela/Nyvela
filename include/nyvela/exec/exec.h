#ifndef NYV_EXEC_H
#define NYV_EXEC_H

#include <stdint.h>

// Minimal flat-binary loader. Copies a ramfs file into an already-mapped
// window [base, base+max), zeroing it first (covers .bss of flat blobs).
// Returns the loaded size in bytes, or negative VFS_ERR_*.
int64_t exec_load(const char *path, uint64_t base, uint64_t max);

#endif // NYV_EXEC_H
