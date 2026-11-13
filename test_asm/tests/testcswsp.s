.include "common.s"

.text
    .global __start
__start:
    mv s0, sp
    la sp, data
    la t2, data
    li a0, 0x0badf00d
    c.swsp a0, 8(sp)
    lw t0, 8(t2)
    li a0, 0xdeadbeef
    c.swsp a0, 252(sp)
    lw t1, 252(t2)
    mv sp, s0
    TEST_CASE 2, t0, 0x0badf00d
    TEST_CASE 3, t1, 0xdeadbeef

    call stop_by_fault

.data
.align 2
data:
    .zero 256
