# 🌌 Nyvela

A modern 64-bit operating system written from scratch in x86-64 Assembly and C. 

Nyvela is an experimental OS focused on understanding computer fundamentals, from the boot process and CPU initialization to memory management, hardware support, and userspace.

## ⚡ Features

* **🚀 Custom Boot Chain:** `boot` (MBR) ➔ `stage_1` ➔ `stage_2`.
* **🧠 Memory Management:** Physical Memory Manager (bitmap), Virtual Memory Manager (4-level paging), and a custom heap (`kmalloc`/`kfree`).
* **⚙️ Hardware & CPU:** GDT, IDT, LAPIC, CPUID, MSR, and Port I/O.
* **🧵 Multitasking:** Thread abstraction, round-robin scheduler, and `iretq`-based context switching.
* **🖥️ Display:** VGA text mode console and colorful kernel logging.
* **🛠️ Architecture:** Real mode ➔ Protected mode ➔ Long mode, with PAE and LME/LMA setup.

## 🏗️ Architecture & Memory

* **Boot Flow:** `BIOS` ➔ `boot.s` (0x7C00) ➔ `stage_1` (0x8000) ➔ `stage_2` (0x8800) ➔ `kernel` (0x100000).
* **PMM:** E820-driven bitmap allocator. Page-granular (4 KiB).
* **VMM:** Runtime PML4 creation, low memory mapped with 2 MiB pages, supports 4 KiB mapping/unmapping.
* **Heap:** Block header with `size`, `is_used`, `next`. 16-byte alignment, block splitting, on-demand page allocation.

## 🛠️ Build & Run

*Requirements:* `gcc` (x86-64), `nasm`, `ld`, `objcopy`, `qemu-system-x86_64`, `make`.

```sh
make        # Build build/os.img
make run    # Run in QEMU
make debug  # Run QEMU with GDB stub (-s -S)
make clean  # Clean build/
```

## 📂 Project Structure

```text
├── include/nyvela/  # Headers (arch, drivers, lib, mm, scheduler, thread)
├── src/             # Source (boot asm, kernel, mm, drivers, cpu)
├── linker.ld        # Kernel layout (0x100000 entry)
└── makefile         # Build system
```

## 🗺️ Roadmap

### Boot

* [x] Boot from disk
* [x] Enable A20
* [x] Set up GDT and IDT
* [x] Enable protected mode
* [x] Enable long mode and paging
* [x] Load kernel to `0x100000`

### Kernel

* [x] Initialize hardware (LAPIC + LAPIC timer)
* [x] Implement physical memory management (`kpmm_init`, `kpalloc`/`kpfree`)
* [x] Implement virtual memory (`kvmm_init`, `kvmmap`/`kvmunmap`)
* [x] Implement interrupts and exceptions (IDT + DE/PF/GP ISRs + LAPIC timer tick)
* [x] Implement a heap allocator (`kmalloc`/`krealloc`/`kfree`)
* [x] Implement basic threads (`spawn_thread`; no processes yet)
* [x] Implement a basic scheduler (round-robin on LAPIC timer)
* [ ] Implement system calls
* [x] Add x86-64 asm abstractions (`arch/x86_64/asm/`: cpu/io/desc/msr/cpuid)
* [ ] Stable preemptive scheduler
* [ ] Full exception handling (DE, PF, GP)
* [ ] SMP support
* [x] TSS and IST for fault isolation (ring 0 only for now)

### Userspace

* [ ] Userspace support and privilege separation
* [ ] Standard library
* [ ] Shell and basic utilities
* [ ] Filesystem support

### Graphics

* [ ] Implement framebuffer support
* [x] Implement text rendering (VGA text-mode console; no framebuffer fonts yet)
* [ ] Implement input handling
* [ ] Build a graphical interface

## 🤝 Contributing

Contributions are welcome! Open an issue or PR, keep commits focused, and ensure `make` builds cleanly.

## 📜 License

GPL-3.0. See `LICENSE` for details.