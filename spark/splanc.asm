; splanc — a spark that does not catch. Irish splanc, flint-fire.
; Flat binary, nasm -f bin, entry at offset 0. `ud2` is the whole
; tale: the house names the miss and the Light's spawn refuses.

        bits    64
        default abs

start:
        ud2
        mov     rax, 1                  ; unreachable if the trap lives
        xor     edi, edi
        int     0xE0
        ud2
