BITS 16
[org 0x8000]

stage_1:
  cli

  mov ax, 0x0800
  mov es, ax
  mov ss, ax
  
  xor ax, ax
  mov ds, ax
  
  sti

  mov si, success_msg
  mov ah, 0x0F
  call printsucc
  
  mov si, stage_2_load_msg
  mov ah, 0x0F
  call printinfo
  
  mov ah, 0x42 ; Read from disk
  mov si, DAP_stage_2
  mov dl, [0x7B00] ; load boot drive number

  int 0x13
  jc stage_2_disk_error
  
  mov si, stage_2_succ_msg
  mov ah, 0x0F
  call printsucc

  mov si, kernel_load_msg
  mov ah, 0x0F
  call printinfo

  mov ah, 0x42 ; Read from disk
  mov si, DAP_kernel
  mov dl, [0x7B00]

  int 0x13
  jc kernel_disk_error
  
  mov si, kernel_succ_msg
  mov ah, 0x0F
  call printsucc

  mov si, a20_msg
  mov ah, 0x0F
  call printinfo

  ; Enable A20 via BIOS
  mov ax, 0x2403 ; Query A20 Gate Support
  int 0x15

  jc .a20_not_supported
  test ah, ah
  jnz .a20_not_supported
  
  jmp .get_a20_status

  .a20_not_supported:
    mov si, a20_not_supported_msg
    mov ah, 0x0F
    call printferr

    jmp halt

  .a20_failed:
    mov si, a20_fail_msg
    mov ah, 0x0F
    call printferr

    jmp halt

  .get_a20_status:
    mov ax, 0x2402 ; Get A20 Gate Status
    int 0x15

    jc .a20_failed
    test ah, ah
    jnz .a20_failed

    test al, al
    jnz .a20_activated
  
  mov ax, 0x2401 ; Activate A20 Gate
  int 0x15

  jc .a20_failed
  test ah, ah
  jnz .a20_failed

  .a20_activated:
    mov si, a20_succ_msg
    mov ah, 0x0F
    call printsucc
  
  mov si, gdt_msg
  mov ah, 0x0F
  call printinfo
 
  ; Encode Null Entry
  xor edi, edi
  xor esi, esi
  mov ch, GDT_NULL_ACCESS_BYTE
  mov cl, GDT_NULL_FLAGS
  mov bx, GDT

  call encode_gdt_entry

  mov si, null_msg
  mov ah, 0x0F
  call printsucc
  
  ; Encode Kernel Code Entry
  mov edi, GDT_KCODE_BASE
  mov esi, GDT_KCODE_LIMIT
  mov ch, GDT_KCODE_ACCESS_BYTE
  mov cl, GDT_KCODE_FLAGS
  mov bx, GDT + 8

  call encode_gdt_entry
  
  mov si, kcode_msg
  mov ah, 0x0F
  call printsucc

  ; Encode Kernel Data Entry
  mov edi, GDT_KDATA_BASE
  mov esi, GDT_KDATA_LIMIT
  mov ch, GDT_KDATA_ACCESS_BYTE
  mov cl, GDT_KDATA_FLAGS
  mov bx, GDT + 16

  call encode_gdt_entry
  
  mov si, kdata_msg
  mov ah, 0x0F
  call printsucc

  ; Encode User Code Entry
  mov edi, GDT_UCODE_BASE
  mov esi, GDT_UCODE_LIMIT
  mov ch, GDT_UCODE_ACCESS_BYTE
  mov cl, GDT_UCODE_FLAGS
  mov bx, GDT + 24

  call encode_gdt_entry
    
  mov si, ucode_msg
  mov ah, 0x0F
  call printsucc
  
  ; Encode User Data Entry
  mov edi, GDT_UDATA_BASE
  mov esi, GDT_UDATA_LIMIT
  mov ch, GDT_UDATA_ACCESS_BYTE
  mov cl, GDT_UDATA_FLAGS
  mov bx, GDT + 32

  call encode_gdt_entry
  
  mov si, udata_msg
  mov ah, 0x0F
  call printsucc
  
  mov edi, GDT_KCODE_LM_BASE
  mov esi, GDT_KCODE_LM_LIMIT
  mov ch, GDT_KCODE_LM_ACCESS_BYTE
  mov cl, GDT_KCODE_LM_FLAGS
  mov bx, GDT + 40

  call encode_gdt_entry
  
  mov si, kcode_lm_msg
  mov ah, 0x0F
  call printsucc

  mov si, gdt_setup_msg
  mov ah, 0x0F
  call printinfo
  
  cli

  ; Set GDT table
  mov eax, GDT
  mov [GDTR + 2], eax

  mov ax, GDT_END - GDT - 1
  mov [GDTR], ax

  lgdt [GDTR]

  mov si, gdt_succ_msg
  mov ah, 0x0F
  call printsucc

  mov si, prot_mode_msg
  mov ah, 0x0F
  call printinfo
 
  ; Enable protected mode
  mov eax, cr0
  or eax, 1
  mov cr0, eax
  
  jmp 0x08:protected_mode_entry

  stage_2_disk_error:
    mov si, stage_2_err_msg
    mov ah, 0x0F
    call printferr
    jmp halt

  kernel_disk_error:
    mov si, kernel_err_msg
    mov ah, 0x0F
    call printferr
    jmp halt
  
  halt:
    hlt
    jmp halt

protected_mode_entry:
  BITS 32
  
  ; Reload segments and jump to stage 2
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax

  jmp dword 0x8600

BITS 16

%include "print.s"

; void encode_gdt_entry(dword edi:base, dword esi:limit, byte ch:access_byte, byte cl:flags, word bx:gdt_entry_addr)
encode_gdt_entry:
  ; Encode the limit
  ; byte 0 = limit & 0xFF
  mov eax, esi
  mov [bx], al

  ; byte 1 = (limit >> 8) & 0xFF
  shr eax, 8
  mov [bx + 1], al

  ; byte 6 = (limit >> 16) & 0x0F
  shr eax, 8
  and eax, 0x0F
  mov [bx + 6], al

  ; Encode the base
  ; byte 2 = base & 0xFF
  mov eax, edi
  mov [bx + 2], al

  ; byte 3 = (base >> 8) & 0xFF
  shr eax, 8
  mov [bx + 3], al

  ; byte 4 = (base >> 16) & 0xFF
  shr eax, 8
  mov [bx + 4], al

  ; byte 7 = (base >> 24) & 0xFF
  shr eax, 8
  mov [bx + 7], al
  
  ; Encode the access byte
  mov [bx + 5], ch

  ; Encode the flags
  ; byte 6 |= (flags << 4)
  shl cl, 4
  or [bx + 6], cl
  
  ret

GDT_NULL_BASE equ 0x0
GDT_NULL_LIMIT equ 0x0
GDT_NULL_ACCESS_BYTE equ 0x0
GDT_NULL_FLAGS equ 0x0

GDT_KCODE_BASE equ 0x0
GDT_KCODE_LIMIT equ 0xFFFFF
GDT_KCODE_ACCESS_BYTE equ 0x9A
GDT_KCODE_FLAGS equ 0xC

GDT_KDATA_BASE equ 0x0
GDT_KDATA_LIMIT equ 0xFFFFF
GDT_KDATA_ACCESS_BYTE equ 0x92
GDT_KDATA_FLAGS equ 0xC

GDT_UCODE_BASE equ 0x0
GDT_UCODE_LIMIT equ 0xFFFFF
GDT_UCODE_ACCESS_BYTE equ 0xFA
GDT_UCODE_FLAGS equ 0xC

GDT_UDATA_BASE equ 0x0
GDT_UDATA_LIMIT equ 0xFFFFF
GDT_UDATA_ACCESS_BYTE equ 0xF2
GDT_UDATA_FLAGS equ 0xC

GDT_KCODE_LM_BASE equ 0x0
GDT_KCODE_LM_LIMIT equ 0xFFFFF
GDT_KCODE_LM_ACCESS_BYTE equ 0x9A
GDT_KCODE_LM_FLAGS equ 0xA

GDT:
  dq 0 ; Null
  dq 0 ; Kernel Code
  dq 0 ; Kernel Data
  dq 0 ; User Code
  dq 0 ; User Data
  dq 0 ; Kernel Code (x64)
GDT_END:

GDTR:
  dw 0 ; for limit storage
  dd 0 ; for base storage

success_msg: db "Entered Stage 1.", 0
stage_2_load_msg: db "Loading Stage 2 to memory...", 0
stage_2_succ_msg: db "Loaded Stage 2 to memory.", 0
stage_2_err_msg: db "Failed to load Stage 2 to memory.", 0
kernel_load_msg: db "Loading Kernel to memory...", 0
kernel_succ_msg: db "Loaded Kernel to memory.", 0
kernel_err_msg: db "Failed to load Kernel to memory.", 0
a20_msg: db "Enabling A20...", 0
a20_succ_msg: db "Enabled A20.", 0
a20_fail_msg: db "Failed to enable A20.", 0
a20_not_supported_msg: db "A20 is not supported.", 0
gdt_msg: db "Preparing GDT...", 0
null_msg: db "Encoded Null Entry to GDT.", 0
kcode_msg: db "Encoded Kernel Code Entry to GDT.", 0
kdata_msg: db "Encoded Kernel Data Entry to GDT.", 0
udata_msg: db "Encoded User Data Entry to GDT.", 0
ucode_msg: db "Encoded User Code Entry to GDT.", 0
kcode_lm_msg: db "Encoded Kernel Code (x64) Entry to GDT.", 0
gdt_setup_msg: db "Setting GDT...", 0
gdt_succ_msg: db "Set GDT.", 0
prot_mode_msg: db "Entering protected mode...", 0

DAP_stage_2: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10 ; Size of packet
  db 0x0 ; Always 0 for some reason
  dw 0x0020 ; 32 sectors to read, 16384 bytes size cap for stage 2
  
  ; Physical address for where to load data, long jump here
  dw 0x0000 ; offset
  dw 0x0860 ; segment
  
  dq 0x04 ; LBA

DAP_kernel: ; Disk Address Packet, required for BIOS's INT13h extensions
  db 0x10
  db 0x0
  dw 0x0011 ; 17 sectors to read, 8704 bytes size cap for kernel

  dw 0x0000
  dw 0x1000 ; load to 0x10000, copy to 0x100000 in long mode
  dq 0x24 

times 3 * 512 - ($ - $$) db 0 ; pad to 3 sectors
