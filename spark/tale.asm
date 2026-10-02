; tale — the first spark. Speaks a leaf, inks the slate, then exits.
; Flat binary, nasm -f bin, entry at offset 0. Position-independent:
; only RIP-relative addressing. Speaks house calls alone (glean 8,
; stow 9, write 2, exit 1) — docs/CAIRN.md.
;
; First it tells first-leaf (the G18 proof, kept). Then it gleans the slate:
; still sealing-wax ("wax waits") it stows "ink holds", tells the slate and
; says "stowed"; already inked it just tells the slate and says "kept".
; A second boot on the same disk must say "kept" — that is the proof the ink
; survived on iron, not in RAM.

        bits    64
        default abs

start:
        mov     rax, 8                  ; glean "first-leaf" into buf
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
        mov     rax, 8                  ; glean "slate" into buf
        lea     rdi, [rel slatename]
        lea     rsi, [rel buf]
        mov     rdx, 256
        int     0xE0
        cmp     rax, 10                 ; the slate's measure is ten
        jne     .fail
        mov     r12, rax
        cld
        lea     rsi, [rel buf]
        lea     rdi, [rel wax]
        mov     ecx, 10
        repe    cmpsb
        je      .stow                   ; still wax — ink it
.kept:
        mov     rax, 2                  ; "kept"
        mov     rdi, 1
        lea     rsi, [rel m_kept]
        mov     rdx, 5
        int     0xE0
        mov     rax, 2                  ; tell the slate as it stands
        mov     rdi, 1
        lea     rsi, [rel buf]
        mov     rdx, r12
        int     0xE0
        mov     rax, 1                  ; exit(0)
        xor     edi, edi
        int     0xE0
        ud2
.stow:
        mov     rax, 9                  ; stow "ink holds" into slate
        lea     rdi, [rel slatename]
        lea     rsi, [rel ink]
        mov     rdx, 10
        int     0xE0
        test    rax, rax
        js      .fail
        mov     rax, 8                  ; glean it back (RAM coherent?)
        lea     rdi, [rel slatename]
        lea     rsi, [rel buf]
        mov     rdx, 256
        int     0xE0
        test    rax, rax
        js      .fail
        mov     r12, rax
        mov     rax, 2                  ; "stowed"
        mov     rdi, 1
        lea     rsi, [rel m_stowed]
        mov     rdx, 7
        int     0xE0
        mov     rax, 2                  ; tell the fresh ink
        mov     rdi, 1
        lea     rsi, [rel buf]
        mov     rdx, r12
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

leafname: db "first-leaf", 0
slatename: db "slate", 0
wax:    db "wax waits", 10
ink:    db "ink holds", 10
m_stowed: db "stowed", 10
m_kept: db "kept", 10
buf:    times 256 db 0
