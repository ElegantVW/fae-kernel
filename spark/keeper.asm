; keeper — Setup Assistant. Reads a name, inks the hand, exits.
; Flat binary, nasm -f bin, entry 0. Position-independent.
; House calls: write 2, read 10, stow 9, exit 1.
; The hand's measure is 32 (G36 exact-measure). Wax is 32 zeros.
; An empty line refuses. Next boot ingle greets the name.

        bits    64
        default abs

start:
        mov     rax, 2                  ; write "keeper\n"
        mov     rdi, 1
        lea     rsi, [rel hello]
        mov     rdx, 7
        int     0xE0
        mov     rax, 2                  ; write "who keeps this fire\n"
        mov     rdi, 1
        lea     rsi, [rel ask]
        mov     rdx, 20
        int     0xE0

        cld
        lea     rdi, [rel namebuf]
        mov     ecx, 32
        xor     eax, eax
        rep     stosb

        xor     ebx, ebx
.more:
        mov     rax, 10                 ; read fd 0, one byte
        xor     edi, edi
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     .fail
        mov     al, [rel key]
        cmp     al, 10
        je      .got
        cmp     rbx, 31
        jae     .more                   ; cap 31; extra keys wait for newline
        lea     rdi, [rel namebuf]
        mov     [rdi + rbx], al
        inc     rbx
        jmp     .more
.got:
        cmp     byte [rel namebuf], 0
        je      .fail
        mov     rax, 9                  ; stow "hand", 32 bytes
        lea     rdi, [rel handname]
        lea     rsi, [rel namebuf]
        mov     rdx, 32
        int     0xE0
        test    rax, rax
        js      .fail
        mov     rax, 1                  ; exit(0)
        xor     edi, edi
        int     0xE0
        ud2
.fail:
        mov     rax, 1                  ; exit(1)
        mov     rdi, 1
        int     0xE0
        ud2

hello:  db      "keeper", 10
ask:    db      "who keeps this fire", 10
handname: db    "hand", 0
key:    db      0
namebuf: times 32 db 0
