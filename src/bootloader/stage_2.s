BITS 32
[org 0x8600]

stage_2:
  cli
 
  mov esi, prot_succ_msg
  mov ah, 0x0F
  call printinfo

  mov esi, idt_setup_msg
  mov ah, 0x0F
  call printinfo
  
  mov edi, isr_de ; #DE handler
  xor bx, bx ; offset
  call idt_set_gate

  mov esi, isr_de_msg
  mov ah, 0x0F
  call printsucc

  mov edi, isr_gp ; #GP handler
  mov bx, 13 * 16 ; offset
  call idt_set_gate

  mov esi, isr_gp_msg
  mov ah, 0x0F
  call printsucc

  mov esi, idt_setting_msg
  mov ah, 0x0F
  call printinfo

  lidt [IDTR]

  mov esi, idt_setup_succ_msg
  mov ah, 0x0F
  call printsucc

  mov esi, paging_disable_msg
  mov ah, 0x0F
  call printinfo
  
  ; Disable paging
  mov ebx, cr0
  and ebx, ~(1 << 31)
  mov cr0, ebx
  
  mov esi, paging_disabled_msg
  mov ah, 0x0F
  call printsucc
  
  mov esi, pae_enable_msg
  mov ah, 0x0F
  call printinfo

  ; Enable PAE
  mov edx, cr4
  or edx, (1 << 5)
  mov cr4, edx

  mov esi, pae_enabled_msg
  mov ah, 0x0F
  call printsucc

  mov esi, setting_lme_msg
  mov ah, 0x0F
  call printinfo
  
  ; Set LME
  mov ecx, 0xC0000080
  rdmsr 
  or eax, (1 << 8)
  wrmsr

  mov esi, set_lme_msg
  mov ah, 0x0F
  call printsucc

  mov esi, updating_pml4
  mov ah, 0x0F
  call printinfo

  mov esi, preparing_pml4_table
  mov ah, 0x0F
  call printinfo
  
  ; pml4_table[0] = pdpt_table
  mov eax, pdpt_table
  or eax, 0x03 ; present | writable
  mov [pml4_table], eax
  
  mov esi, prepared_pml4_table
  mov ah, 0x0F
  call printsucc
  
  mov esi, preparing_pdpt_table
  mov ah, 0x0F
  call printinfo
  
  ; pdpt_table[0] = pd_table
  mov eax, pd_table
  or eax, 0x03
  mov [pdpt_table], eax

  mov esi, prepared_pdpt_table
  mov ah, 0x0F
  call printsucc
  
  mov esi, mapping_pd_table
  mov ah, 0x0F
  call printinfo
  
  mov eax, [0xC700]
  push eax

  mov edi, pd_table
  xor eax, eax ; physical address = 0
  mov ecx, 512 

  .map:
    mov edx, eax
    or edx, 0x83 ; Present | Writable | Page Size
    mov [edi], edx
    mov dword [edi + 4], 0

    add eax, 0x200000 ; next 2 MiB
    add edi, 8
    loop .map

  pop eax
  mov [0xC700], eax
 
  mov esi, mapped_pd_table
  mov ah, 0x0F
  call printsucc

  ; Updating PML4 table
  mov eax, pml4_table
  mov cr3, eax

  mov esi, updated_pml4
  mov ah, 0x0F 
  call printsucc
  
  mov esi, entering_long_mode
  mov ah, 0x0F
  call printinfo

  ; Enable paging
  mov ebx, cr0
  or ebx, (1 << 31)
  mov cr0, ebx
  
  ; Update segments
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax
 
  jmp 0x28:long_mode_enter

  hlt

BITS 64

long_mode_enter:
  mov rsp, 0x80000

  ; Copy Kernel from 0x10000 to 0x100000
  mov rsi, 0x10000
  mov rdi, 0x100000
  mov rcx, KERNEL_SIZE_IN_SECTORS * 512 / 8 
  cld
  rep movsq
  
  mov rax, 0x100000
  jmp rax

BITS 32

; void idt_set_gate(dword address:edi, word offset:bx)
idt_set_gate:
  mov word [IDT + bx + 0], di
  mov word [IDT + bx + 2], 0x28
  mov byte [IDT + bx + 4], 0x0
  mov byte [IDT + bx + 5], 0x8E

  shr edi, 16
  mov word [IDT + bx + 6], di
  
  shr edi, 16
  mov dword [IDT + bx + 8], edi
  mov dword [IDT + bx + 12], 0

  ret

%include "print32.s"
%include "idt.s"

prot_succ_msg: db "Entered protected mode successfully.", 0
idt_setup_msg: db "Preparing IDT...", 0
isr_de_msg: db "Encoded #DE entry.", 0
isr_gp_msg: db "Encoded #GP entry.", 0
idt_setting_msg: db "Setting IDT...", 0
idt_setup_succ_msg: db "Set IDT.", 0
paging_disable_msg: db "Disabling paging...", 0
paging_disabled_msg: db "Disabled paging.", 0
pae_enable_msg: db "Enabling PAE...", 0
pae_enabled_msg: db "Enabled PAE.", 0
setting_lme_msg: db "Setting LME...", 0
set_lme_msg: db "Set LME.", 0
preparing_pml4_table: db "Preparing PML4 table...", 0
prepared_pml4_table: db "Prepared PML4 table.", 0
preparing_pdpt_table: db "Preparing PDPT table...", 0
prepared_pdpt_table: db "Prepared PDPT table.", 0
mapping_pd_table: db "Mapping PD table...", 0
mapped_pd_table: db "Mapped PD table.", 0
updating_pml4: db "Updating PML4 table...", 0
updated_pml4: db "Updated PML4 table.", 0
entering_long_mode: db "Entering long mode...", 0
entered_long_mode: db "Entered long mode.", 0

KERNEL_SIZE_IN_SECTORS equ 17

IDT: ; Interrupt Descriptor Table
  times 14 dq 0, 0
IDT_END:

IDTR:
  dw IDT_END - IDT - 1
  dd IDT
  dd 0 ; high 32 bits for 64-bit lidt (zero-extended)

times ((0x1000 - (($ - $$ + 0x8600) % 0x1000)) % 0x1000) db 0
pml4_table:
  times 512 dq 0

pdpt_table:
  times 512 dq 0

pd_table:
  times 512 dq 0

times 16384 - ($ - $$) db 0 
