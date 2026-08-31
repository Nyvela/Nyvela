CC := gcc
AS := nasm
LD := ld
OBJCOPY := objcopy

SRC_DIR := src
BUILD := build

BOOT := $(BUILD)/boot.bin
KERNEL := $(BUILD)/kernel.bin
IMAGE := $(BUILD)/os.img

CFLAGS := -ffreestanding -m64 -mno-red-zone \
          -fno-stack-protector -fno-pie \
          -Wall -Wextra

ASFLAGS := -f elf64

LDFLAGS := -T linker.ld

C_SRC := $(shell find $(SRC_DIR) -type f -name '*.c')
ASM_SRC := $(shell find $(SRC_DIR) -type f -name '*.s' ! -name 'boot.s')

C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD)/%.o,$(C_SRC))
ASM_OBJ := $(patsubst $(SRC_DIR)/%.s,$(BUILD)/%.o,$(ASM_SRC))

KERNEL_OBJ := $(C_OBJ) $(ASM_OBJ)

.PHONY: all clean run

all: $(IMAGE)

$(BOOT): $(SRC_DIR)/boot.s
	@mkdir -p $(dir $@)
	$(AS) -f bin $< -o $@

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

$(IMAGE): $(BOOT) $(KERNEL)
	@mkdir -p $(dir $@)
	cat $(BOOT) $(KERNEL) > $@

run: $(IMAGE)
	qemu-system-x86_64 \
		-drive format=raw,file=$(IMAGE)

clean:
	rm -rf $(BUILD)
