; cerne-fw — firmware + loader fused
; 64KiB ROM at 0xF0000. First glyph is Lilac (VGA index 13).

        org     0
        bits    16
        default abs

start:
        cli
        mov     ax, 0xF000
        mov     ds, ax
        mov     es, ax
        xor     ax, ax
        mov     ss, ax
        mov     sp, 0x7000

        call    vga_lilac
        call    serial_init16
        mov     si, msg_ansi
        call    puts16
        mov     si, msg_fw
        call    puts16

        call    cmos_ram
        call    a20_fast

        lgdt    [gdt_ptr]
        mov     eax, cr0
        or      eax, 1
        mov     cr0, eax
        jmp     dword 0x08:pm32 + 0xF0000

; --- VGA: DAC 16 + first glyph lilac '*' ---
vga_lilac:
        push    ax
        push    cx
        push    dx
        push    si
        push    es
        push    di
        mov     dx, 0x3C8
        xor     al, al
        out     dx, al
        mov     dx, 0x3C9
        mov     si, palette
        mov     cx, 16 * 3
.dac:
        lodsb
        out     dx, al
        loop    .dac
        mov     ax, 0xB800
        mov     es, ax
        xor     di, di
        mov     cx, 80 * 25
        mov     ax, 0x0D20              ; space, attr lilac-on-night
        cld
        rep     stosw
        xor     bx, bx
        mov     si, logo
.line:
        mov     ax, 160
        mul     bx
        mov     di, ax
.ch:
        lodsb
        test    al, al
        jz      .done
        cmp     al, 10
        je      .nl
        mov     ah, 0x0D
        stosw
        jmp     .ch
.nl:
        inc     bx
        jmp     .line
.done:
        pop     di
        pop     es
        pop     si
        pop     dx
        pop     cx
        pop     ax
        ret

; --- CMOS RAM size → FMAP at 0x8000 (we are the BIOS; no int 0x15) ---
cmos_read:
        or      al, 0x80                ; keep NMI off (we hold the machine)
        out     0x70, al
        in      al, 0x71
        ret

cmos_ram:
        push    eax
        push    ebx
        push    ds
        xor     ax, ax
        mov     ds, ax
        mov     dword [0x8000], 0x50414D46      ; 'FMAP'
        mov     dword [0x8004], 0
        mov     dword [0x800C], 0
        mov     al, 0x34
        call    cmos_read
        mov     bl, al
        mov     al, 0x35
        call    cmos_read
        mov     bh, al                          ; BX = 64KiB units above 16 MiB
        mov     eax, 16 * 1024 * 1024
        movzx   ebx, bx
        shl     ebx, 16                         ; * 65536
        add     eax, ebx
        cmp     eax, 16 * 1024 * 1024
        jne     .store
        mov     al, 0x30
        call    cmos_read
        mov     bl, al
        mov     al, 0x31
        call    cmos_read
        mov     bh, al
        movzx   eax, bx
        shl     eax, 10                         ; KB → bytes
        add     eax, 1024 * 1024
.store:
        mov     [0x8008], eax
        pop     ds
        pop     ebx
        pop     eax
        ret

serial_init16:
        mov     dx, 0x3F9
        xor     al, al
        out     dx, al
        mov     dx, 0x3FB
        mov     al, 0x80
        out     dx, al
        mov     dx, 0x3F8
        mov     al, 0x03
        out     dx, al
        mov     dx, 0x3F9
        xor     al, al
        out     dx, al
        mov     dx, 0x3FB
        mov     al, 0x03
        out     dx, al
        mov     dx, 0x3FA
        mov     al, 0xC7
        out     dx, al
        mov     dx, 0x3FC
        mov     al, 0x0B
        out     dx, al
        ret

putc16:
        push    dx
        push    ax
        mov     dx, 0x3FD
.wait:
        in      al, dx
        test    al, 0x20
        jz      .wait
        pop     ax
        mov     dx, 0x3F8
        out     dx, al
        pop     dx
        ret

puts16:
        lodsb
        test    al, al
        jz      .done
        cmp     al, 10
        jne     .out
        mov     al, 13
        call    putc16
        mov     al, 10
.out:
        call    putc16
        jmp     puts16
.done:
        ret

a20_fast:
        in      al, 0x92
        or      al, 2
        out     0x92, al
        ret

        bits    32
pm32:
        mov     ax, 0x10
        mov     ds, ax
        mov     es, ax
        mov     ss, ax
        mov     fs, ax
        mov     gs, ax
        mov     esp, 0x7000

        mov     edi, 0x1000
        mov     ecx, 0xC00
        xor     eax, eax
        rep     stosd

        mov     dword [0x1000], 0x2003
        mov     dword [0x2000], 0x3003

        ; Map only CMOS ram_end (2 MiB pages), not a fake 1 GiB.
        mov     eax, [0x8008]
        add     eax, 0x1FFFFF
        shr     eax, 21
        cmp     eax, 1
        jae     .pages
        mov     eax, 1
.pages:
        cmp     eax, 512
        jbe     .cap
        mov     eax, 512
.cap:
        mov     ecx, eax
        mov     edi, 0x3000
        mov     eax, 0x83
.fillpd:
        mov     [edi], eax
        add     edi, 8
        add     eax, 0x200000
        loop    .fillpd

        mov     eax, 0x1000
        mov     cr3, eax

        mov     eax, cr4
        or      eax, 1 << 5
        mov     cr4, eax

        mov     ecx, 0xC0000080
        rdmsr
        or      eax, 1 << 8
        wrmsr

        mov     eax, cr0
        or      eax, 1 << 31
        mov     cr0, eax

        jmp     0x18:lm64 + 0xF0000

        bits    64
lm64:
        mov     ax, 0x20
        mov     ds, ax
        mov     es, ax
        mov     ss, ax
        mov     fs, ax
        mov     gs, ax
        mov     rsp, 0x7000

        mov     rsi, 0xF0000 + msg_ld
        call    puts64

        cmp     dword [0x200000], 0x4C444E4B     ; 'KNDL'
        je      .have
        mov     rsi, 0xF0000 + msg_miss
        call    puts64
.hang:
        hlt
        jmp     .hang
.have:
        mov     rax, 0x200004
        jmp     rax

puts64:
        lodsb
        test    al, al
        jz      .done
        cmp     al, 10
        jne     .out
        mov     al, 13
        call    putc64
        mov     al, 10
.out:
        call    putc64
        jmp     puts64
.done:
        ret

putc64:
        push    rdx
        push    rax
        mov     dx, 0x3FD
.wait:
        in      al, dx
        test    al, 0x20
        jz      .wait
        pop     rax
        mov     dx, 0x3F8
        out     dx, al
        pop     rdx
        ret

; ANSI 16-color bright magenta = palette 13 Lilac (host serial)
msg_ansi: db    27, "[95m", 0
msg_fw:   db    "cerne-fw", 10, 0
msg_ld:   db    "cerne-ld", 10, 0
msg_miss: db    "kindling: no guest at 0x200000", 10, 0
        db      "kindling remembers the reset", 0
        db      "the jump is the vow", 0

logo:   db      "  *", 10
        db      " /|\", 10
        db      "/ | \", 10
        db      "  |", 10
        db      " / \", 10
        db      "/   \", 10, 0

        ; VGA 6-bit RGB, index 0..15 (see docs/identity/IDENTITY.md)
palette:
        db      6, 4, 6
        db      26, 27, 41
        db      35, 47, 38
        db      35, 48, 49
        db      57, 39, 44
        db      43, 34, 49
        db      57, 47, 27
        db      59, 56, 59
        db      18, 14, 18
        db      41, 44, 57
        db      45, 54, 45
        db      49, 57, 58
        db      59, 45, 49
        db      52, 44, 57              ; 13 Lilac
        db      59, 53, 39
        db      63, 61, 62

        align   8
gdt:
        dq      0
        dq      0x00CF9A000000FFFF
        dq      0x00CF92000000FFFF
        dq      0x00AF9A000000FFFF
        dq      0x00AF92000000FFFF
gdt_end:

gdt_ptr:
        dw      gdt_end - gdt - 1
        dd      0xF0000 + gdt

        bits    16
        default abs
        times   0xFFF0-($-$$) db 0
reset:
        jmp     0xF000:start
        times   0x10000-($-$$) db 0
