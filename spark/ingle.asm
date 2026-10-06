; ingle — greeter spark, the desktop. Writes its name, reads a line, says
; the fire is lit, kindles leaf once, writes the light remains when that
; spark smoors, reads until q, exits. Flat binary, nasm -f bin, entry 0.
; House calls: write 2, read 10, spawn 5, exit 1.

        bits    64
        default abs

start:
        mov     rax, 2                  ; write "ingle\n"
        mov     rdi, 1
        lea     rsi, [rel hello]
        mov     rdx, 6
        int     0xE0

        xor     ebx, ebx
.more:
        mov     rax, 10                 ; read fd 0, one byte
        xor     edi, edi
        lea     rsi, [rel buf]
        add     rsi, rbx
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     .fail
        mov     al, [rsi]
        cmp     al, 10
        je      .got
        inc     rbx
        cmp     rbx, 255
        jb      .more
.got:
        mov     rax, 2                  ; write "the fire is lit\n"
        mov     rdi, 1
        lea     rsi, [rel lit]
        mov     rdx, 16
        int     0xE0

        mov     rax, 5                  ; spawn "leaf"
        lea     rdi, [rel leafname]
        int     0xE0
        test    rax, rax
        js      .waitq
        mov     rax, 2                  ; write "the light remains\n"
        mov     rdi, 1
        lea     rsi, [rel home]
        mov     rdx, 18
        int     0xE0
.waitq:
        mov     rax, 10                 ; read until q
        xor     edi, edi
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     .fail
        cmp     byte [rel key], 'q'
        jne     .waitq
        mov     rax, 1                  ; exit(0)
        xor     edi, edi
        int     0xE0
        ud2
.fail:
        mov     rax, 1                  ; exit(1)
        mov     rdi, 1
        int     0xE0
        ud2

hello:  db      "ingle", 10
lit:    db      "the fire is lit", 10
home:   db      "the light remains", 10
leafname: db    "leaf", 0
key:    db      0
buf:    times 256 db 0
