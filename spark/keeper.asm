; keeper — Setup Assistant and the hall.
; First boot (empty book): name, word, word again, enlist.
; Answers: the name is cut / the word sleeps in the twin.
; Choose: this fire knows you. Dismiss: that name is given back.
; Confirm miss: the second word is a stranger.
; Later: list keepers; digit chooses, n enlists, d dismisses.
; The word echoes as stars. Name keys echo. Backspace edits.
; House calls: write 2, read 10, glean 8, roll 11, enlist 12,
; choose 13, dismiss 14, exit 1.
; Book leaves hands/twin are 716 bytes (kernel SEAL_LEN).

        bits    64
        default abs

%macro WRITE 2
        mov     rax, 2
        mov     rdi, 1
        lea     rsi, [rel %1]
        mov     rdx, %2
        int     0xE0
%endmacro

start:
        WRITE   hello, 7
        jmp     .cycle

.cycle:
        mov     rax, 11                 ; roll
        lea     rdi, [rel rollbuf]
        mov     rsi, 256
        int     0xE0
        test    rax, rax
        js      die
        mov     r13, rax                ; count
        test    rax, rax
        jnz     .hall

        ; empty book: upgrade from an inked hand, or ask a name
        mov     rax, 8
        lea     rdi, [rel handname]
        lea     rsi, [rel namebuf]
        mov     rdx, 32
        int     0xE0
        test    rax, rax
        js      .askname
        cmp     byte [rel namebuf], 0
        je      .askname
        lea     rsi, [rel namebuf]
        xor     edx, edx
.unlen:
        cmp     rdx, 32
        jae     .unwrite
        cmp     byte [rsi + rdx], 0
        je      .unwrite
        inc     rdx
        jmp     .unlen
.unwrite:
        test    rdx, rdx
        jz      .askname
        mov     rax, 2
        mov     rdi, 1
        int     0xE0
        WRITE   nl, 1
        jmp     .askword_first

.askname:
        WRITE   ask, 20
        lea     r12, [rel namebuf]
        mov     r13, 31
        xor     r15, r15
        call    read_line
        test    rbx, rbx
        jz      .askname
.askword_first:
        call    ask_words
        test    rax, rax
        jnz     .askword_first
        mov     rax, 12                 ; enlist
        lea     rdi, [rel namebuf]
        lea     rsi, [rel wordbuf]
        lea     rdx, [rel word2]
        int     0xE0
        test    rax, rax
        js      .enlist_fail
        call    tell_enlisted
        jmp     ok
.enlist_fail:
        cmp     rax, -19                ; -ENODEV shut
        je      die
        cmp     rax, -22                ; -EINVAL confirm
        je      .askword_first
        jmp     .cycle                  ; ink miss; a lone leaf is wax; split dies on roll

.hall:
        WRITE   ask, 20
        xor     r14, r14
.list:
        cmp     r14, r13
        jae     .menu
        WRITE   pad, 2
        mov     al, '1'
        add     al, r14b
        mov     [rel digit], al
        WRITE   digit, 1
        WRITE   spc, 1
        lea     rsi, [rel rollbuf]
        mov     rax, r14
        shl     rax, 5
        add     rsi, rax
        xor     edx, edx
.hlen:
        cmp     rdx, 32
        jae     .hwrite
        cmp     byte [rsi + rdx], 0
        je      .hwrite
        inc     rdx
        jmp     .hlen
.hwrite:
        test    rdx, rdx
        jz      .hnl
        mov     rax, 2
        mov     rdi, 1
        int     0xE0
.hnl:
        WRITE   nl, 1
        inc     r14
        jmp     .list
.menu:
        WRITE   newk, 16
        WRITE   dism, 11
        call    read_key
        cmp     al, 'n'
        je      .new
        cmp     al, 'd'
        je      .drop
        cmp     al, '1'
        jb      .hall
        cmp     al, '8'
        ja      .hall
        sub     al, '1'
        movzx   r14, al
        cmp     r14, r13
        jae     .hall
        lea     rsi, [rel rollbuf]
        mov     rax, r14
        shl     rax, 5
        add     rsi, rax
        lea     rdi, [rel namebuf]
        mov     ecx, 32
        cld
        rep     movsb
        call    ask_one_word
        test    rax, rax
        jnz     .hall
        mov     rax, 13                 ; choose
        lea     rdi, [rel namebuf]
        lea     rsi, [rel wordbuf]
        int     0xE0
        test    rax, rax
        js      .choose_fail
        WRITE   knows, knows_len
        jmp     ok
.choose_fail:
        cmp     rax, -5
        je      die
        cmp     rax, -19
        je      die
        jmp     .cycle

.new:
        WRITE   ask, 20
        lea     r12, [rel namebuf]
        mov     r13, 31
        xor     r15, r15
        call    read_line
        test    rbx, rbx
        jz      .cycle
        call    ask_words
        test    rax, rax
        jnz     .cycle
        mov     rax, 12
        lea     rdi, [rel namebuf]
        lea     rsi, [rel wordbuf]
        lea     rdx, [rel word2]
        int     0xE0
        test    rax, rax
        js      .enlist_fail
        call    tell_enlisted
        jmp     .cycle

.drop:
        WRITE   ask, 20
        lea     r12, [rel namebuf]
        mov     r13, 31
        xor     r15, r15
        call    read_line
        test    rbx, rbx
        jz      .cycle
        cmp     rbx, 1
        jne     .drop_named
        mov     al, [rel namebuf]
        cmp     al, '1'
        jb      .drop_named
        cmp     al, '8'
        ja      .drop_named
        sub     al, '1'
        movzx   r14, al
        ; re-roll count is in... we lost r13 count after read_line.
        ; copy from last rollbuf; if empty slot name is zeros, dismiss will fail.
        lea     rsi, [rel rollbuf]
        mov     rax, r14
        shl     rax, 5
        add     rsi, rax
        cmp     byte [rsi], 0
        je      .cycle
        lea     rdi, [rel namebuf]
        mov     ecx, 32
        cld
        rep     movsb
.drop_named:
        call    ask_one_word
        test    rax, rax
        jnz     .cycle
        mov     rax, 14                 ; dismiss
        lea     rdi, [rel namebuf]
        lea     rsi, [rel wordbuf]
        int     0xE0
        test    rax, rax
        js      .choose_fail
        WRITE   given, given_len
        jmp     .cycle

ok:
        mov     rax, 1
        xor     edi, edi
        int     0xE0
        ud2
die:
        mov     rax, 1
        mov     rdi, 1
        int     0xE0
        ud2

; ask_words: speak the word, read stars, speak it again, read stars.
; rax=0 ok, rax=1 empty or the two words are not kin
ask_words:
        WRITE   wordp, 15
        lea     r12, [rel wordbuf]
        mov     r13, 63
        mov     r15, 1
        call    read_line
        test    rbx, rbx
        jz      .aw_bad
        mov     [rel wlen], rbx
        WRITE   againp, 15
        lea     r12, [rel word2]
        mov     r13, 63
        mov     r15, 1
        call    read_line
        test    rbx, rbx
        jz      .aw_bad
        cmp     rbx, [rel wlen]
        jne     .aw_miss
        mov     rcx, rbx
        lea     rsi, [rel wordbuf]
        lea     rdi, [rel word2]
        cld
        repe    cmpsb
        jne     .aw_miss
        xor     eax, eax
        ret
.aw_miss:
        WRITE   stranger, stranger_len
        mov     eax, 1
        ret
.aw_bad:
        mov     eax, 1
        ret

tell_enlisted:
        WRITE   cut, cut_len
        WRITE   sleepw, sleepw_len
        ret

ask_one_word:
        WRITE   wordp, 15
        lea     r12, [rel wordbuf]
        mov     r13, 63
        mov     r15, 1
        call    read_line
        test    rbx, rbx
        jz      .ow_bad
        xor     eax, eax
        ret
.ow_bad:
        mov     eax, 1
        ret

; read_key: one byte into al (and [rel key])
read_key:
        mov     rax, 10
        xor     edi, edi
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     die
        mov     al, [rel key]
        ret

; read_line: r12=buf, r13=cap, r15=0 echo glyph / 1 star
; rbx=length
read_line:
        cld
        mov     rdi, r12
        mov     rcx, r13
        inc     rcx
        xor     eax, eax
        rep     stosb
        xor     ebx, ebx
.rl_more:
        mov     rax, 10
        xor     edi, edi
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        test    rax, rax
        jle     die
        mov     al, [rel key]
        cmp     al, 10
        je      .rl_got
        cmp     al, 8
        je      .rl_bk
        cmp     al, 0x20
        jb      .rl_more
        cmp     al, 0x7E
        ja      .rl_more
        cmp     rbx, r13
        jae     .rl_more
        mov     [r12 + rbx], al
        inc     rbx
        cmp     r15, 1
        je      .rl_star
        mov     rax, 2
        mov     rdi, 1
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        jmp     .rl_more
.rl_star:
        mov     rax, 2
        mov     rdi, 1
        lea     rsi, [rel star]
        mov     rdx, 1
        int     0xE0
        jmp     .rl_more
.rl_bk:
        test    rbx, rbx
        jz      .rl_more
        dec     rbx
        mov     byte [r12 + rbx], 0
        mov     rax, 2
        mov     rdi, 1
        lea     rsi, [rel key]
        mov     rdx, 1
        int     0xE0
        jmp     .rl_more
.rl_got:
        WRITE   nl, 1
        ret

hello:  db      "keeper", 10
ask:    db      "who keeps this fire", 10
wordp:  db      "speak the word", 10
againp: db      "speak it again", 10
newk:   db      "n  a new keeper", 10
dism:   db      "d  dismiss", 10
cut:    db      "the name is cut", 10
cut_len equ     $ - cut
sleepw: db      "the word sleeps in the twin", 10
sleepw_len equ  $ - sleepw
knows:  db      "this fire knows you", 10
knows_len equ   $ - knows
given:  db      "that name is given back", 10
given_len equ   $ - given
stranger: db    "the second word is a stranger", 10
stranger_len equ $ - stranger
pad:    db      "  "
spc:    db      " "
nl:     db      10
star:   db      "*"
digit:  db      "1"
handname: db    "hand", 0
key:    db      0
wlen:   dq      0
namebuf: times 32 db 0
wordbuf: times 64 db 0
word2:  times 64 db 0
rollbuf: times 256 db 0
