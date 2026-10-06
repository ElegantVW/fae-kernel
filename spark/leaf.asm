; leaf — glean LEAF, write the page, read until q, exit.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent.
; House calls: glean 8, write 2, read 10, exit 1.
; Gleam name LEAF (cairn first, then the volume root). The page is the
; proof; this spark does not write its own name.

        bits    64
        default abs

start:
        mov     rax, 8                  ; glean "LEAF"
        lea     rdi, [rel name]
        lea     rsi, [rel buf]
        mov     rdx, 256
        int     0xE0
        test    rax, rax
        js      .fail
        mov     rdx, rax                ; write fd1, the page
        lea     rsi, [rel buf]
        mov     rdi, 1
        mov     rax, 2
        int     0xE0
.more:
        mov     rax, 10                 ; read fd 0, one byte
        xor     edi, edi
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     .fail
        cmp     byte [rel key], 'q'
        jne     .more
        mov     rax, 1                  ; exit(0)
        xor     edi, edi
        int     0xE0
        ud2
.fail:
        mov     rax, 1                  ; exit(1)
        mov     rdi, 1
        int     0xE0
        ud2

name:   db      "LEAF", 0
key:    db      0
buf:    times 256 db 0
