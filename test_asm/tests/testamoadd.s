.include "common.s"

.text
    .global __start
__start:
    li a0, 0x80000000
    li a1, 0xfffff800
    la a3, amo_operand
    sw a0, 0(a3)
    amoadd.w a4, a1, 0(a3)
    TEST_CASE 2, a4, 0x80000000
    lw a5, 0(a3)
    TEST_CASE 3, a5, 0x7ffff800

    li a1, 0x80000000
    amoadd.w a4, a1, 0(a3)
    TEST_CASE 4, a4, 0x7ffff800
    lw a5, 0(a3)
    TEST_CASE 5, a5, 0xfffff800

    li a1, 1
    amoadd.w.aqrl a4, a1, 0(a3)
    TEST_CASE 6, a4, 0xfffff800
    lw a5, 0(a3)
    TEST_CASE 7, a5, 0xfffff801

    amoadd.w x0, a1, 0(a3)
    TEST_CASE 8, zero, 0
    lw a5, 0(a3)
    TEST_CASE 9, a5, 0xfffff802

    amoadd.w a1, a1, 0(a3)
    TEST_CASE 10, a1, 0xfffff802
    lw a5, 0(a3)
    TEST_CASE 11, a5, 0xfffff803

    call stop_by_fault

.data
.align 2
amo_operand:
    .word 0
