section .rodata

global shell_blob_start
global shell_blob_end

shell_blob_start:
incbin "build/user/shell.bin"
shell_blob_end:
