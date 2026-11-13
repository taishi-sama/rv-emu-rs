.include "common.s"

.text
    .global __start
__start:
    li s0, 0
    la t0, 2f
    c.jr t0
    li s0, 1
    j 3f
2:  li s0, 2
3:  TEST_CASE 2, s0, 2

    li s0, 0
    la t0, 4f
    ori t0, t0, 1
    c.jr t0
    li s0, 3
    j 5f
4:  li s0, 4
5:  TEST_CASE 3, s0, 4

    call stop_by_fault
