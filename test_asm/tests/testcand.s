.include "common.s"

.text
    .global __start
__start:
    li s1, 20
    li a0, 6
    c.and s1, a0
    TEST_CASE 2, s1, 4

    li s1, 0xffffffff
    li a0, 0x00ff00ff
    c.and s1, a0
    TEST_CASE 3, s1, 0x00ff00ff

    call stop_by_fault
