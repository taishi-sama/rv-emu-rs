.include "common.s"

.text
    .global __start
__start:
    li t0, 0
    jal ra, 2f
1:  li t0, 1
    j 3f
2:  la t1, 1b
    li t0, 0
    beq ra, t1, 3f
    li t0, 1
3:  TEST_CASE 2, t0, 0

    li t0, 1
    jal x0, 4f
    addi t0, t0, 1
    addi t0, t0, 1
4:  addi t0, t0, 2
    TEST_CASE 3, t0, 3

    li t0, 5
    jal x0, 5f
    li t0, 6
5:  TEST_CASE 4, t0, 5

    li t0, 0
    j 6f
7:  addi t0, t0, 1
    jal x0, 8f
6:  jal ra, 7b
8:  TEST_CASE 5, t0, 1

    call stop_by_fault
