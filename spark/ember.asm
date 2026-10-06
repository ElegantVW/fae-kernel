; ember — the live coal. Kindles wick, writes "stayed", exits with
; wick's last word. Flat binary, nasm -f bin, entry at offset 0.
; Position-independent: only RIP-relative addressing. Speaks house
; calls alone (spawn 5, write 2, exit 1). The G30 proof: the light
; remains while the wick burns, then spawn returns home.

        bits    64
        default abs

start:
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

wickname: db "wick", 0
stayed:   db "stayed", 10
