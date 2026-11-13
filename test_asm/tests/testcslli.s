.include "common.s"

.text
    .global __start
__start:
    li a0, 1
    c.slli a0, 31
    TEST_CASE 2, a0, 0x80000000

    li a0, 0x12345678
    c.slli a0, 4
    TEST_CASE 3, a0, 0x23456780

    c.slli zero, 4
    TEST_CASE 4, zero, 0

    call stop_by_fault
