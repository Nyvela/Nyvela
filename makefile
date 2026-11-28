-include lumen.conf

.DEFAULT_GOAL := all

INIT_DIR ?= ../Lumd
INIT_BIN_NAME ?= lumd
INIT_BUILD_COMMAND ?= USER_CFLAGS="-I$(CURDIR)/include $(X86_CFLAGS)" $(MAKE) -C $(INIT_DIR)
SHELL_DIR ?= ../Lumsh
SHELL_BIN_NAME ?= shell
SHELL_BUILD_COMMAND ?= $(MAKE) -C $(SHELL_DIR)
NVMED_DIR ?= ../nvmed-rs
NVMED_BIN_NAME ?= nvmed
LPCID_DIR ?= ../Lpcid
LPCID_BIN_NAME ?= lpcid
FAT_DIR ?= ../Lumfat

INIT_BIN := $(INIT_DIR)/build/$(INIT_BIN_NAME).bin
SHELL_BIN := $(SHELL_DIR)/build/$(SHELL_BIN_NAME).bin
SHELL_INPUTS := $(wildcard $(SHELL_DIR)/src/*.rs) $(SHELL_DIR)/makefile \
                $(SHELL_DIR)/Cargo.toml $(SHELL_DIR)/linker.ld
INIT_INPUTS := $(shell find $(INIT_DIR)/src $(INIT_DIR)/include -type f 2>/dev/null) \
               $(INIT_DIR)/makefile $(INIT_DIR)/linker.ld

NVMED_BIN := $(NVMED_DIR)/build/$(NVMED_BIN_NAME).bin
NVMED_INPUTS := $(shell find $(NVMED_DIR)/src $(NVMED_DIR)/include -type f 2>/dev/null) \
                $(NVMED_DIR)/makefile $(NVMED_DIR)/linker.ld \
                $(wildcard $(NVMED_DIR)/Cargo.toml $(NVMED_DIR)/Cargo.lock)

LPCID_BIN := $(LPCID_DIR)/build/$(LPCID_BIN_NAME).bin
LPCID_INPUTS := $(shell find $(LPCID_DIR)/src $(LPCID_DIR)/include -type f 2>/dev/null) \
                $(LPCID_DIR)/makefile $(LPCID_DIR)/linker.ld

FAT_BIN := $(FAT_DIR)/build/fat.bin
FAT_INPUTS := $(wildcard $(FAT_DIR)/src/*.rs) $(FAT_DIR)/Cargo.toml \
              $(FAT_DIR)/Cargo.lock $(FAT_DIR)/makefile $(FAT_DIR)/linker.ld

CC := gcc
AS := nasm
LD := ld
OBJCOPY := objcopy
CARGO ?= cargo

SRC_DIR := src
BUILD := build

ARCH := x86_64
ARCH_DIR := $(SRC_DIR)/arch/$(ARCH)
BOOT_DIR := $(ARCH_DIR)/boot

BOOT := $(BUILD)/boot.bin
STAGE1 := $(BUILD)/stage_1.bin
STAGE2 := $(BUILD)/stage_2.bin
AP_BOOT := $(BUILD)/ap.bin
KERNEL := $(BUILD)/kernel.bin
IMAGE := $(BUILD)/os.img

# Derived from the linked kernel so the boot loaders and the image can never
# disagree about how many sectors the kernel occupies.
KERNEL_SECTORS = $(shell s=$$(stat -c%s $(KERNEL)); echo $$(( (s + 511) / 512 )))
KERNEL_LBA := 37

# Thread contexts do not save floating-point or SIMD registers.
X86_CFLAGS := -m64 -mno-red-zone -mgeneral-regs-only

CFLAGS := -std=gnu2x -ffreestanding $(X86_CFLAGS) \
          -fno-stack-protector -fno-pie \
          -Wall -Wextra \
          -Iinclude -g -MMD -MP

USER_CFLAGS := -std=gnu2x -ffreestanding $(X86_CFLAGS) \
          -fno-stack-protector -fno-pie -fno-pic \
          -ffunction-sections -fdata-sections \
          -Wall -Wextra \
          -Iinclude

USER_LDFLAGS := -nostdlib -static -Wl,--gc-sections

USER_BUILD := $(BUILD)/user
USER_HELLO_ELF := $(USER_BUILD)/hello.elf
USER_HELLO_BIN := $(USER_BUILD)/hello.bin

INIT_BLOB_S := $(SRC_DIR)/user/blobs/init_blob.s
INIT_BLOB_O := $(BUILD)/user/blobs/init_blob.o

SHELL_BLOB_S := $(SRC_DIR)/user/blobs/shell_blob.s
SHELL_BLOB_O := $(BUILD)/user/blobs/shell_blob.o

NVMED_BLOB_S := $(SRC_DIR)/user/blobs/nvmed_blob.s
NVMED_BLOB_O := $(BUILD)/user/blobs/nvmed_blob.o

LPCID_BLOB_S := $(SRC_DIR)/user/blobs/lpcid_blob.s
LPCID_BLOB_O := $(BUILD)/user/blobs/lpcid_blob.o

ASFLAGS := -f elf64
LDFLAGS := -T linker.ld

KERNEL_C_SRC := $(shell find $(SRC_DIR) -type f -name '*.c' \
                   ! -path '$(SRC_DIR)/user/*')

KERNEL_ASM_SRC := $(shell find $(SRC_DIR) -type f -name '*.s' \
                   ! -path '$(BOOT_DIR)/*')

C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(KERNEL_C_SRC))
ASM_OBJ := $(patsubst $(SRC_DIR)/%.s,$(BUILD)/%.o,$(KERNEL_ASM_SRC))

KERNEL_OBJ := $(C_OBJ) $(ASM_OBJ)
RUST_DIR := $(SRC_DIR)/kernel/rust
RUST_LIB := $(BUILD)/kernel/liblumen_rust.a
RUST_CARGO_TARGET := $(BUILD)/kernel/cargo
RUST_SOURCES := $(shell find $(RUST_DIR) -type f -name '*.rs') \
                $(RUST_DIR)/Cargo.toml $(RUST_DIR)/Cargo.lock \
                ../Lumfw/Cargo.toml $(wildcard ../Lumfw/src/*.rs) \
                ../Lumgfx/Cargo.toml $(wildcard ../Lumgfx/src/*.rs) \
                ../Lumeth/Cargo.toml $(wildcard ../Lumeth/src/*.rs) \
                ../Lumouse/Cargo.toml $(wildcard ../Lumouse/src/*.rs) sdk/abi.rs
RUST_FLAGS := -C no-redzone=yes -C relocation-model=static -C code-model=small

.PHONY: all clean run debug build-init fat-image

all: $(IMAGE)

-include $(C_OBJ:.o=.d)

$(INIT_BIN): makefile $(wildcard lumen.conf) $(INIT_INPUTS)
	$(INIT_BUILD_COMMAND)

$(NVMED_BIN): $(NVMED_INPUTS)
	@$(MAKE) -C $(NVMED_DIR)

$(LPCID_BIN): makefile $(LPCID_INPUTS)
	@$(MAKE) -B -C $(LPCID_DIR) CC="$(CC) $(X86_CFLAGS)"

$(FAT_BIN): $(FAT_INPUTS)
	@$(MAKE) -C $(FAT_DIR)

$(BOOT): $(BOOT_DIR)/boot.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ $< -o $@

$(STAGE1): $(BOOT_DIR)/stage_1.s $(KERNEL)
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ -DKERNEL_SECTORS=$(KERNEL_SECTORS) $< -o $@

$(STAGE2): $(BOOT_DIR)/stage_2.s $(KERNEL)
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ -DKERNEL_SECTORS=$(KERNEL_SECTORS) $< -o $@

$(AP_BOOT): $(BOOT_DIR)/ap.s
	@mkdir -p $(dir $@)
	$(AS) -f bin $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.c $(INIT_BIN) makefile
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.s
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) $< -o $@

$(USER_HELLO_ELF): $(SRC_DIR)/user/hello/hello.c include/lumen/user/syslib.h $(SRC_DIR)/user/ld/prog.ld makefile
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -T $(SRC_DIR)/user/ld/prog.ld $(USER_LDFLAGS) $< -o $@

$(USER_HELLO_BIN): $(USER_HELLO_ELF)
	$(OBJCOPY) -O binary $< $@

$(SHELL_BIN): $(SHELL_INPUTS)
	$(SHELL_BUILD_COMMAND)

$(SHELL_BLOB_O): $(SHELL_BLOB_S) $(SHELL_BIN)
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -DSHELL_BIN=\"$(abspath $(SHELL_BIN))\" $< -o $@

$(BUILD)/user/blobs/hello_blob.o: $(USER_HELLO_BIN)

$(INIT_BLOB_O): $(INIT_BLOB_S) $(INIT_BIN)
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -DINIT_BIN=\"$(abspath $(INIT_BIN))\" $< -o $@

$(NVMED_BLOB_O): $(NVMED_BLOB_S) $(NVMED_BIN)
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -DNVMED_BIN=\"$(abspath $(NVMED_BIN))\" $< -o $@

$(LPCID_BLOB_O): $(LPCID_BLOB_S) $(LPCID_BIN)
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -DLPCID_BIN=\"$(abspath $(LPCID_BIN))\" $< -o $@

$(BUILD)/kernel/kernel.o: include/lumen/kernel/rust.h

$(RUST_LIB): $(RUST_SOURCES) $(AP_BOOT) $(FAT_BIN) makefile
	@mkdir -p $(dir $@)
	CARGO_TARGET_DIR=$(abspath $(RUST_CARGO_TARGET)) RUSTFLAGS="$(RUST_FLAGS)" \
	LUMEN_AP_BOOT="$(abspath $(AP_BOOT))" LUMEN_FAT_BIN="$(abspath $(FAT_BIN))" \
	$(CARGO) build --release --target x86_64-unknown-none --manifest-path $(RUST_DIR)/Cargo.toml
	cp $(RUST_CARGO_TARGET)/x86_64-unknown-none/release/liblumen_kernel.a $@

$(KERNEL): $(KERNEL_OBJ) $(RUST_LIB) linker.ld makefile
	@mkdir -p $(dir $@)
	$(LD) $(LDFLAGS) --gc-sections -o $(BUILD)/kernel.elf $(KERNEL_OBJ) $(RUST_LIB)
	$(OBJCOPY) -O binary $(BUILD)/kernel.elf $@

$(IMAGE): $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL)
	@mkdir -p $(dir $@)
	cat $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL) > $@
	@truncate -s $$(( ( $(KERNEL_LBA) + $(KERNEL_SECTORS) ) * 512 )) $@

FAT_IMAGE ?= $(BUILD)/fat32.img
QEMU_NVME_IMAGE ?= $(FAT_IMAGE)
QEMU_CPUS ?= 4
QEMU ?= qemu-system-x86_64
QEMU_MEMORY ?= 256M
QEMU_CPU ?= max
QEMU_NET ?= -nic user,model=rtl8139

fat-image: $(FAT_IMAGE)

$(FAT_IMAGE):
	@mkdir -p $(dir $@)
	mkfs.fat -C -F 32 -n LUMEN $@ 65536

QEMU_DEVICES := \
	-drive if=none,id=nvme0,file=$(QEMU_NVME_IMAGE),format=raw \
	-device nvme,drive=nvme0,serial=lumen0

ifneq ($(QEMU_NVME_IMAGE),$(FAT_IMAGE))
$(QEMU_NVME_IMAGE):
	@mkdir -p $(dir $@)
	truncate -s 64M $@
endif

QEMU_COMMON = -smp $(QEMU_CPUS) -m $(QEMU_MEMORY) -cpu $(QEMU_CPU) \
              -drive format=raw,file=$(IMAGE) $(QEMU_DEVICES) $(QEMU_NET) -no-reboot -no-shutdown

run: $(IMAGE) $(QEMU_NVME_IMAGE)
	$(QEMU) $(QEMU_COMMON) -serial stdio

debug: $(IMAGE) $(QEMU_NVME_IMAGE)
	$(QEMU) $(QEMU_COMMON) \
		-d int,cpu_reset,guest_errors \
		-D qemu.log \
		-s -S

clean:
	@if test -d "$(BUILD)"; then \
		find "$(abspath $(BUILD))" -mindepth 1 -maxdepth 1 -exec sh -c '\
			fat=$$1; nvme=$$2; shift 2; \
			for artifact do \
				case "$$fat" in "$$artifact"|"$$artifact"/*) continue;; esac; \
				case "$$nvme" in "$$artifact"|"$$artifact"/*) continue;; esac; \
				rm -rf -- "$$artifact" || exit; \
			done' clean "$(abspath $(FAT_IMAGE))" "$(abspath $(QEMU_NVME_IMAGE))" {} +; \
	fi

include libc/libc.mk
include ports/ports.mk
