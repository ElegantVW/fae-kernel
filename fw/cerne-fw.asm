; cerne-fw — the firmware. 64KiB at 0xF0000. The loader is not fused any more:
; we read it off the boot disk (LBA 1..32 -> 0x9000, 'LDOK' trailer proves the
; slot landed) and hand over in long mode — ld/cerne-ld.asm, docs/FIRMWARE.md.
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

        call    load_loader
        jnc     .ld_ok
        mov     si, msg_miss
        call    puts16
.hang16:
        hlt
        jmp     .hang16
.ld_ok:

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
        push    bx
        xor     ax, ax
        mov     es, ax
        xor     di, di
        xor     bx, bx
        mov     cx, 32
.fill:
        mov     ax, trap16_stubs
        add     ax, bx
        stosw
        mov     ax, 0xF000
        stosw
        add     bx, 5
        loop    .fill
        pop     bx
        pop     cx
        pop     di
        pop     es
        ret

trap16_common:
        mov     ah, 0
        push    ax
        mov     ax, 0xF000
        mov     ds, ax
        mov     si, msg_fw_trap
        call    puts16
        pop     ax
        call    putdec8
        mov     al, 10
        call    putc16
.hang:
        hlt
        jmp     .hang

putdec8:
        aam
        add     ax, 0x3030
        push    ax
        mov     al, ah
        call    putc16
        pop     ax
        call    putc16
        ret

trap16_stubs:
%assign i 0
%rep 32
        mov     al, i
        jmp     strict near trap16_common
%assign i i+1
%endrep

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
        mov     [0x8008], eax                   ; ram_end
        mov     dword [0x800C], 2               ; nreg
        mov     dword [0x8010], 0
        mov     dword [0x8014], 0x9F000
        mov     dword [0x8018], 0x100000
        cmp     eax, 0x100000
        jae     .ext
        xor     eax, eax
        jmp     .len1
.ext:
        sub     eax, 0x100000
.len1:
        mov     [0x801C], eax
        mov     eax, [0x8008]
        xor     eax, 2
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

; --- load the loader: LBA 1..32 of the boot disk -> 0x9000.
; The slot trailer 'LDOK' at 0xCFFC (written by scripts/mkimg.py) proves the
; whole slot arrived. On failure this prints its own cause and returns CF=1;
; the caller prints the house line and halts.
load_loader:
        push    es
        push    di
        push    si
        push    bx
        push    ax
        xor     ax, ax
        mov     es, ax                  ; dest ES:DI = 0000:9000
        mov     bl, 3                   ; attempts
.try:
        mov     di, 0x9000
        mov     dx, 0x1F6
        mov     al, 0xE0                ; drive 0, LBA 27:24 = 0
        out     dx, al
        mov     dx, 0x3F6               ; 400ns settle
        in      al, dx
        in      al, dx
        in      al, dx
        in      al, dx
        mov     dx, 0x1F7
        in      al, dx
        test    al, al
        jz      .nodisk                 ; floating bus: nothing behind the port
        cmp     al, 0xFF
        je      .nodisk
        mov     dx, 0x1F2
        mov     al, 32                  ; the whole 16KiB slot
        out     dx, al
        mov     al, 1                   ; LBA 1
        mov     dx, 0x1F3
        out     dx, al
        xor     al, al
        mov     dx, 0x1F4
        out     dx, al
        mov     dx, 0x1F5
        out     dx, al
        mov     dx, 0x1F7
        mov     al, 0x20                ; READ SECTORS
        out     dx, al
        mov     si, 32                  ; sectors left
.sector:
        call    ata_poll16
        jc      .fail
        mov     cx, 256
        mov     dx, 0x1F0
        cld
.word:
        in      ax, dx
        stosw
        loop    .word
        dec     si
        jnz     .sector
        cmp     dword [es:0xCFFC], 0x4B4F444C   ; 'LDOK'
        jne     .nomarker
        pop     ax
        pop     bx
        pop     si
        pop     di
        pop     es
        clc
        ret
.fail:
        dec     bl
        jz      .nodisk
        mov     dx, 0x3F6               ; SRST the channel, retry
        mov     al, 0x04
        out     dx, al
        in      al, dx
        in      al, dx
        xor     al, al
        out     dx, al
        jmp     .try
.nomarker:
        mov     si, msg_noloader
        call    puts16
        jmp     .done
.nodisk:
        mov     si, msg_nodisk
        call    puts16
.done:
        pop     ax
        pop     bx
        pop     si
        pop     di
        pop     es
        stc
        ret

; --- ata_poll16: wait for the next sector. CF = 1 on timeout / ERR / DF.
ata_poll16:
        push    ax
        push    bx
        push    cx
        mov     bx, 64                  ; ~4M polls: spin-up is slow on metal
.outer:
        mov     cx, 0xFFFF
.wait:
        mov     dx, 0x1F7
        in      al, dx
        test    al, 0x80                ; BSY
        jnz     .again
        test    al, 0x21                ; ERR | DF
        jnz     .bad
        test    al, 0x08                ; DRQ
        jnz     .ok
.again:
        loop    .wait
        dec     bx
        jnz     .outer
.bad:
        pop     cx
        pop     bx
        pop     ax
        stc
        ret
.ok:
        pop     cx
        pop     bx
        pop     ax
        clc
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

        ; 32-bit IDT at 0x5000 — vectors 0–31 named stubs
        mov     edi, 0x5000
        mov     ebx, trap32_stubs + 0xF0000
        mov     ecx, 32
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
        add     ebx, 10
        loop    .fill32
        mov     ecx, 224
        mov     ebx, trap32_common + 0xF0000
.fill32b:
        mov     eax, ebx
        stosw
        mov     ax, 0x08
        stosw
        mov     ax, 0x8E00
        stosw
        mov     eax, ebx
        shr     eax, 16
        stosw
        loop    .fill32b
        lidt    [idt32_ptr + 0xF0000]
%ifdef AUDIT_FW_TRAP
        int     6
%endif

        mov     edi, 0x1000
        mov     ecx, 0x1000
        xor     eax, eax
        rep     stosd
        mov     dword [0x1000], 0x2003
        mov     dword [0x2000], 0x3003
        mov     dword [0x3000], 0x4003          ; PD[0] → 4K PT
        mov     edi, 0x4000
        xor     ebx, ebx
        mov     ecx, 512
.fillpt:
        mov     eax, ebx
        or      eax, 3
        cmp     ebx, 0xA0000
        jb      .rampt
        cmp     ebx, 0x100000
        jae     .rampt
        or      eax, 0x18                       ; PCD|PWT hole
.rampt:
        mov     [edi], eax
        add     edi, 8
        add     ebx, 0x1000
        loop    .fillpt
        mov     eax, [0x8008]
        add     eax, 0x1FFFFF
        shr     eax, 21
        cmp     eax, 2
        jb      .skip2m
        dec     eax
        cmp     eax, 511
        jbe     .cap
        mov     eax, 511
.cap:
        mov     ecx, eax
        mov     edi, 0x3008
        mov     eax, 0x200000 | 0x83
.fillpd:
        mov     [edi], eax
        add     edi, 8
        add     eax, 0x200000
        loop    .fillpd
.skip2m:

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

trap32_stubs:
%assign i 0
%rep 32
        mov     eax, i
        jmp     strict near trap32_common
%assign i i+1
%endrep

trap32_common:
        mov     ax, 0x10
        mov     ds, ax
        mov     es, ax
        push    eax
        mov     esi, msg_fw_trap + 0xF0000
.p32:
        lodsb
        test    al, al
        jz      .num32
        cmp     al, 10
        jne     .o32
        jmp     .p32
.o32:
        call    putc32
        jmp     .p32
.num32:
        pop     eax
        call    putdec32
        mov     al, 10
        call    putc32
.h32:
        hlt
        jmp     .h32

putdec32:
        aam
        add     ax, 0x3030
        push    eax
        mov     al, ah
        call    putc32
        pop     eax
        call    putc32
        ret

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
        mov     rcx, 32
        mov     rbx, trap64_stubs + 0xF0000
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
        add     rbx, 10
        loop    .fill64
        mov     rcx, 224
        mov     rbx, trap64_common + 0xF0000
.fill64b:
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
        loop    .fill64b
        lidt    [idt64_ptr + 0xF0000]

        ; the loader is a guest now: it earned the jump (ld/cerne-ld.asm)
        mov     rax, 0x9000
        jmp     rax

trap64_stubs:
%assign i 0
%rep 32
        mov     edi, i
        jmp     strict near trap64_common
%assign i i+1
%endrep

trap64_common:
        mov     ax, 0x20
        mov     ds, ax
        push    rdi
        mov     rsi, 0xF0000 + msg_fw_trap
        call    puts64
        pop     rdi
        mov     rax, rdi
        call    putdec64
        mov     al, 10
        call    putc64
.h64:
        hlt
        jmp     .h64

putdec64:
        push    rbx
        push    rdx
        xor     rdx, rdx
        mov     rbx, 10
        div     rbx
        add     al, '0'
        push    rdx
        call    putc64
        pop     rdx
        mov     al, dl
        add     al, '0'
        call    putc64
        pop     rdx
        pop     rbx
        ret

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
msg_nodisk:  db "cerne-fw: no disk", 10, 0
msg_noloader: db "cerne-fw: no loader", 10, 0
msg_miss:    db "kindling: no guest at 0x200000", 10, 0
msg_fw_trap: db "cerne-fw: trap ", 0
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
