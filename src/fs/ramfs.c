#include "../../include/nyvela/fs/ramfs.h"
#include "../../include/nyvela/fs/vfs.h"
#include "../../include/nyvela/mm/heap.h"
#include "../../include/nyvela/mm/pmm.h"
#include "../../include/nyvela/lib/utils.h"

#define RAMFS_PAGES_PER_FILE 8

typedef struct ramfs_node {
  char name[VFS_NAME_MAX + 1];
  bool is_dir;
  uint64_t size;
  struct ramfs_node *child;   // first child (dirs only)
  struct ramfs_node *sibling; // next sibling
  uint64_t pages[RAMFS_PAGES_PER_FILE]; // kpalloc phys (=identity virt)
  uint64_t page_count;
} ramfs_node_t;

static ramfs_node_t *ramfs_root = NULL;

static ramfs_node_t *ramfs_alloc_node(const char *name, bool is_dir) {
  ramfs_node_t *n = kmalloc(sizeof(ramfs_node_t));

  if (!n) return NULL;

  memset(n, 0, sizeof(ramfs_node_t));
  kstrncpy(n->name, name, VFS_NAME_MAX);
  n->name[VFS_NAME_MAX] = '\0';
  n->is_dir = is_dir;

  return n;
}

// Find direct child by name. Returns NULL if missing.
static ramfs_node_t *ramfs_find_child(ramfs_node_t *dir, const char *name) {
  if (!dir || !dir->is_dir) return NULL;

  for (ramfs_node_t *c = dir->child; c; c = c->sibling) {
    if (kstrcmp(c->name, name) == 0) return c;
  }

  return NULL;
}

// Split absolute path into parent node + leaf name.
// leaf_out must hold VFS_NAME_MAX+1 bytes.
// Returns parent or NULL on error. Handles "/" specially (parent=NULL, leaf="").
static ramfs_node_t *ramfs_parent_of(const char *path, char *leaf_out) {
  if (!path || path[0] != '/') return NULL;

  // Root itself has no parent.
  if (kstrcmp(path, "/") == 0) {
    leaf_out[0] = '\0';
    return NULL;
  }

  ramfs_node_t *cur = ramfs_root;
  size_t i = 1; // skip leading '/'

  char comp[VFS_NAME_MAX + 1];

  for (;;) {
    // Extract next component.
    size_t clen = 0;

    while (path[i] && path[i] != '/') {
      if (clen >= VFS_NAME_MAX) return NULL; // component too long
      comp[clen++] = path[i++];
    }

    comp[clen] = '\0';

    if (clen == 0) return NULL; // empty component ("//" or trailing "/")

    bool last = (path[i] == '\0');

    if (last) {
      kstrncpy(leaf_out, comp, VFS_NAME_MAX);
      leaf_out[VFS_NAME_MAX] = '\0';
      return cur;
    }

    // Intermediate: must descend into existing dir.
    ramfs_node_t *next = ramfs_find_child(cur, comp);

    if (!next || !next->is_dir) return NULL;

    cur = next;
    i++; // skip '/'
  }
}

static ramfs_node_t *ramfs_lookup(const char *path) {
  if (!path || path[0] != '/') return NULL;
  if (kstrcmp(path, "/") == 0) return ramfs_root;

  char leaf[VFS_NAME_MAX + 1];
  ramfs_node_t *parent = ramfs_parent_of(path, leaf);

  if (!parent) return NULL;

  return ramfs_find_child(parent, leaf);
}

bool ramfs_init(void) {
  if (ramfs_root) return true;

  ramfs_root = ramfs_alloc_node("", true);

  return ramfs_root != NULL;
}

int ramfs_create(const char *path, bool is_dir) {
  if (!ramfs_root || !path || path[0] != '/' || kstrcmp(path, "/") == 0) {
    return VFS_ERR_BADPATH;
  }

  if (kstrlen(path) > VFS_PATH_MAX) return VFS_ERR_BADPATH;

  char leaf[VFS_NAME_MAX + 1];
  ramfs_node_t *parent = ramfs_parent_of(path, leaf);

  if (!parent || leaf[0] == '\0') return VFS_ERR_BADPATH;
  if (!parent->is_dir) return VFS_ERR_NOTDIR;
  if (ramfs_find_child(parent, leaf)) return VFS_ERR_EXISTS;

  ramfs_node_t *n = ramfs_alloc_node(leaf, is_dir);

  if (!n) return VFS_ERR_NOSPACE;

  // Prepend to child list (O(1)).
  n->sibling = parent->child;
  parent->child = n;

  return VFS_OK;
}

static void *ramfs_ensure_page(ramfs_node_t *f, uint64_t page_idx) {
  while (f->page_count <= page_idx) {
    if (f->page_count >= RAMFS_PAGES_PER_FILE) return NULL;

    void *p = kpalloc();

    if (!p) return NULL;

    memset(p, 0, 4096);
    f->pages[f->page_count++] = (uint64_t)p;
  }

  return (void *)f->pages[page_idx];
}

int64_t ramfs_write(const char *path, const void *buf, uint64_t len, uint64_t offset) {
  if (!ramfs_root || !path || (!buf && len > 0)) return VFS_ERR_INVAL;

  ramfs_node_t *f = ramfs_lookup(path);

  if (!f) return VFS_ERR_NOTFOUND;
  if (f->is_dir) return VFS_ERR_ISDIR;
  if (len == 0) return 0;
  if (offset > f->size) return VFS_ERR_INVAL;
  if (offset + len > VFS_MAX_FILE_SIZE) return VFS_ERR_NOSPACE;

  const uint8_t *src = buf;
  uint64_t written = 0;

  while (written < len) {
    uint64_t off = offset + written;
    uint64_t pidx = off / 4096;
    uint64_t poff = off % 4096;
    uint64_t chunk = 4096 - poff;

    if (chunk > len - written) chunk = len - written;

    void *page = ramfs_ensure_page(f, pidx);

    if (!page) return VFS_ERR_NOSPACE;

    memcpy((uint8_t *)page + poff, src + written, (size_t)chunk);
    written += chunk;
  }

  uint64_t end = offset + written;

  if (end > f->size) f->size = end;

  return (int64_t)written;
}

int64_t ramfs_read(const char *path, void *buf, uint64_t len, uint64_t offset) {
  if (!ramfs_root || !path || (!buf && len > 0)) return VFS_ERR_INVAL;

  ramfs_node_t *f = ramfs_lookup(path);

  if (!f) return VFS_ERR_NOTFOUND;
  if (f->is_dir) return VFS_ERR_ISDIR;
  if (len == 0) return 0;
  if (offset >= f->size) return 0;

  if (offset + len > f->size) len = f->size - offset;

  uint8_t *dst = buf;
  uint64_t done = 0;

  while (done < len) {
    uint64_t off = offset + done;
    uint64_t pidx = off / 4096;
    uint64_t poff = off % 4096;
    uint64_t chunk = 4096 - poff;

    if (chunk > len - done) chunk = len - done;
    if (pidx >= f->page_count) break; // sparse (shouldn't happen)

    memcpy(dst + done, (void *)(f->pages[pidx] + poff), (size_t)chunk);
    done += chunk;
  }

  return (int64_t)done;
}

int64_t ramfs_list(const char *path, char *buf, uint64_t len) {
  if (!ramfs_root || !path || !buf || len == 0) return VFS_ERR_INVAL;

  ramfs_node_t *d = ramfs_lookup(path);

  if (!d) return VFS_ERR_NOTFOUND;
  if (!d->is_dir) return VFS_ERR_NOTDIR;

  uint64_t pos = 0;

  for (ramfs_node_t *c = d->child; c; c = c->sibling) {
    size_t nl = kstrlen(c->name);

    // Need name + '\n' + final NUL reserve.
    if (pos + nl + 1 + 1 > len) return VFS_ERR_NOSPACE;

    memcpy(buf + pos, c->name, nl);
    pos += nl;
    buf[pos++] = '\n';
  }

  buf[pos] = '\0';

  return (int64_t)pos;
}

int64_t ramfs_size(const char *path) {
  if (!ramfs_root || !path) return VFS_ERR_INVAL;

  ramfs_node_t *f = ramfs_lookup(path);

  if (!f) return VFS_ERR_NOTFOUND;
  if (f->is_dir) return VFS_ERR_ISDIR;

  return (int64_t)f->size;
}
