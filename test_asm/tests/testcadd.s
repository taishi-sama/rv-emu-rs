.include "common.s"

.text
    .global __start
__start:
    li a0, 0x123
    li t0, 0x123
    c.add t0, a0
    TEST_CASE 2, t0, 0x246

    li a0, -1
    li t0, 1
    c.add t0, a0
    TEST_CASE 3, t0, 0

    call stop_by_fault
