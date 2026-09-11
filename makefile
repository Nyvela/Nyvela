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

ASFLAGS := -f elf64
LDFLAGS := -T linker.ld

KERNEL_C_SRC := $(shell find $(SRC_DIR) -type f -name '*.c')

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

$(KERNEL): $(KERNEL_OBJ)
	@mkdir -p $(dir $@)
	$(LD) $(LDFLAGS) -o $(BUILD)/kernel.elf $(KERNEL_OBJ)
	$(OBJCOPY) -O binary $(BUILD)/kernel.elf $@

$(IMAGE): $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL)
	@mkdir -p $(dir $@)
	cat $(BOOT) $(STAGE1) $(STAGE2) $(KERNEL) > $@

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
