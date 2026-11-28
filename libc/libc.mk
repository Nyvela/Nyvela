MUSL_DIR ?= ../musl
MUSL_VERSION := v1.2.5
RUSTC ?= rustc
RUST_UNWIND := $(shell $(RUSTC) --print sysroot)/lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/libunwind.a
MUSL_SOURCE := $(abspath $(BUILD)/libc/source)
MUSL_BUILD := $(abspath $(BUILD)/libc/objects)
MUSL_SYSROOT := $(abspath $(BUILD)/libc/sysroot)
LUMEN_SYSROOT ?= $(MUSL_SYSROOT)
export LUMEN_SYSROOT
MUSL_READY := $(MUSL_SYSROOT)/.ready
MUSL_BRIDGE := $(MUSL_SYSROOT)/lib/liblumen_musl.a

.PHONY: libc

libc: $(MUSL_READY) $(MUSL_BRIDGE)

$(MUSL_SOURCE)/.patched: libc/musl.patch
	@mkdir -p $(MUSL_SOURCE)
	git -C $(MUSL_DIR) archive $(MUSL_VERSION) | tar -x -C $(MUSL_SOURCE)
	cd $(MUSL_SOURCE) && patch -p1 < $(abspath $<)
	touch $@

$(MUSL_BUILD)/config.mak: $(MUSL_SOURCE)/.patched libc/libc.mk
	@mkdir -p $(MUSL_BUILD)
	cd $(MUSL_BUILD) && $(MUSL_SOURCE)/configure --disable-shared --prefix=$(MUSL_SYSROOT) CC=clang

$(MUSL_READY): $(MUSL_BUILD)/config.mak $(MUSL_SOURCE)/.patched $(RUST_UNWIND)
	$(MAKE) -C $(MUSL_BUILD) install
	cp $(RUST_UNWIND) $(MUSL_SYSROOT)/lib/libunwind.a
	touch $@

$(MUSL_BRIDGE): libc/bridge.rs sdk/abi.rs sdk/sys.rs libc/libc.mk
	@mkdir -p $(dir $@)
	rustc --edition 2024 --target x86_64-unknown-none --crate-type staticlib -C panic=abort -C opt-level=s \
		-C relocation-model=static $< -o $@.tmp
	@panic_symbol=$$(nm --defined-only $@.tmp 2>/dev/null | awk '$$3 ~ /rust_begin_unwind$$/ {print $$3}'); \
	test -n "$$panic_symbol" && $(OBJCOPY) --redefine-sym "$$panic_symbol=__lumen_musl_panic" $@.tmp
	mv $@.tmp $@
