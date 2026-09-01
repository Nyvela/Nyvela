; void print(char *ds:si);
; string must be null-terminated
print:
  cld ; clear direction flag so lodsb will increment SI
  
  .loop:
    lodsb ; read byte into al

    test al, al
    jz .newline
  
    mov ah, 0x0E ; teletype output
    mov bh, 0 ; page
    mov bl, 0x0F ; white on black
    int 0x10

    jmp .loop

  .newline:
    mov ah, 0x03 ; get cursor
    int 0x10

    inc dh ; row
    xor dl, dl ; column
    
    mov ah, 0x02 ; set cursor
    int 0x10

    ret


