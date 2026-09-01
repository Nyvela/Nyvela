CC := gcc
AS := nasm
LD := ld
OBJCOPY := objcopy

SRC_DIR := src
BUILD := build

BOOT := $(BUILD)/boot.bin
STAGE1 := $(BUILD)/stage_1.bin
STAGE2 := $(BUILD)/stage_2.bin
KERNEL := $(BUILD)/kernel.bin
IMAGE := $(BUILD)/os.img

CFLAGS := -ffreestanding -m64 -mno-red-zone \
          -fno-stack-protector -fno-pie \
          -Wall -Wextra

ASFLAGS := -f elf64
LDFLAGS := -T linker.ld

KERNEL_C_SRC := $(shell find $(SRC_DIR)/kernel -type f -name '*.c')
KERNEL_ASM_SRC := $(shell find $(SRC_DIR)/kernel -type f -name '*.s')

C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(KERNEL_C_SRC))
ASM_OBJ := $(patsubst $(SRC_DIR)/%.s,$(BUILD)/%.o,$(KERNEL_ASM_SRC))

KERNEL_OBJ := $(C_OBJ) $(ASM_OBJ)

.PHONY: all clean run

all: $(IMAGE)

$(BOOT): $(SRC_DIR)/bootloader/boot.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(SRC_DIR)/bootloader/ $< -o $@

$(STAGE1): $(SRC_DIR)/bootloader/stage_1.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(SRC_DIR)/bootloader/ $< -o $@

$(STAGE2): $(SRC_DIR)/bootloader/stage_2.s
	@mkdir -p $(dir $@)
	$(AS) -f bin -I$(SRC_DIR)/bootloader/ $< -o $@

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
		-drive format=raw,file=$(IMAGE)

clean:
	rm -rf $(BUILD)
