; Embedded shell blob: kernel copies it to USER_CODE_VIRT (0x400000).
; Built by makefile from build/user/shell.bin (flat binary, see src/user/ld/shell.ld).

section .rodata

global shell_blob_start
global shell_blob_end

shell_blob_start:
incbin "build/user/shell.bin"
shell_blob_end:
