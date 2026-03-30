.page 0                  ; label syntax, comment style shown for intent only
start:
    lim r0, 0x10
    mld r1, [0x02]
    mlx r2, [r3+4]
    mst [0x02], r1
    msx [r3-1], r2
    jmp loop
    bra start, ?not_equal

.page 1
loop:
    add r2, r2, r3
    addi r3, 1
    cmpi r3, 11
    bra loop, ?lower

    mst [0x02], r2
    mld r4, [0x02]

    lim r5, 0x03
    msx [r5+0], r4
    mlx r6, [r5+0]

    tsti r6, 1
    cmov r7, r6, ?not_equal
    bra done, ?equal

    lim r7, 0
done:
    halt


.org 0xff00
data:
    .string "text"
    .bytes 0x00, 0x1c, 0xff
