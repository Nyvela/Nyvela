X11_WORK_DIR ?= $(abspath $(BUILD)/x11/upstream)
X11_JOBS ?= 8
X11_ROOT := $(X11_WORK_DIR)/root
X11_STAGED := $(X11_WORK_DIR)/.staged
X11_ARCHIVE := $(BUILD)/x11/lumen-x11.cpio
X11_INPUTS := $(wildcard ports/x11/*) sdk/abi.rs sdk/sys.rs tools/lumen-clang
X11_PROGRAMS := bin/Xvfb bin/jwm bin/xedit bin/xclock bin/lumdesk

.PHONY: x11 run-desktop

x11: $(X11_ARCHIVE)

$(X11_STAGED): $(X11_INPUTS) $(MUSL_READY) $(MUSL_BRIDGE)
	@mkdir -p $(dir $@)
	sh ports/x11/build.sh "$(CURDIR)" "$(X11_WORK_DIR)" "$(X11_JOBS)"
	touch $@

$(X11_ARCHIVE): $(X11_STAGED) $(PORT_ROOT)/bin/lumdesk
	@mkdir -p $(dir $@)
	cp "$(PORT_ROOT)/bin/lumdesk" "$(X11_ROOT)/bin/lumdesk"
	cd "$(X11_ROOT)" && { printf '%s\0' bin $(X11_PROGRAMS); find usr -print0; } | LC_ALL=C sort -z | \
		cpio --null --reproducible -o -H newc --quiet > "$(abspath $@).tmp"
	mv "$@.tmp" "$@"

run-desktop: $(IMAGE) $(QEMU_NVME_IMAGE) $(X11_ARCHIVE)
	$(QEMU) $(QEMU_COMMON) -fw_cfg name=opt/lumen/initrd,file=$(X11_ARCHIVE) \
		-fw_cfg name=opt/lumen/desktop,string=1 -serial stdio
