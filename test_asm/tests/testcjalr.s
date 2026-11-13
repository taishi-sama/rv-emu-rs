.include "common.s"

.text
    .global __start
__start:
    li s0, 0
    la t0, 2f
    la t1, 4f
    li ra, 0
    c.jalr t0
4:  li ra, 0xbad
    j 5f
2:  li s0, 2
    sub s2, ra, t1
    TEST_CASE 2, s0, 2
    TEST_CASE 3, s2, 0

    li s0, 0
    la t0, 6f
    ori t0, t0, 1
    c.jalr t0
    li s0, 3
    j 7f
6:  li s0, 4
7:  TEST_CASE 4, s0, 4
5:
    call stop_by_fault
