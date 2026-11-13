.include "common.s"

.text
    .global __start
__start:
    li t0, 0
    c.j 2f
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 2, t0, 1

    li t0, 0
    j 3f
4:  addi t0, t0, 1
    c.j 5f
3:  c.j 4b
5:  TEST_CASE 3, t0, 1

    call stop_by_fault
