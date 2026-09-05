; void prints(char *ds:esi, byte color:ah);
; string must be null-terminated
; cursor is stored at 0xC700 for simplicity, may be updated later
; this assumes screen is 80x25, but will be updated later
prints:
  pusha
  cld ; clear direction flag so lodsb will increment ESI
  
  .loop:
    lodsb ; read byte into al

    test al, al
    jz .ret

    cmp byte [0xC700], 80
    jb .write

    call printnl
  
  .write:
    movzx ebx, word [0xC700] ; bl = column, bh = row
    
    movzx edx, bh
    imul edx, 80

    movzx edi, bl
    add edi, edx
    shl edi, 1
    
    add edi, 0xB8000

    mov byte [edi], al
    mov byte [edi + 1], ah
    
    inc bl
    mov [0xC700], bx
    
    jmp .loop

    .ret:
      popa
      ret

; void println(char *ds:esi, byte color:ah)
; wrapper for prints and printnl
println:
  call prints
  call printnl
  ret

; void printnl()
; prints newline
printnl:
  pusha
  cld

  mov byte [0xC700], 0 ; column = 0
  inc byte [0xC701] ; row++

  cmp byte [0xC701], 25
  jb .ret

  mov esi, 0xB8000 + 0xA0
  mov edi, 0xB8000
  mov ecx, 24 * 80

  rep movsw

  mov edi, 0xB8000 + 24 * 0xA0
  mov ax, 0x0720 ; ' ' with grey attribute
  mov ecx, 80

  rep stosw

  mov byte [0xC701], 24
  
  .ret:
    popa
    ret

; void _print_prefixed_str(char *ds:edi, char *ds:esi, byte colors:ax)
; al is used for prefix color, ah is used for string color. 
; prints string in format "[ <ds:di> ] <ds:si>". This also adds newline.
_print_prefixed_str:
  pusha

  mov ebx, esi
  mov cx, ax

  mov esi, part_lbrack
  mov ah, 0x0F
  call prints

  mov esi, edi
  mov ah, cl
  call prints

  mov esi, part_rbrack
  mov ah, 0x0F 
  call prints

  mov esi, ebx
  mov ah, ch
  call println
 
  popa
  ret

; void printfail(char *ds:esi, byte color:ah)
; prints string with "[ FAIL ]" prefix
printfail:
  mov al, 0x0C
  mov edi, part_fail
  jmp _print_prefixed_str

; void printinfo(char *ds:esi, byte color:ah)
; prints string with "[ INFO ]" prefix
printinfo:
  mov al, 0x09
  mov edi, part_info
  jmp _print_prefixed_str

; void printsucc(char *ds:esi, byte color:ah)
; prints string with "[ SUCC ]" prefix
printsucc:
  mov al, 0x0A
  mov edi, part_succ
  jmp _print_prefixed_str

; void printferr(char *ds:esi, byte color:ah)
; prints string with "[ FERR ]" prefix
printferr:
  mov al, 0x04
  mov edi, part_ferr
  jmp _print_prefixed_str

; void printinit(char *ds:esi, byte color:ah)
; prints string with "[ INIT ]" prefix
printinit:
  mov al, 0x0B
  mov edi, part_init
  jmp _print_prefixed_str

part_init: db "INIT", 0
part_info: db "INFO", 0
part_succ: db "SUCC", 0
part_fail: db "FAIL", 0
part_ferr: db "FERR", 0
part_lbrack: db "[ ", 0
part_rbrack: db " ] ", 0
