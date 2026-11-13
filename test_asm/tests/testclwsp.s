.include "common.s"

.text
    .global __start
__start:
    mv s0, sp
    la sp, data
    la t2, data
    li t0, 0x11223344
    sw t0, 4(t2)
    li t0, 0x55667788
    sw t0, 252(t2)
    c.lwsp a0, 4(sp)
    c.lwsp a1, 252(sp)
    mv sp, s0
    TEST_CASE 2, a0, 0x11223344
    TEST_CASE 3, a1, 0x55667788

    call stop_by_fault

.data
.align 2
data:
    .zero 256
