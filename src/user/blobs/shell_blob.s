section .rodata

global shell_blob_start
global shell_blob_end

shell_blob_start:
incbin SHELL_BIN
shell_blob_end:
