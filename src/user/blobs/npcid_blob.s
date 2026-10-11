section .rodata

global npcid_blob_start
global npcid_blob_end

npcid_blob_start:
incbin NPCID_BIN
npcid_blob_end:
