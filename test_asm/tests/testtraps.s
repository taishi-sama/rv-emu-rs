.include "common.s"

.text
    .global __start
__start:
    la t0, trap_handler
    csrw mtvec, t0

    li s1, 0
    li s3, 1
    ecall
    TEST_CASE 2, s1, 11
    TEST_CASE 3, s3, 0

    li s1, 0
    la t1, 2f
2:  ebreak
    TEST_CASE 4, s1, 3
    sub t0, s3, t1
    TEST_CASE 5, t0, 0

    la t1, 3f
3:  ecall
    sub t0, s2, t1
    TEST_CASE 6, t0, 0

    li s0, 0
    la t0, 4f
    csrw mepc, t0
    mret
    li s0, 2
    j 5f
4:  li s0, 1
5:  TEST_CASE 7, s0, 1

    call stop_by_fault

trap_handler:
    csrr s1, mcause
    csrr s2, mepc
    csrr s3, mtval
    addi t0, s2, 4
    csrw mepc, t0
    mret
