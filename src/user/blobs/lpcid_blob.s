section .rodata

global lpcid_blob_start
global lpcid_blob_end

lpcid_blob_start:
incbin LPCID_BIN
lpcid_blob_end:
