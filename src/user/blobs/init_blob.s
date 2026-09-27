; Embedded init blob: kernel publishes it as ramfs /bin/init.
; Built by makefile from build/user/init.bin (flat binary, see src/user/ld/prog.ld).

section .rodata

global init_blob_start
global init_blob_end

init_blob_start:
incbin "build/user/init.bin"
init_blob_end:
