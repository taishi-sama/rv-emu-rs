.include "common.s"

.text
    .global __start
__start:
    li s1, 20
    li a0, 6
    c.or s1, a0
    TEST_CASE 2, s1, 22

    li s1, 0x80000000
    li a0, 0x7fffffff
    c.or s1, a0
    TEST_CASE 3, s1, 0xffffffff

    call stop_by_fault
