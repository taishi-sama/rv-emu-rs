.include "common.s"

.text
    .global __start
__start:
    li a0, 0xffffffff
    li a1, 0x0fffffff
    la a3, amo_operand
    sw a0, 0(a3)
    amoand.w a4, a1, 0(a3)
    TEST_CASE 2, a4, 0xffffffff
    lw a5, 0(a3)
    TEST_CASE 3, a5, 0x0fffffff

    li a1, 0xff00ff00
    amoand.w a4, a1, 0(a3)
    TEST_CASE 4, a4, 0x0fffffff
    lw a5, 0(a3)
    TEST_CASE 5, a5, 0x0f00ff00

    call stop_by_fault

.data
.align 2
amo_operand:
    .word 0
