.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    la s0, data
    la t2, data

    li t0, 0x3f800000
    sw t0, 124(t2)
    c.flw f8, 124(s0)
    c.fsw f8, 0(s0)
    lw t0, 0(t2)
    TEST_CASE 2, t0, 0x3f800000

    sw zero, 248(t2)
    li t0, 0x3ff00000
    sw t0, 252(t2)
    c.fld f8, 248(s0)
    c.fsd f8, 0(s0)
    lw t0, 0(t2)
    TEST_CASE 3, t0, 0
    lw t0, 4(t2)
    TEST_CASE 4, t0, 0x3ff00000

    mv s1, sp
    la sp, data
    li t0, 0x40000000
    sw t0, 252(sp)
    c.flwsp f8, 252(sp)
    c.fswsp f8, 0(sp)
    lw t0, 0(t2)
    TEST_CASE 5, t0, 0x40000000

    sw zero, 504(sp)
    li t0, 0x3fe00000
    sw t0, 508(sp)
    c.fldsp f8, 504(sp)
    c.fsdsp f8, 0(sp)
    lw t0, 0(t2)
    TEST_CASE 6, t0, 0
    lw t0, 4(t2)
    TEST_CASE 7, t0, 0x3fe00000
    mv sp, s1

    call stop_by_fault

.data
.align 3
    .zero 16
data:
    .zero 512
