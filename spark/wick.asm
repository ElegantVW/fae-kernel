; wick — the second spark. Tells second-leaf, then exits.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent:
; only RIP-relative addressing. Speaks house calls alone (glean 8,
; write 2, exit 1). The G20 proof: Kindling spawned it onto a private
; cup and a private CR3; it still gleans a leaf and speaks.

        bits    64
        default abs

start:
        mov     rax, 8                  ; glean "second-leaf" into buf
        lea     rdi, [rel leafname]
        lea     rsi, [rel buf]
        mov     rdx, 256
        int     0xE0
        test    rax, rax
        js      .fail
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

leafname: db "second-leaf", 0
buf:    times 256 db 0
