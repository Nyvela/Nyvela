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
  call print

  mov si, a20_msg
  call print

  ; Enable A20 via BIOS
  mov ax, 0x2403 ; Query A20 Gate Support
  int 0x15

  jc .a20_not_supported
  test ah, ah
  jnz .a20_not_supported
  
  jmp .get_a20_status

  .a20_not_supported:
    mov si, a20_not_supported_msg
    jmp .hlt

  .a20_failed:
    mov si, a20_fail_msg
    jmp .hlt

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
    call print
  
  mov si, gdt_msg
  call print
 
  ; Encode Null Entry
  mov edi, GDT_NULL_BASE
  mov esi, GDT_NULL_LIMIT
  mov ch, GDT_NULL_ACCESS_BYTE
  mov cl, GDT_NULL_FLAGS
  mov bx, GDT

  call encode_gdt_entry

  mov si, null_msg
  call print
  
  ; Encode Kernel Code Entry
  mov edi, GDT_KCODE_BASE
  mov esi, GDT_KCODE_LIMIT
  mov ch, GDT_KCODE_ACCESS_BYTE
  mov cl, GDT_KCODE_FLAGS
  mov bx, GDT + 8

  call encode_gdt_entry
  
  mov si, kcode_msg
  call print

  ; Encode Kernel Data Entry
  mov edi, GDT_KDATA_BASE
  mov esi, GDT_KDATA_LIMIT
  mov ch, GDT_KDATA_ACCESS_BYTE
  mov cl, GDT_KDATA_FLAGS
  mov bx, GDT + 16

  call encode_gdt_entry
  
  mov si, kdata_msg
  call print

  ; Encode User Code Entry
  mov edi, GDT_UCODE_BASE
  mov esi, GDT_UCODE_LIMIT
  mov ch, GDT_UCODE_ACCESS_BYTE
  mov cl, GDT_UCODE_FLAGS
  mov bx, GDT + 24

  call encode_gdt_entry
    
  mov si, ucode_msg
  call print
  
  ; Encode User Data Entry
  mov edi, GDT_UDATA_BASE
  mov esi, GDT_UDATA_LIMIT
  mov ch, GDT_UDATA_ACCESS_BYTE
  mov cl, GDT_UDATA_FLAGS
  mov bx, GDT + 32

  call encode_gdt_entry
  
  mov si, udata_msg
  call print

  mov si, gdt_setup_msg
  call print
  
  cli

  ; Set GDT table
  mov eax, GDT
  mov [GDTR + 2], eax

  mov ax, GDT_END - GDT - 1
  mov [GDTR], ax

  lgdt [GDTR]

  mov si, gdt_succ_msg
  call print

  mov si, prot_mode_msg
  call print
   
  ; Enable protected mode
  mov eax, cr0
  or eax, 1
  mov cr0, eax
  
  jmp dword 0x08:protected_mode_entry
  
  .hlt:
    hlt
    jmp .hlt

protected_mode_entry:
  ; Reload segments and jump to stage 2
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax

  jmp dword 0x8400

%include "print.s"

; void encode_gdt_entry(dword edi:base, dword esi:limit, byte ch:access_byte, byte cl:flags, word bx:gdt_entry_addr)
encode_gdt_entry:
  ; Encode the limit
  ; byte 0 = limit & 0xFF
  mov eax, esi
  mov [bx], al

  ; byte 1 = (limit >> 8) & 0xFF
  mov eax, esi
  shr eax, 8
  mov [bx + 1], al

  ; byte 6 = (limit >> 16) & 0x0F
  mov eax, esi 
  shr eax, 16
  and eax, 0x0F
  mov [bx + 6], al

  ; Encode the base
  ; byte 2 = base & 0xFF
  mov eax, edi
  mov [bx + 2], al

  ; byte 3 = (base >> 8) & 0xFF
  mov eax, edi
  shr eax, 8
  mov [bx + 3], al

  ; byte 4 = (base >> 16) & 0xFF
  mov eax, edi
  shr eax, 16
  mov [bx + 4], al

  ; byte 7 = (base >> 24) & 0xFF
  mov eax, edi
  shr eax, 24
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

GDT:
  dq 0 ; Null
  dq 0 ; Kernel Code
  dq 0 ; Kernel Data
  dq 0 ; User Code
  dq 0 ; User Data
GDT_END:

GDTR:
  dw 0 ; for limit storage
  dd 0 ; for base storage

success_msg: db "[ SUCC ] Entered Stage 1.", 0
a20_msg: db "[ INFO ] Enabling A20...", 0
a20_succ_msg: db "[ SUCC ] Enabled A20.", 0
a20_fail_msg: db "[ FERR ] Failed to enable A20.", 0
a20_not_supported_msg: db "[ FERR ] A20 is not supported.", 0
gdt_msg: db "[ INFO ] Preparing GDT table...", 0
null_msg: db "[ SUCC ] Encoded Null Entry to GDT.", 0
kcode_msg: db "[ SUCC ] Encoded Kernel Code Entry to GDT.", 0
kdata_msg: db "[ SUCC ] Encoded Kernel Data Entry to GDT.", 0
udata_msg: db "[ SUCC ] Encoded User Data Entry to GDT.", 0
ucode_msg: db "[ SUCC ] Encoded User Code Entry to GDT.", 0
gdt_setup_msg: db "[ INFO ] Setting GDT table...", 0
gdt_succ_msg: db "[ SUCC ] Set GDT table.", 0
prot_mode_msg: db "[ INFO ] Enabling protected mode and jumping to Stage 2...", 0

times 1024 - ($ - $$) db 0 ; pad to 2 sectors
