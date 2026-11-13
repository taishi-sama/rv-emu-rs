.include "common.s"

.text
    .global __start
__start:
    li t0, 1
    c.addi t0, 31
    TEST_CASE 2, t0, 32

    li t0, -1
    c.addi t0, -32
    TEST_CASE 3, t0, -33

    li t0, 0x1234
    c.nop
    TEST_CASE 4, t0, 0x1234

    c.addi zero, 5
    TEST_CASE 5, zero, 0

    call stop_by_fault
