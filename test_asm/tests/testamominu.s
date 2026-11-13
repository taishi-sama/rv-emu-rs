.include "common.s"

.text
    .global __start
__start:
    li a0, 0x80000000
    li a1, 0x7fffffff
    la a3, amo_operand
    sw a0, 0(a3)
    amominu.w a4, a1, 0(a3)
    TEST_CASE 2, a4, 0x80000000
    lw a5, 0(a3)
    TEST_CASE 3, a5, 0x7fffffff

    li a1, 1
    amominu.w a4, a1, 0(a3)
    TEST_CASE 4, a4, 0x7fffffff
    lw a5, 0(a3)
    TEST_CASE 5, a5, 1

    li a1, 0
    amominu.w a4, a1, 0(a3)
    TEST_CASE 6, a4, 1
    lw a5, 0(a3)
    TEST_CASE 7, a5, 0

    call stop_by_fault

.data
.align 2
amo_operand:
    .word 0
