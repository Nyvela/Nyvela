# Host tools, source checkouts and optional replacements for distro paths.
SQLITE_DIR ?= ../sqlite
DOOM_DIR ?= ../doomgeneric
DOOM_IWAD ?= /usr/share/games/doom/freedoom1.wad
LUMEN_RUSTC := $(CURDIR)/tools/lumen-rustc
LUMEN_CLANG := $(CURDIR)/tools/lumen-clang
SQLITE_BUILD := $(BUILD)/ports/sqlite
DOOM_BUILD := $(BUILD)/ports/doom
DOOM_SOURCE := $(DOOM_DIR)/doomgeneric
PORT_ROOT := $(BUILD)/ports/root
PORT_ARCHIVE := $(BUILD)/ports/lumen.cpio
DESKTOP_TARGET := $(abspath $(BUILD)/desktop/cargo)
DESKTOP_INPUTS := $(wildcard desktop/src/*.rs) desktop/build.rs desktop/Cargo.toml desktop/Cargo.lock \
                  sdk/abi.rs sdk/sys.rs $(FAT_DIR)/src/disk.rs

SQLITE_FLAGS := -DSQLITE_THREADSAFE=0 -DSQLITE_OMIT_LOAD_EXTENSION \
                -DSQLITE_OMIT_WAL -DSQLITE_TEMP_STORE=3 \
                '-DSQLITE_DEFAULT_UNIX_VFS="unix-dotfile"'

DOOM_EXCLUDE := doomgeneric_%.c i_allegro%.c i_sdl%.c gusconf.c icon.c mus2mid.c
DOOM_FILES := $(filter-out $(DOOM_EXCLUDE),$(notdir $(wildcard $(DOOM_SOURCE)/*.c)))
DOOM_OBJECTS := $(addprefix $(DOOM_BUILD)/,$(DOOM_FILES:.c=.o))
DOOM_FLAGS := -O2 -ffunction-sections -fdata-sections -std=gnu17 -D_DEFAULT_SOURCE \
              -DNORMALUNIX -DLINUX -DSNDSERV

.PHONY: ports run-ports run-lumdesk

ports: $(PORT_ARCHIVE)

$(PORT_ROOT)/bin/hello-std: ports/hello-std.rs tools/lumen-rustc tools/lumen-clang $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	$(LUMEN_RUSTC) -C opt-level=s -C strip=symbols $< -o $@

$(SQLITE_BUILD)/Makefile: $(SQLITE_DIR)/configure ports/ports.mk
	@mkdir -p $(SQLITE_BUILD)
	cd $(SQLITE_BUILD) && $(abspath $(SQLITE_DIR)/configure) --disable-shared --disable-readline --disable-tcl
	touch $@

$(SQLITE_BUILD)/sqlite3.c $(SQLITE_BUILD)/shell.c &: $(SQLITE_BUILD)/Makefile
	$(MAKE) -C $(SQLITE_BUILD) sqlite3.c shell.c
	touch $(SQLITE_BUILD)/sqlite3.c $(SQLITE_BUILD)/shell.c

$(PORT_ROOT)/bin/sqlite3: $(SQLITE_BUILD)/shell.c $(SQLITE_BUILD)/sqlite3.c tools/lumen-clang ports/ports.mk $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	$(LUMEN_CLANG) -O2 -s -ffunction-sections -fdata-sections $(SQLITE_FLAGS) $(filter %.c,$^) -o $@

$(DOOM_BUILD)/%.o: $(DOOM_SOURCE)/%.c tools/lumen-clang ports/ports.mk $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	$(LUMEN_CLANG) $(DOOM_FLAGS) -MMD -MP -c $< -o $@

-include $(DOOM_OBJECTS:.o=.d)

$(DOOM_BUILD)/lumen-platform.o: ports/doom.rs tools/lumen-rustc sdk/abi.rs sdk/sys.rs
	@mkdir -p $(dir $@)
	$(LUMEN_RUSTC) -C opt-level=2 -C panic=abort --crate-type lib --emit=obj $< -o $@

$(PORT_ROOT)/bin/doom: $(DOOM_BUILD)/lumen-platform.o $(DOOM_OBJECTS) tools/lumen-clang $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	$(LUMEN_CLANG) -s $(filter %.o,$^) -o $@

$(PORT_ROOT)/freedoom1.wad: $(DOOM_IWAD)
	@mkdir -p $(dir $@)
	cp $< $@

$(PORT_ROOT)/bin/lumdesk: $(DESKTOP_INPUTS) tools/lumen-cargo tools/lumen-clang $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	CARGO_TARGET_DIR=$(DESKTOP_TARGET) LUMEN_DISK_SOURCE=$(abspath $(FAT_DIR)/src/disk.rs) \
		tools/lumen-cargo build --locked --release --manifest-path desktop/Cargo.toml
	cp $(DESKTOP_TARGET)/x86_64-unknown-linux-musl/release/lumdesk $@

$(PORT_ARCHIVE): $(PORT_ROOT)/bin/hello-std $(PORT_ROOT)/bin/sqlite3 \
                 $(PORT_ROOT)/bin/doom $(PORT_ROOT)/bin/lumdesk $(PORT_ROOT)/freedoom1.wad
	cd $(PORT_ROOT) && printf '%s\0' bin bin/hello-std bin/sqlite3 bin/doom bin/lumdesk freedoom1.wad | \
		cpio --null --reproducible -o -H newc --quiet > $(abspath $@).tmp
	mv $@.tmp $@

run-ports: $(IMAGE) $(QEMU_NVME_IMAGE) $(PORT_ARCHIVE)
	$(QEMU) $(QEMU_COMMON) -fw_cfg name=opt/lumen/initrd,file=$(PORT_ARCHIVE) -serial stdio

run-lumdesk: $(IMAGE) $(QEMU_NVME_IMAGE) $(PORT_ARCHIVE)
	$(QEMU) $(QEMU_COMMON) -fw_cfg name=opt/lumen/initrd,file=$(PORT_ARCHIVE) \
		-fw_cfg name=opt/lumen/desktop,string=1 -serial stdio
