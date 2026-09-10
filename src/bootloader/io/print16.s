; void prints(char *ds:si, byte color:ah);
; string must be null-terminated
; cursor is stored at 0xC800 for simplicity, may be updated later
; this assumes screen is 80x25, but will be updated later
prints:
  pusha
  cld ; clear direction flag so lodsb will increment SI
  
  mov dx, 0xB800
  mov es, dx

  .loop:
    lodsb ; read byte into al

    test al, al
    jz .ret

    push ax
    
    mov bx, [0xC800] ; bl = column, bh = row
    
    xor dx, dx
    mov dl, bh
    mov ax, 80
    mul dx ; ax = row * 80

    mov dl, bl
    add ax, dx
    shl ax, 1
    
    mov di, ax
    pop ax
   
    mov byte es:[di], al
    mov byte es:[di + 1], ah
    
    inc bl

    cmp bl, 80 ; check if at last column
    jb .save_cursor

    xor bl, bl ; column = 0
    inc bh ; row++
     
    .save_cursor:
      mov [0xC800], bx
    
    jmp .loop

    .ret:
      popa
      ret

; void println(char *ds:si, byte color:ah)
; wrapper for prints and printnl
println:
  call prints
  call printnl
  ret

; void printnl()
; prints newline
printnl:
  inc byte [0xC801] ; row++
  mov byte [0xC800], 0 ; column = 0
  ret

; void _print_prefixed_str(char *ds:di, char *ds:si, byte colors:ax)
; al is used for prefix color, ah is used for string color. 
; prints string in format "[ <ds:di> ] <ds:si>". This also adds newline.
_print_prefixed_str:
  pusha

  mov bx, si
  mov cx, ax

  mov si, part_lbrack
  mov ah, 0x0F
  call prints

  mov si, di
  mov ah, cl
  call prints

  mov si, part_rbrack
  mov ah, 0x0F 
  call prints

  mov si, bx
  mov ah, ch
  call println
 
  popa
  ret

; void printfail(char *ds:si, byte color:ah)
; prints string with "[ FAIL ]" prefix
printfail:
  mov al, 0x0C
  mov di, part_fail
  jmp _print_prefixed_str

; void printinfo(char *ds:si, byte color:ah)
; prints string with "[ INFO ]" prefix
printinfo:
  mov al, 0x09
  mov di, part_info
  jmp _print_prefixed_str

; void printsucc(char *ds:si, byte color:ax)
; prints string with "[ SUCC ]" prefix
printsucc:
  mov al, 0x0A
  mov di, part_succ
  jmp _print_prefixed_str

; void printferr(char *ds:si, byte color:ax)
; prints string with "[ FERR ]" prefix
printferr:
  mov al, 0x04
  mov di, part_ferr
  jmp _print_prefixed_str

; void printinit(char *ds:si, byte color:ax)
; prints string with "[ INIT ]" prefix
printinit:
  mov al, 0x0B
  mov di, part_init
  jmp _print_prefixed_str

part_init: db "INIT", 0
part_info: db "INFO", 0
part_succ: db "SUCC", 0
part_fail: db "FAIL", 0
part_ferr: db "FERR", 0
part_lbrack: db "[ ", 0
part_rbrack: db " ] ", 0
