.include "common.s"

.text
    .global __start
__start:
    li s1, 20
    li a0, 6
    c.xor s1, a0
    TEST_CASE 2, s1, 18

    li s1, 0xffffffff
    li a0, 0x80000000
    c.xor s1, a0
    TEST_CASE 3, s1, 0x7fffffff

    call stop_by_fault
