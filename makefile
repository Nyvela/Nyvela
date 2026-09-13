CC := gcc
AS := nasm
LD := ld
OBJCOPY := objcopy

SRC_DIR := src
BUILD := build

ARCH := x86_64
ARCH_DIR := $(SRC_DIR)/arch/$(ARCH)
BOOT_DIR := $(ARCH_DIR)/boot

BOOT := $(BUILD)/boot.bin
STAGE1 := $(BUILD)/stage_1.bin
STAGE2 := $(BUILD)/stage_2.bin
KERNEL := $(BUILD)/kernel.bin
IMAGE := $(BUILD)/os.img

CFLAGS := -ffreestanding -m64 -mno-red-zone \
          -fno-stack-protector -fno-pie \
          -Wall -Wextra \
          -Iinclude

# Rust toolchain (rustup default location) for user-space Rust programs.
PATH := $(HOME)/.cargo/bin:$(PATH)

# User programs: flat freestanding binaries for fixed base addresses.
# -ffunction-sections + .text._start first in ld scripts => _start at BASE.
USER_CFLAGS := -ffreestanding -m64 -mno-red-zone \
          -fno-stack-protector -fno-pie -fno-pic \
          -ffunction-sections -fdata-sections \
          -Wall -Wextra \
          -Iinclude
USER_LDFLAGS := -nostdlib -static -Wl,--gc-sections

USER_BUILD := $(BUILD)/user
USER_SHELL_ELF := $(USER_BUILD)/shell.elf
USER_SHELL_BIN := $(USER_BUILD)/shell.bin
USER_HELLO_ELF := $(USER_BUILD)/hello.elf
USER_HELLO_BIN := $(USER_BUILD)/hello.bin

ASFLAGS := -f elf64
LDFLAGS := -T linker.ld

# NOTE: src/user/*.c are separate user-space binaries (see USER rules below),
# not part of the kernel image.
KERNEL_C_SRC := $(shell find $(SRC_DIR) -type f -name '*.c' \
                   ! -path '$(SRC_DIR)/user/*')

KERNEL_ASM_SRC := $(shell find $(SRC_DIR) -type f -name '*.s' \
                   ! -path '$(BOOT_DIR)/*')

C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(KERNEL_C_SRC))
ASM_OBJ := $(patsubst $(SRC_DIR)/%.s,$(BUILD)/%.o,$(KERNEL_ASM_SRC))

KERNEL_OBJ := $(C_OBJ) $(ASM_OBJ)

.PHONY: all clean run debug

all: $(IMAGE)

$(BOOT): $(BOOT_DIR)/boot.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ $< -o $@

$(STAGE1): $(BOOT_DIR)/stage_1.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ $< -o $@

$(STAGE2): $(BOOT_DIR)/stage_2.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.s
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) $< -o $@

# --- user-space programs (flat binaries embedded into the kernel) ---

# Rust shell (own cargo crate, no_std, x86_64-unknown-none).
# Linked with the same shell.ld so _start lands at 0x400000.
USER_SHELL_RS_SRC := $(SRC_DIR)/user/shell/src/main.rs $(SRC_DIR)/user/shell/src/sys.rs $(SRC_DIR)/user/shell/Cargo.toml

$(USER_SHELL_ELF): $(USER_SHELL_RS_SRC) $(SRC_DIR)/user/ld/shell.ld
	@mkdir -p $(dir $@) $(BUILD)/cargo
	@command -v cargo >/dev/null || (echo "error: cargo not found (need rustup toolchain + x86_64-unknown-none target)"; exit 1)
	@command -v rustc >/dev/null || (echo "error: rustc not found"; exit 1)
	@# no-redzone is mandatory: switch_context pushes the iretq frame 40 bytes
	@# below the saved user RSP, which would clobber LLVM's red-zone locals.
	CARGO_TARGET_DIR="$(CURDIR)/$(BUILD)/cargo" \
	RUSTFLAGS="-C link-arg=-T$(CURDIR)/$(SRC_DIR)/user/ld/shell.ld -C link-arg=-nostdlib -C link-arg=-static -C link-arg=-no-pie -C link-arg=--gc-sections -C no-redzone=yes" \
	cargo build --manifest-path $(SRC_DIR)/user/shell/Cargo.toml --release --target x86_64-unknown-none
	cp $(BUILD)/cargo/x86_64-unknown-none/release/shell $@

$(USER_SHELL_BIN): $(USER_SHELL_ELF)
	$(OBJCOPY) -O binary $< $@

$(USER_HELLO_ELF): $(SRC_DIR)/user/hello/hello.c include/nyvela/user/syslib.h $(SRC_DIR)/user/ld/prog.ld
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -T $(SRC_DIR)/user/ld/prog.ld $(USER_LDFLAGS) $< -o $@

$(USER_HELLO_BIN): $(USER_HELLO_ELF)
	$(OBJCOPY) -O binary $< $@

# Blob objects incbin the flat binaries, so they must be built first.
$(BUILD)/user/blobs/shell_blob.o: $(USER_SHELL_BIN)
$(BUILD)/user/blobs/hello_blob.o: $(USER_HELLO_BIN)

$(KERNEL): $(KERNEL_OBJ)
	@mkdir -p $(dir $@)
	$(LD) $(LDFLAGS) -o $(BUILD)/kernel.elf $(KERNEL_OBJ)
	$(OBJCOPY) -O binary $(BUILD)/kernel.elf $@

# Kernel disk reservation. Must match DAP_kernel count in stage_1.s and
# KERNEL_SIZE_IN_SECTORS in stage_2.s. LBA 37 = 1 (boot) + 4 (stage1) + 32 (stage2).
KERNEL_SECTORS := 96
KERNEL_LBA := 37

$(IMAGE): $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL)
	@mkdir -p $(dir $@)
	@if [ $$(stat -c%s $(KERNEL)) -gt $$(( $(KERNEL_SECTORS) * 512 )) ]; then \
		echo "error: $(KERNEL) larger than $(KERNEL_SECTORS) sectors; bump KERNEL_SECTORS + stage_1.s/stage_2.s"; \
		exit 1; \
	fi
	cat $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL) > $@
	@# Pad so the BIOS INT 13h read of KERNEL_SECTORS at LBA $(KERNEL_LBA)
	@# never runs past end-of-disk (past-EOF reads fail with JC).
	@truncate -s $$(( ( $(KERNEL_LBA) + $(KERNEL_SECTORS) ) * 512 )) $@

run: $(IMAGE)
	qemu-system-x86_64 \
		-drive format=raw,file=$(IMAGE) \
		-d int,cpu_reset,guest_errors

debug: $(IMAGE)
	qemu-system-x86_64 \
		-drive format=raw,file=$(IMAGE) \
		-d int,cpu_reset,guest_errors \
		-D qemu.log \
		-no-reboot -no-shutdown \
		-s -S

clean:
	rm -rf $(BUILD)
