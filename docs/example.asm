; Count from 0 to 9, then stop.

.define LIMIT 10

.page 0
start:
    lim r0, 0

loop:
    addi r0, 1
    cmpi r0, LIMIT
    bra done, ?equal
    bra loop, ?always

done:
    halt
