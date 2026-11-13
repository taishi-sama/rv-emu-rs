.include "common.s"

.text
    .global __start
__start:
    c.li a0, -32
    TEST_CASE 2, a0, -32

    c.li a0, 31
    TEST_CASE 3, a0, 31

    c.li a0, 0
    TEST_CASE 4, a0, 0

    call stop_by_fault
