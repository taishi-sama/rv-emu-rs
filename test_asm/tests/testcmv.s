.include "common.s"

.text
    .global __start
__start:
    li a0, 0x123
    c.mv t0, a0
    TEST_CASE 2, t0, 0x123

    li a0, 0
    li t0, 0x456
    c.mv t0, a0
    TEST_CASE 3, t0, 0

    call stop_by_fault
