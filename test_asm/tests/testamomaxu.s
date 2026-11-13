.include "common.s"

.text
    .global __start
__start:
    li a0, 0x80000000
    li a1, 0xfffff800
    la a3, amo_operand
    sw a0, 0(a3)
    amomaxu.w a4, a1, 0(a3)
    TEST_CASE 2, a4, 0x80000000
    lw a5, 0(a3)
    TEST_CASE 3, a5, 0xfffff800

    li a1, 0xffffffff
    sw zero, 0(a3)
    amomaxu.w a4, a1, 0(a3)
    TEST_CASE 4, a4, 0
    lw a5, 0(a3)
    TEST_CASE 5, a5, 0xffffffff

    li a0, 1
    li a1, 0x80000000
    sw a0, 0(a3)
    amomaxu.w a4, a1, 0(a3)
    TEST_CASE 6, a4, 1
    lw a5, 0(a3)
    TEST_CASE 7, a5, 0x80000000

    call stop_by_fault

.data
.align 2
amo_operand:
    .word 0
