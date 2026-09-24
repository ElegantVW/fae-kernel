; cerne-fw — firmware + loader fused. 64KiB at 0xF0000.
; First glyph: Lilac (VGA 13). We are the BIOS: no int 10h / 15h.

        bits    16
        default abs

section .text start=0

start:
        cli
        mov     ax, 0xF000
        mov     ds, ax
        mov     es, ax
        xor     ax, ax
        mov     ss, ax
        mov     sp, 0x7000

        call    vga_mode3
        call    vga_lilac
        call    serial_init16
        mov     si, msg_ansi
        call    puts16
        mov     si, msg_fw
        call    puts16
        call    ivt16
        call    pic_init
        call    cmos_ram
        call    a20_fast

        lgdt    [gdt_ptr]
        mov     eax, cr0
        or      eax, 1
        mov     cr0, eax
        jmp     dword 0x08:pm32 + 0xF0000

; --- real-mode IVT 0..31 → trap16 in this ROM ---
ivt16:
        push    es
        push    di
        push    cx
        xor     ax, ax
        mov     es, ax
        xor     di, di
        mov     cx, 32
.fill:
        mov     ax, trap16
        stosw
        mov     ax, 0xF000
        stosw
        loop    .fill
        pop     cx
        pop     di
        pop     es
        ret

trap16:
        mov     ax, 0xF000
        mov     ds, ax
        mov     si, msg_fw_trap
        call    puts16
.hang:
        hlt
        jmp     .hang

; --- VGA 80×25 colour text (no BIOS) then DAC + logo ---
vga_wr:
        out     dx, al
        inc     dx
        mov     al, ah
        out     dx, al
        dec     dx
        ret

vga_mode3:
        pushad
        mov     dx, 0x3C2
        mov     al, 0x67
        out     dx, al
        mov     dx, 0x3C4
        mov     si, seq
        xor     bx, bx
.seq:
        mov     al, bl
        mov     ah, [si]
        call    vga_wr
        inc     si
        inc     bx
        cmp     bx, 5
        jb      .seq
        mov     dx, 0x3D4
        mov     al, 0x11
        out     dx, al
        inc     dx
        in      al, dx
        and     al, 0x7F
        out     dx, al
        dec     dx
        mov     si, crtc
        xor     bx, bx
.crtc:
        mov     al, bl
        mov     ah, [si]
        call    vga_wr
        inc     si
        inc     bx
        cmp     bx, 25
        jb      .crtc
        mov     dx, 0x3CE
        mov     si, gc
        xor     bx, bx
.gc:
        mov     al, bl
        mov     ah, [si]
        call    vga_wr
        inc     si
        inc     bx
        cmp     bx, 9
        jb      .gc
        mov     dx, 0x3DA
        in      al, dx
        mov     dx, 0x3C0
        mov     si, ac
        xor     bx, bx
.ac:
        mov     al, bl
        out     dx, al
        mov     al, [si]
        out     dx, al
        inc     si
        inc     bx
        cmp     bx, 21
        jb      .ac
        mov     al, 0x20
        out     dx, al
        popad
        ret

vga_lilac:
        push    ax
        push    cx
        push    dx
        push    si
        push    es
        push    di
        push    bx
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
        mov     ax, 0x0D20
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
        pop     bx
        pop     di
        pop     es
        pop     si
        pop     dx
        pop     cx
        pop     ax
        ret

cmos_read:
        or      al, 0x80
        out     0x70, al
        in      al, 0x71
        ret

cmos_ram:
        push    eax
        push    ebx
        push    ds
        xor     ax, ax
        mov     ds, ax
        mov     dword [0x8000], 0x50414D46
        mov     dword [0x800C], 0
        mov     al, 0x34
        call    cmos_read
        mov     bl, al
        mov     al, 0x35
        call    cmos_read
        mov     bh, al
        mov     eax, 16 * 1024 * 1024
        movzx   ebx, bx
        shl     ebx, 16
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
        shl     eax, 10
        add     eax, 1024 * 1024
.store:
        mov     [0x8008], eax
        xor     eax, 0x4C444E4B
%ifdef AUDIT_BAD_FMAP
        xor     eax, 1
%endif
        mov     [0x8004], eax
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

pic_init:
        mov     al, 0x11
        out     0x20, al
        out     0xA0, al
        mov     al, 0x20
        out     0x21, al
        mov     al, 0x28
        out     0xA1, al
        mov     al, 0x04
        out     0x21, al
        mov     al, 0x02
        out     0xA1, al
        mov     al, 0x01
        out     0x21, al
        out     0xA1, al
        mov     al, 0xFF
        out     0x21, al
        out     0xA1, al
        ret

a20_fast:
        in      al, 0x92
        test    al, 2
        jnz     .on
        or      al, 2
        out     0x92, al
.on:
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

        ; 32-bit IDT at 0x5000 — all gates → trap32
        mov     edi, 0x5000
        mov     ecx, 256
        mov     ebx, trap32 + 0xF0000
.fill32:
        mov     eax, ebx
        stosw
        mov     ax, 0x08
        stosw
        mov     ax, 0x8E00
        stosw
        mov     eax, ebx
        shr     eax, 16
        stosw
        loop    .fill32
        lidt    [idt32_ptr + 0xF0000]
%ifdef AUDIT_FW_TRAP
        ud2
%endif

        mov     edi, 0x1000
        mov     ecx, 0xC00
        xor     eax, eax
        rep     stosd
        mov     dword [0x1000], 0x2003
        mov     dword [0x2000], 0x3003
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

trap32:
        mov     ax, 0x10
        mov     ds, ax
        mov     es, ax
        mov     esi, msg_fw_trap + 0xF0000
.p32:
        lodsb
        test    al, al
        jz      .h32
        cmp     al, 10
        jne     .o32
        mov     al, 13
        call    putc32
        mov     al, 10
.o32:
        call    putc32
        jmp     .p32
.h32:
        hlt
        jmp     .h32

putc32:
        push    edx
        push    eax
        mov     dx, 0x3FD
.w:
        in      al, dx
        test    al, 0x20
        jz      .w
        pop     eax
        mov     dx, 0x3F8
        out     dx, al
        pop     edx
        ret

        bits    64
        default abs
lm64:
        mov     ax, 0x20
        mov     ds, ax
        mov     es, ax
        mov     ss, ax
        mov     fs, ax
        mov     gs, ax
        mov     rsp, 0x7000
        lgdt    [gdt_ptr64 + 0xF0000]

        ; 64-bit IDT at 0x6000 until Kindling lidt
        mov     rdi, 0x6000
        mov     rcx, 256
        mov     rbx, trap64 + 0xF0000
.fill64:
        mov     rax, rbx
        stosw
        mov     ax, 0x18
        stosw
        mov     ax, 0x8E00
        stosw
        mov     rax, rbx
        shr     rax, 16
        stosw
        shr     rax, 16
        stosd
        xor     eax, eax
        stosd
        loop    .fill64
        lidt    [idt64_ptr + 0xF0000]

        mov     rsi, 0xF0000 + msg_ld
        call    puts64
        cmp     dword [0x200000], 0x4C444E4B
        je      .have
        mov     rsi, 0xF0000 + msg_miss
        call    puts64
.hang:
        hlt
        jmp     .hang
.have:
        mov     rax, 0x200004
        jmp     rax

trap64:
        mov     ax, 0x20
        mov     ds, ax
        mov     rsi, 0xF0000 + msg_fw_trap
        call    puts64
.h64:
        hlt
        jmp     .h64

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

msg_ansi:    db 27, "[95m", 0
msg_fw:      db "cerne-fw", 10, 0
msg_ld:      db "cerne-ld", 10, 0
msg_miss:    db "kindling: no guest at 0x200000", 10, 0
msg_fw_trap: db "cerne-fw: trap", 10, 0
             db "kindling remembers the reset", 0
             db "the jump is the vow", 0

logo:   db "  *", 10, " /|\", 10, "/ | \", 10, "  |", 10, " / \", 10, "/   \", 10, 0

palette:
        db 6,4,6, 26,27,41, 35,47,38, 35,48,49
        db 57,39,44, 43,34,49, 57,47,27, 59,56,59
        db 18,14,18, 41,44,57, 45,54,45, 49,57,58
        db 59,45,49, 52,44,57, 59,53,39, 63,61,62

seq:    db 0x03, 0x00, 0x03, 0x00, 0x02
crtc:   db 0x5F,0x4F,0x50,0x82,0x55,0x81,0xBF,0x1F
        db 0x00,0x4F,0x0D,0x0E,0x00,0x00,0x00,0x50
        db 0x9C,0x0E,0x8F,0x28,0x1F,0x96,0xB9,0xA3,0xFF
gc:     db 0x00,0x00,0x00,0x00,0x00,0x10,0x0E,0x00,0xFF
ac:     db 0x00,0x01,0x02,0x03,0x04,0x05,0x14,0x07
        db 0x38,0x39,0x3A,0x3B,0x3C,0x3D,0x3E,0x3F
        db 0x0C,0x00,0x0F,0x08,0x00

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

gdt_ptr64:
        dw      gdt_end - gdt - 1
        dq      0xF0000 + gdt

idt32_ptr:
        dw      256 * 8 - 1
        dd      0x5000

idt64_ptr:
        dw      256 * 16 - 1
        dq      0x6000

section .reset start=0xFFF0
        bits    16
        default abs
        jmp     0xF000:start
        times   16-($-$$) db 0
