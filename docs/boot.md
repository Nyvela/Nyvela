# Boot chain

```
BIOS -> boot.s (0x7C00) -> stage_1.s (0x8000) -> stage_2.s (0x8800) -> kmain (0x100000)
```

Each stage does one job and hands over. The CPU starts in 16-bit real mode and
ends up in 64-bit long mode with paging on, jumping into a kernel.

## boot.s - the MBR

Loaded by the BIOS at `0x7C00`. Sets up segments, saves the boot drive number at
`0x7B00`, sets video mode `0x03`, then reads 4 sectors from LBA 1 to
`0x0800:0x0000` using an INT 13h extended-read DAP, and far jumps to
`0x0800:0x0000`.

Stage 1 is capped at 4 sectors; `times 4 * 512 - ($ - $$) db 0` makes nasm fail
loudly if it ever outgrows that.

## stage_1.s - real mode setup, staging, protected mode

1. Loads `stage_2` (32 sectors, LBA 5) to `0x0880:0x0000`.
2. Loads the kernel to `0x1000:0x0000` = `0x10000`.
3. Collects the E820 memory map into `0xD000`, entry count at `0xCFFE`.
4. Enables A20 through the BIOS (`INT 15h` `AX=0x2401`, verified with
   `0x2402`).
5. Builds a GDT by hand - null, 32-bit kernel code, 32-bit kernel data, user
   code, user data, and a 64-bit code segment - via a small
   `encode_gdt_entry` helper, then `lgdt`.
6. Sets `CR0.PE` and far jumps to `0x08:protected_mode_entry`, which reloads the
   data segments from selector `0x10` and jumps to `0x8800`.

Paging is deliberately *disabled* on entry to stage 2 so long mode can be set up
in a known state.

## stage_2.s - long mode

1. Clears `CR0.PG`, sets `CR4.PAE`, sets `LME` in `EFER`.
2. Builds a minimal 1:1 identity map at fixed tables: PML4 at `0x9000`, PDPT at
   `0xA000`, PD at `0xB000`, with 512 entries of `0x83` (present, writable,
   2 MiB pages) covering the low gigabyte.
3. Loads `CR3` and sets `CR0.PG`.
4. Far jumps to selector `0x28` (the 64-bit code segment), sets `RSP = 0x80000`.
5. Copies the staged kernel from `0x10000` to `0x100000` with `rep movsq`,
   `KERNEL_SECTORS * 512 / 8` quadwords, and jumps there.

Stage 2 is padded to exactly 16384 bytes, matching the 32 sectors stage 1 reads.

### Why stage at `0x10000`?

The kernel is linked for `0x100000`, but the loaders run in 16-bit mode and only
comfortably address the low megabyte, and BIOS reads land below `0x100000`. So
the image is staged in low memory and relocated once paging is up. The copy is
driven by the same `KERNEL_SECTORS` value as the read, which is why a mismatch
there shows up as a kernel that runs but prints nothing.

## Kernel image

`linker.ld` lays out the kernel at `0x100000`:

| section | notes |
|---------|-------|
| `.text` | `.text.entry` first, so `kmain` is the first thing executed |
| `.rodata` | 4 KiB aligned |
| `.data` | 4 KiB aligned |
| `.bss` | 4 KiB aligned |
| `.stack` | 16 KiB, exported as `kernel_stack_bottom` / `kernel_stack_top` |

`kernel_end` marks the end. `objcopy -O binary` emits `.text` through `.data`;
`.bss` and `.stack` are `NOBITS` and are *not* in the flat image, so they rely on
the destination being zeroed (true in practice for fresh RAM, but worth
remembering if the load address ever changes).

## Image layout

| region | sectors | LBA |
|--------|---------|-----|
| `boot` | 1 | 0 |
| `stage_1` | 4 | 1 |
| `stage_2` | 32 | 5 |
| `kernel` | `KERNEL_SECTORS` | 37 |

`make` concatenates them into `build/os.img` and pads it to
`(KERNEL_LBA + KERNEL_SECTORS) * 512` bytes so the BIOS read never runs past EOF.
There is no upper bound on kernel size - `stage_1` issues as many 128-sector
reads as it takes.

## Memory management after boot

`kmain` turns the 2 MiB boot identity map into 4 KiB pages and builds the real
allocators:

- **PMM** (`src/mm/pmm.c`): E820-driven bitmap at `0x10000`, marking
  `BITMAP_RESERVED` vs `BITMAP_FREE`. Everything from `kernel_end` to the top of
  RAM is allocatable. `kpalloc`/`kpfree` are page-granular.
- **VMM** (`src/mm/vmm.c`): 4-level paging, `kvmmap`/`kvmunmap`, propagating
  `U/S` on the path tables when `flags & 0x04`, plus per-address-space helpers
  (`kvmmap_at`, `kvmunmap_at`, `kvmunmap_and_free_at`, `kget_pte_addr_at`,
  `kget_phys_page_addr_at`, `kvmm_create_user_pml4`, `kvmm_free_user_pml4`).
  TLB is flushed by reloading `CR3`.
- **Heap** (`src/mm/heap.c`): `kmalloc`/`krealloc`/`kfree` over a `block_t`
  header with `size`, `is_used`, `next`. 16-byte aligned, splits blocks,
  bootstraps from `kpalloc`. One allocation must stay under ~4 KiB; `kfree`
  frees without coalescing.

## Interrupts

`kidt_init` installs a 256-slot IDT with 7 vectors: `#DE`, `#PF`, `#GP` stubs
plus PIT `0x20`, keyboard `0x21`, LAPIC timer `0x30`, and the syscall gate
`0x80` at DPL 3. The PIT is used only to calibrate the LAPIC and is then
disabled (`kpic_disable`).

Fault handling is log-and-halt: the handlers dump the vector, error code, RIP,
CS, RSP and 16 bytes at RIP in hex, then stop. There is no recovery and no IST
stacks yet, so a fault in ring3 takes the whole machine down rather than killing
the process.

See also: [build system](build.md), [debugging](debugging.md).
