section .rodata

global init_blob_start
global init_blob_end

init_blob_start:
incbin INIT_BIN
init_blob_end:
