# Static ELF applications

Lumen's ELF processes use the x86-64 Linux syscall and initial stack
conventions implemented by static musl applications. The toolchain wrappers
select the link address and build format expected by the kernel.

## Build and boot the applications

On Debian or Ubuntu, install Clang, LLD, the Clang compiler-rt development
package, musl development files, GNU cpio, and Freedoom in addition to the
[kernel dependencies](build.md). On Ubuntu the package names are `clang`,
`lld`, `libclang-rt-dev`, `musl-dev`, `cpio`, and `freedoom`.

Install the Rust targets and clone the upstream port sources alongside Lumen.

```sh
rustup target add x86_64-unknown-none x86_64-unknown-linux-musl
git clone --branch version-3.50.4 https://github.com/sqlite/sqlite.git ../sqlite
git clone https://github.com/ozkl/doomgeneric.git ../doomgeneric
git -C ../doomgeneric checkout e257a09527062bfb986b6e5e0780839b6cbd5b71
make -j8 all ports
make run-ports
```

`make ports` creates `build/ports/lumen.cpio`. QEMU supplies it through
fw_cfg, and Lumen extracts it to RAMFS during boot. `SQLITE_DIR`, `DOOM_DIR`,
and `DOOM_IWAD` override the checkout and WAD locations. `make run` boots the
base image with NVMe and Ethernet. The four-CPU QEMU machine uses Standard
VGA, RTL8139 and a 256 MiB RAM default.

At the `Lumen >` prompt in the QEMU window:

```text
run hello-std "two words"
run sqlite3 /demo.db "create table demo (n); insert into demo values (42);"
run sqlite3 /demo.db "select n from demo;"
run doom
```

SQLite uses the dotfile-lock VFS with rollback journals. Databases and Doom
saves are stored in RAMFS for the current boot. Live NVMe/FAT32 storage is
accessible through the existing `fat` utility. Doom uses the Freedoom Phase 1
IWAD from the host package, a 640×400 framebuffer, and PS/2 key events.
Arrow keys move, Ctrl fires, Space uses doors, and Escape opens the menu.
Audio is disabled.

## Build another application

```sh
tools/lumen-rustc -C opt-level=s -C strip=symbols hello.rs -o hello
tools/lumen-cargo build --manifest-path /path/to/Cargo.toml --release
tools/lumen-clang -O2 -s hello.c -o hello
readelf -l hello
```

The Clang wrapper defaults to the Debian/Ubuntu x86-64 musl include and
library directories. Set `MUSL_INCLUDE`, `MUSL_LIB`, or `CLANG` for a
different installation. The Rust compiler uses its own bundled static musl
runtime and the upstream `x86_64-unknown-linux-musl` standard library.

Place applications in `bin/` in a `newc` cpio archive and pass it to the
base VM with `-fw_cfg name=opt/lumen/initrd,file=/path/to/archive.cpio`.
Archive names are relative to the RAMFS root, with directories before the
files they contain. The shell runs executables from `/bin`.

## Executable ABI

* The loader accepts little-endian x86-64 static `ET_EXEC` images. The
  wrappers link at `0x50000000`. Load segments can occupy the intervals
  `0x400000..0x800000` or `0x40000000..0xc0000000`. Each image has a 64 MiB
  mapped-size limit.
* Each ELF program begins at its header entry point with a private address
  space, page permissions from `PT_LOAD`, zero-filled BSS, and a guarded 1 MiB
  stack. The initial stack supplies `argc`, `argv`, `HOME`, `PATH`, `TERM`,
  and auxiliary-vector entries for the musl runtime. The scheduler saves
  x87/SSE state and the FS base used by TLS.
* The `syscall` instruction uses Linux x86-64 argument registers and errno
  values. The subset covers terminal I/O, RAMFS file descriptors and
  directories, anonymous mappings, `brk`, time, entropy on CPUs with RDRAND,
  TLS and IPv4 sockets. Dynamic linking, PIE, process fork, POSIX threads,
  signal delivery and AVX are outside this ABI. Synchronous faults terminate
  the foreground program and report exit status 128 plus the corresponding
  signal number.
* The BSP schedules userspace. Init and several device services still use
  the native flat-image ABI. The ELF syscall path has its own pointer
  validation, address-space lifecycle and descriptor table.

## Network

Lumen uses smoltcp with the [Lumeth RTL8139 driver](https://github.com/amogus-gggy/Lumeth).
The QEMU user network supplies the gateway at `10.0.2.2`. Lumen configures
the guest statically at `10.0.2.15/24`. IPv4 TCP client and listener sockets
and UDP datagrams are supported. Ports can use Rust `std::net` with numeric
IPv4 socket addresses. To expose a guest listener on host loopback:

```sh
make run-ports QEMU_NET='-nic user,model=rtl8139,hostfwd=tcp:127.0.0.1:8080-:8080'
```
