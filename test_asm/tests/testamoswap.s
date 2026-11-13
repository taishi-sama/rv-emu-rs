.include "common.s"

.text
    .global __start
__start:
    li a0, 0x80000000
    li a1, 0xfffff800
    la a3, amo_operand
    sw a0, 0(a3)
    amoswap.w a4, a1, 0(a3)
    TEST_CASE 2, a4, 0x80000000
    lw a5, 0(a3)
    TEST_CASE 3, a5, 0xfffff800

    li a1, 0x80000000
    amoswap.w a4, a1, 0(a3)
    TEST_CASE 4, a4, 0xfffff800
    lw a5, 0(a3)
    TEST_CASE 5, a5, 0x80000000

    call stop_by_fault

.data
.align 2
amo_operand:
    .word 0
