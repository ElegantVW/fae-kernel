; tale — the first spark. Speaks a leaf, then exits.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent:
; only RIP-relative addressing. Speaks house calls alone (glean 8,
; write 2, exit 1) — docs/CAIRN.md.

        bits    64
        default abs

start:
        mov     rax, 8                  ; glean "first-leaf" into buf
        lea     rdi, [rel name]
        lea     rsi, [rel buf]
        mov     rdx, 256
        int     0xE0
        test    rax, rax
        js      .fail                   ; -errno: no leaf told
        mov     rdx, rax                ; write fd1, buf, bytes
        lea     rsi, [rel buf]
        mov     rdi, 1
        mov     rax, 2
        int     0xE0
        mov     rax, 1                  ; exit(0)
        xor     edi, edi
        int     0xE0
        ud2
.fail:
        mov     rax, 1                  ; exit(1)
        mov     rdi, 1
        int     0xE0
        ud2

name:   db "first-leaf", 0
buf:    times 256 db 0
