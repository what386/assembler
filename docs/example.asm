.section text
.page 0                  ; label syntax, comment style shown for intent only
start:
    lim r0, 0x10
    mld r1, [0x0200]
    mlx r2, [r3+4]
    mst r1, [0x0201]
    msx r2, [r3-1]
    jmp loop
    bra ?equal, done

.page 1
loop:
    add r2, r2, r3
    addi r3, 1
    cmpi r3, 11
    bra ?lower, loop

    mst r2, [0x0200]
    mld r4, [0x0200]

    lim r5, 0x0300
    msx r4, [r5+0]
    mlx r6, [r5+0]

    tsti r6, 1
    cmov r7, r6, ?not_equal
    bra ?equal, done

    lim r7, 0
    jmp done

.page 2
done:
    halt

.section data

.org 0x0010
data:
    .string "text"
    .bytes 0x00, 0x1c, 0xff

.org 0x0f00
buffer:
    .zero 64
