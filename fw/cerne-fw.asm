; cerne-fw — firmware + loader fused (phase 0)
; 64KiB ROM at 0xF0000. File offsets are 0-based; linear = 0xF0000 + offset.

        org     0
        bits    16

start:
        cli
        mov     ax, 0xF000
        mov     ds, ax
        mov     es, ax
        xor     ax, ax
        mov     ss, ax
        mov     sp, 0x7000

        call    serial_init16
        mov     si, msg_fw
        call    puts16

        call    a20_fast

        lgdt    [gdt_ptr]

        mov     eax, cr0
        or      eax, 1
        mov     cr0, eax
        jmp     dword 0x08:pm32 + 0xF0000

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

        mov     edi, 0x3000
        mov     eax, 0x83
        mov     ecx, 512
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

        mov     rax, 0x200000
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

msg_fw: db      "cerne-fw", 10, 0
msg_ld: db      "cerne-ld", 10, 0

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
        times   0xFFF0-($-$$) db 0
reset:
        jmp     0xF000:start
        times   0x10000-($-$$) db 0
