include nyvela.conf

ifndef INIT_DIR
$(error INIT_DIR is not configured. (see nyvela.conf))
endif

ifeq ($(wildcard $(INIT_DIR)/.),)
$(error init directory $(INIT_DIR) does not exist. (see nyvela.conf))
endif

ifndef INIT_BIN_NAME
$(error INIT_BIN_NAME is not configured. (see nyvela.conf))
endif

ifndef INIT_BUILD_COMMAND
$(error INIT_BUILD_COMMAND is not configured. (see nyvela.conf))
endif

ifndef SHELL_DIR
$(error SHELL_DIR is not configured. (see nyvela.conf))
endif

ifndef SHELL_BIN_NAME
$(error SHELL_BIN_NAME is not configured. (see nyvela.conf))
endif

ifndef SHELL_BUILD_COMMAND
$(error SHELL_BUILD_COMMAND is not configured. (see nyvela.conf))
endif

ifndef NVMED_DIR
$(error NVMED_DIR is not configured. (see nyvela.conf))
endif

ifndef NVMED_BIN_NAME
$(error NVMED_BIN_NAME is not configured. (see nyvela.conf))
endif

INIT_BIN := $(INIT_DIR)/build/$(INIT_BIN_NAME).bin
SHELL_BIN := $(SHELL_DIR)/build/$(SHELL_BIN_NAME).bin
INIT_INPUTS := $(shell find $(INIT_DIR)/src $(INIT_DIR)/include -type f 2>/dev/null) \
               $(INIT_DIR)/makefile $(INIT_DIR)/linker.ld

NVMED_BIN := $(NVMED_DIR)/build/$(NVMED_BIN_NAME).bin
NVMED_INPUTS := $(shell find $(NVMED_DIR)/src $(NVMED_DIR)/include -type f 2>/dev/null) \
                $(NVMED_DIR)/makefile $(NVMED_DIR)/linker.ld

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

# Derived from the linked kernel so the boot loaders and the image can never
# disagree about how many sectors the kernel occupies.
KERNEL_SECTORS = $(shell s=$$(stat -c%s $(KERNEL)); echo $$(( (s + 511) / 512 )))
KERNEL_LBA := 37

CFLAGS := -ffreestanding -m64 -mno-red-zone \
          -fno-stack-protector -fno-pie \
          -Wall -Wextra \
          -Iinclude -g

USER_CFLAGS := -ffreestanding -m64 -mno-red-zone \
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

ASFLAGS := -f elf64
LDFLAGS := -T linker.ld

KERNEL_C_SRC := $(shell find $(SRC_DIR) -type f -name '*.c' \
                   ! -path '$(SRC_DIR)/user/*')

KERNEL_ASM_SRC := $(shell find $(SRC_DIR) -type f -name '*.s' \
                   ! -path '$(BOOT_DIR)/*')

C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(KERNEL_C_SRC))
ASM_OBJ := $(patsubst $(SRC_DIR)/%.s,$(BUILD)/%.o,$(KERNEL_ASM_SRC))

KERNEL_OBJ := $(C_OBJ) $(ASM_OBJ)

.PHONY: all clean run debug build-init

all: $(IMAGE)

$(INIT_BIN): nyvela.conf $(INIT_INPUTS)
	$(INIT_BUILD_COMMAND)

$(NVMED_BIN): $(NVMED_INPUTS)
	@$(MAKE) -C $(NVMED_DIR)

$(BOOT): $(BOOT_DIR)/boot.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ $< -o $@

$(STAGE1): $(BOOT_DIR)/stage_1.s $(KERNEL)
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ -DKERNEL_SECTORS=$(KERNEL_SECTORS) $< -o $@

$(STAGE2): $(BOOT_DIR)/stage_2.s $(KERNEL)
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(BOOT_DIR)/ -DKERNEL_SECTORS=$(KERNEL_SECTORS) $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.c $(INIT_BIN)
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/%.o: $(SRC_DIR)/%.s
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) $< -o $@

$(USER_HELLO_ELF): $(SRC_DIR)/user/hello/hello.c include/nyvela/user/syslib.h $(SRC_DIR)/user/ld/prog.ld
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -T $(SRC_DIR)/user/ld/prog.ld $(USER_LDFLAGS) $< -o $@

$(USER_HELLO_BIN): $(USER_HELLO_ELF)
	$(OBJCOPY) -O binary $< $@

$(SHELL_BIN):
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

$(KERNEL): $(KERNEL_OBJ)
	@mkdir -p $(dir $@)
	$(LD) $(LDFLAGS) -o $(BUILD)/kernel.elf $(KERNEL_OBJ)
	$(OBJCOPY) -O binary $(BUILD)/kernel.elf $@

$(IMAGE): $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL)
	@mkdir -p $(dir $@)
	cat $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL) > $@
	@truncate -s $$(( ( $(KERNEL_LBA) + $(KERNEL_SECTORS) ) * 512 )) $@

run: $(IMAGE)
	qemu-system-x86_64 \
		-drive format=raw,file=$(IMAGE) \
		-d int,cpu_reset,guest_errors \
		-no-reboot -no-shutdown

debug: $(IMAGE)
	qemu-system-x86_64 \
		-drive format=raw,file=$(IMAGE) \
		-d int,cpu_reset,guest_errors \
		-D qemu.log \
		-no-reboot -no-shutdown \
		-s -S

clean:
	rm -rf $(BUILD)
