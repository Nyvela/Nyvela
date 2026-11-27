bits 16
org 0x7000

ap_start:
  cli
  cld
  xor ax, ax
  mov ds, ax
  lgdt [gdtr]
  mov eax, cr0
  or eax, 1
  mov cr0, eax
  jmp 0x08:protected

bits 32
protected:
  mov ax, 0x10
  mov ds, ax
  mov ss, ax
  mov eax, cr4
  or eax, 1 << 5                ; PAE
  mov cr4, eax
  mov eax, [page_table]
  mov cr3, eax
  mov ecx, 0xc0000080           ; IA32_EFER
  rdmsr
  or eax, 1 << 8                ; LME
  wrmsr
  mov eax, cr0
  or eax, 1 << 31               ; PG
  mov cr0, eax
  jmp 0x18:long_mode

bits 64
long_mode:
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov ss, ax
  mov eax, 1
  cpuid
  shr ebx, 24
  mov rsp, [abs stacks + rbx * 8]
  test rsp, rsp
  jz .halt
  mov edi, ebx
  call [rel entry]
.halt:
  cli
  hlt
  jmp .halt

align 8
gdt:
  dq 0
  dq 0x00cf9a000000ffff         ; 32-bit code
  dq 0x00cf92000000ffff         ; data
  dq 0x00af9a000000ffff         ; 64-bit code
.end:
gdtr:
  dw gdt.end - gdt - 1
  dd gdt

; The BSP fills these fields before it releases any AP from reset.
times 0x3f0 - ($ - $$) db 0
page_table: dq 0
entry: dq 0
stacks: times 256 dq 0
