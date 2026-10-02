section .rodata

global hello_blob_start
global hello_blob_end

hello_blob_start:
incbin "build/user/hello.bin"
hello_blob_end:
