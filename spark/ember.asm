; ember — the live coal. Kindles a named spark and stays.
; G31: if splanc is packed, kindle it; spawn refuses, exit 1.
; G30: no splanc in the cairn, kindle wick, write stayed, exit
; with wick's last word.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent.
; Speaks house calls alone (spawn 5, write 2, exit 1).

        bits    64
        default abs

start:
        mov     rax, 5                  ; spawn "splanc" if packed
        lea     rdi, [rel splancname]
        int     0xE0
        test    rax, rax
        js      .splanc_miss
        jmp     .fail                   ; a last word from splanc is a lie

.splanc_miss:
        cmp     rax, -2                 ; -ENOENT: not in this cairn
        je      .wick
        cmp     rax, -19                ; -ENODEV: no cairn
        je      .wick
        jmp     .fail                   ; the spark went out; spawn refused

.wick:
        mov     rax, 5                  ; spawn "wick"
        lea     rdi, [rel wickname]
        int     0xE0
        test    rax, rax
        js      .fail
        mov     r12, rax                ; wick's last word
        mov     rax, 2                  ; write "stayed\n"
        mov     rdi, 1
        lea     rsi, [rel stayed]
        mov     edx, 7
        int     0xE0
        mov     rax, 1                  ; exit(wick's word)
        mov     rdi, r12
        int     0xE0
        ud2
.fail:
        mov     rax, 1                  ; exit(1)
        mov     rdi, 1
        int     0xE0
        ud2

splancname: db "splanc", 0
wickname:   db "wick", 0
stayed:     db "stayed", 10
