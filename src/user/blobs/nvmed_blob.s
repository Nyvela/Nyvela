section .rodata

global nvmed_blob_start
global nvmed_blob_end

nvmed_blob_start:
incbin NVMED_BIN
nvmed_blob_end:
