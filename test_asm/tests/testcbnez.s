.include "common.s"

.text
    .global __start
__start:
    li a0, 1
    c.bnez a0, 2f
    li t0, 1
    j 3f
2:  li t0, 2
3:  TEST_CASE 2, t0, 2

    li a0, 0
    c.bnez a0, 4f
    li t0, 3
    j 5f
4:  li t0, 4
5:  TEST_CASE 3, t0, 3

    li s0, 0
    j 4f
3:  addi s0, s0, 1
    c.bnez s0, 6f
    li s0, 9
6:  TEST_CASE 4, s0, 1
    j 8f
4:  li a0, 1
    c.bnez a0, 3b
8:  call stop_by_fault
