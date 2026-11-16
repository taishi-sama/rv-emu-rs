.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    la s0, data
    addi s1, s0, 8

    li t0, 0x11223344
    sw t0, 0(s0)
    li t0, 0x55667788
    sw t0, 4(s0)

    fld f1, 0(s0)
    fsd f1, 16(s0)
    lw t0, 16(s0)
    TEST_CASE 2, t0, 0x11223344
    lw t0, 20(s0)
    TEST_CASE 3, t0, 0x55667788

    fld f2, -8(s1)
    fsd f2, 24(s0)
    lw t0, 24(s0)
    TEST_CASE 4, t0, 0x11223344
    lw t0, 28(s0)
    TEST_CASE 5, t0, 0x55667788

    fsw f1, 32(s0)
    lw t0, 32(s0)
    TEST_CASE 6, t0, 0x11223344
    lw t0, 36(s0)
    TEST_CASE 7, t0, 0

    li t0, 0x3f800000
    sw t0, 40(s0)
    flw f3, 40(s0)
    fsd f3, 48(s0)
    lw t0, 48(s0)
    TEST_CASE 8, t0, 0x3f800000
    lw t0, 52(s0)
    TEST_CASE 9, t0, 0xffffffff

    li t0, 0x3f800000
    sw t0, 56(s0)
    sw zero, 60(s0)
    fld f4, 56(s0)
    fsd f4, 64(s0)
    lw t0, 64(s0)
    TEST_CASE 10, t0, 0x3f800000
    lw t0, 68(s0)
    TEST_CASE 11, t0, 0

    fsd f4, -8(s1)
    lw t0, 0(s0)
    TEST_CASE 12, t0, 0x3f800000
    lw t0, 4(s0)
    TEST_CASE 13, t0, 0

    call stop_by_fault

.data
.align 3
data:
    .zero 72
