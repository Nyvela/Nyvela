; Embedded hello blob: kernel publishes it as ramfs /bin/hello.
; Built by makefile from build/user/hello.bin (flat binary, see src/user/ld/prog.ld).

section .rodata

global hello_blob_start
global hello_blob_end

hello_blob_start:
incbin "build/user/hello.bin"
hello_blob_end:
