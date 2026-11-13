.include "common.s"

.text
    .global __start
__start:
    li s1, 20
    li a0, 6
    c.sub s1, a0
    TEST_CASE 2, s1, 14

    li s1, 0
    li a0, 1
    c.sub s1, a0
    TEST_CASE 3, s1, -1

    call stop_by_fault
