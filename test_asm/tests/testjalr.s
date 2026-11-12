.include "common.s"

.text
    .global __start
__start:
    li s0, 0
    la t1, 2f
    jalr t0, t1, 0
1:  li s0, 1
    j 3f
2:  la t1, 1b
    sub t0, t0, t1
    TEST_CASE 2, t0, 0
3:  TEST_CASE 3, s0, 0

    li s0, 0
    la t0, 5f
    jalr t0, t0, 0
4:  li s0, 1
    j 6f
5:  la t1, 4b
    sub t0, t0, t1
    TEST_CASE 4, t0, 0
6:  TEST_CASE 5, s0, 0

    li s0, 0
    la t1, 8f
    addi t1, t1, -8
    jalr t0, t1, 8
7:  li s0, 1
    j 9f
8:  TEST_CASE 6, s0, 0
9:  TEST_CASE 7, s0, 0

    li s0, 0
    la t1, 11f
    jalr x0, t1, 0
10: li s0, 2
    j 12f
11: li s0, 1
12: TEST_CASE 8, s0, 1
    TEST_CASE 9, zero, 0

    call stop_by_fault
