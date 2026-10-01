; cerne-ld — the loader, no longer fused into the ROM.
; Flat binary, nasm -f bin. The firmware reads LBA 1..32 of the boot disk to
; 0x9000 and jumps here in long mode; the slot trailer 'LDOK' (written by
; scripts/mkimg.py at the end of the 16KiB slot) proves the whole slot landed.
;
; Contract in  (docs/FIRMWARE.md): long mode, flat DS/ES/SS = 0x20, RSP 0x7000,
;   identity map (0-2MiB 4K pages, 2MiB pages above), serial 0x3F8 ready,
;   FMAP at 0x8000. RAM map: we own 0x9000-0xCFFF (ourselves) and 0x8400
;   (KMAP scratch); both are free RAM once the guest takes the jump.
; Contract out: kernel per KMAP at 0x200000, its XOR and 'KNDL' both checked,
;   then the cairn per KMAP+20/24 at 0x100000 when packed, jump 0x200004.
;   Failure is honest: cause line, then the house line
;   "kindling: no guest at 0x200000", then hlt. Never half a jump.

        bits    64
        default abs
        org     0x9000

KMAP            equ     0x8400          ; KMAP scratch (one sector)
KERNEL          equ     0x200000        ; guest slot
CAIRN           equ     0x100000        ; cairn slot (leaves + sparks)
CHUNK           equ     128             ; sectors per READ SECTORS command
KMAP_MAGIC      equ     0x50414D4B      ; 'KMAP'
KNDL_MAGIC      equ     0x4C444E4B      ; 'KNDL'

kstart:
        cli
        mov     ax, 0x20
        mov     ds, ax
        mov     es, ax
        mov     ss, ax
        mov     rsp, 0x7000
        mov     rsi, msg_ld
        call    puts

        ; --- KMAP (LBA 0) -> 0x8400
        xor     eax, eax
        mov     cl, 1
        mov     rdi, KMAP
        call    ata_read
        jc      .nodisk
        cmp     dword [KMAP], KMAP_MAGIC
        jne     .badkmap
        mov     eax, [KMAP + 8]                 ; kernel_lba
        xor     eax, [KMAP + 12]                ; ^ kernel_sectors
        xor     eax, [KMAP + 16]                ; ^ kernel_xor
        xor     eax, KMAP_MAGIC                 ; ^ 'KMAP'
        cmp     eax, [KMAP + 4]
        jne     .badkmap

        ; --- the kernel, straight to 0x200000, CHUNK sectors at a time
        mov     r12d, [KMAP + 8]                ; next lba
        mov     r13d, [KMAP + 12]               ; sectors left
        mov     r14d, CHUNK
        mov     rdi, KERNEL
.load:
        test    r13d, r13d
        jz      .loaded
        cmp     r13d, r14d
        jae     .cnt
        mov     r14d, r13d
.cnt:
        mov     eax, r12d
        mov     ecx, r14d
        call    ata_read
        jc      .nodisk
        mov     eax, r14d
        shl     eax, 9
        add     rdi, rax
        add     r12d, r14d
        sub     r13d, r14d
        mov     r14d, CHUNK
        jmp     .load

.loaded:
        ; --- kernel_xor over the padded image, dword by dword
        mov     ecx, [KMAP + 12]
        shl     ecx, 7                          ; dwords = sectors * 128
        mov     rsi, KERNEL
        xor     eax, eax
.sum:
        xor     eax, [rsi]
        add     rsi, 4
        dec     ecx
        jnz     .sum
        cmp     eax, [KMAP + 16]
        jne     .badsum
        cmp     dword [KERNEL], KNDL_MAGIC
        jne     .noguest
        ; --- the cairn, if packed: straight to 0x100000, same rite.
        ; KMAP+20/24 ride after the kernel half the old checksum covers —
        ; the loader never checks them, the kernel does (docs/CAIRN.md).
        mov     eax, [KMAP + 20]                ; cairn_lba
        test    eax, eax
        jz      .guest                          ; none packed — honest
        mov     r12d, eax                       ; next lba
        mov     r13d, [KMAP + 24]               ; sectors left
        mov     rdi, CAIRN
.cload:
        test    r13d, r13d
        jz      .guest
        mov     r14d, CHUNK
        cmp     r13d, r14d
        jae     .ccnt
        mov     r14d, r13d
.ccnt:
        mov     eax, r12d
        mov     ecx, r14d
        call    ata_read
        jc      .nodisk
        mov     eax, r14d
        shl     eax, 9
        add     rdi, rax
        add     r12d, r14d
        sub     r13d, r14d
        jmp     .cload

.guest:
        mov     rax, KERNEL + 4
        jmp     rax

.badkmap:
        mov     rsi, msg_badkmap
        call    puts
        jmp     .noguest
.badsum:
        mov     rsi, msg_badsum
        call    puts
.noguest:
        mov     rsi, msg_miss
        call    puts
.halt:
        hlt
        jmp     .halt
.nodisk:
        mov     rsi, msg_nodisk
        call    puts
        jmp     .noguest

; --- ata_read: RAX = LBA (28-bit), CL = sectors (1..255), RDI = destination.
; Preserves RBX/RBP/RSI/RDI/R12-R15; clobbers RAX/RCX/RDX/R8-R11.
; CF = 1 on failure: nothing behind the port, poll timeout, or ERR/DF.
ata_read:
        push    rbx
        push    rsi
        push    rdi
        push    r12
        push    r11
        mov     r8, rax                         ; lba
        mov     r11, rdi                        ; dest, restored per retry
        movzx   r12d, cl                        ; sectors
        mov     bh, 3                           ; attempts
.try:
        mov     rdi, r11
        mov     dx, 0x1F6
        mov     eax, r8d
        shr     eax, 24
        and     al, 0x0F
        or      al, 0xE0                        ; drive 0, LBA 27:24
        out     dx, al
        mov     dx, 0x3F6                       ; 400ns settle
        in      al, dx
        in      al, dx
        in      al, dx
        in      al, dx
        mov     dx, 0x1F7
        in      al, dx
        test    al, al
        jz      .gone                           ; floating bus: no disk
        cmp     al, 0xFF
        je      .gone
        mov     dx, 0x1F2
        mov     al, r12b
        out     dx, al
        mov     dx, 0x1F3
        mov     eax, r8d
        out     dx, al
        mov     dx, 0x1F4
        mov     eax, r8d
        shr     eax, 8
        out     dx, al
        mov     dx, 0x1F5
        mov     eax, r8d
        shr     eax, 16
        out     dx, al
        mov     dx, 0x1F7
        mov     al, 0x20                        ; READ SECTORS
        out     dx, al
        mov     r10d, r12d
.sector:
        call    ata_poll
        jc      .fail
        mov     ecx, 256
        mov     dx, 0x1F0
        cld
.word:
        in      ax, dx
        stosw
        dec     ecx
        jnz     .word
        dec     r10d
        jnz     .sector
        pop     r11
        pop     r12
        pop     rdi
        pop     rsi
        pop     rbx
        clc
        ret
.fail:
        dec     bh
        jz      .gone
        mov     dx, 0x3F6                       ; SRST the channel, retry
        mov     al, 0x04
        out     dx, al
        in      al, dx
        in      al, dx
        xor     al, al
        out     dx, al
        jmp     .try
.gone:
        pop     r11
        pop     r12
        pop     rdi
        pop     rsi
        pop     rbx
        stc
        ret

; --- ata_poll: wait for the next sector of a command. CF = 1 on timeout,
; ERR or device fault. The budget is generous (spin-up is slow on metal); a
; floating bus never gets here (ata_read checks status first).
ata_poll:
        push    rax
        push    rcx
        push    rdx
        mov     ecx, 0x400000
.wait:
        mov     dx, 0x1F7
        in      al, dx
        test    al, 0x80                        ; BSY
        jnz     .again
        test    al, 0x21                        ; ERR | DF
        jnz     .bad
        test    al, 0x08                        ; DRQ
        jnz     .ok
.again:
        dec     ecx
        jnz     .wait
.bad:
        pop     rdx
        pop     rcx
        pop     rax
        stc
        ret
.ok:
        pop     rdx
        pop     rcx
        pop     rax
        clc
        ret

puts:
        lodsb
        test    al, al
        jz      .done
        cmp     al, 10
        jne     .out
        mov     al, 13
        call    putc
        mov     al, 10
.out:
        call    putc
        jmp     puts
.done:
        ret

putc:
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

msg_ld:        db "cerne-ld", 10, 0
msg_nodisk:    db "cerne-ld: no disk", 10, 0
msg_badkmap:   db "cerne-ld: bad kmap", 10, 0
msg_badsum:    db "cerne-ld: kernel checksum", 10, 0
msg_miss:      db "kindling: no guest at 0x200000", 10, 0

        align   8
