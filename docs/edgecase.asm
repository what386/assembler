; Assembly surface-area fixture.
; This file intentionally exercises weird or special-case behavior:
; - preprocessor directives
; - alias families and family-wide overloads
; - pseudo-instructions
; - labels and qualified labels
; - standard and alternate conditions
; - indexed addressing
; - data/layout directives

.define DATA_BYTE 'A'
.define PTR [r3+1]
.define LOOP_COUNT 3

.page 0
start:
    lim r0, 0
    lim r1, LOOP_COUNT
    lim r2, 0x10
    lim r3, 0x20

loop.inner:
    mov r4, r0
    xchg r4, r1
    mov r5, r6, ?always

    psh r0
    poke r1
    pshf r2
    isp r3

    pop r0
    peek r1
    popf r2
    dsp r3

    ret
    brk
    iret ?equal

    cmp r0, r1
    inc r0
    dec r1
    not r4, r5

    and r4, r5, r6
    xor r4, r5, r6
    bsli r4, r5, 2
    mul r4, r5, r6
    popcnt r4, r5, 3

    mlx r6, PTR
    msx [r3-1], r6
    bra loop.inner, @greater_equal
    jmp done

    .bytes 0x00, DATA_BYTE, '\n'
    .fill 2, 0xff
    .string "hi"
    .cstring "ok"

.ifdef ENABLE_EXTRA
extra.path:
    timer.init 0x10
    timer.val 0x20
    pcw.clear 1
    pcw.set 2
    int 3
    mpge 4
.else
    nop
.endif

.page 1
done:
    halt
    .org 0x90
tail:
    brx ?always, [r3]
