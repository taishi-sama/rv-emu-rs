.include "common.s"

.text
    .global __start
__start:
    la t0, trap_handler
    csrw mtvec, t0

    li s1, 0
    li s3, 7
    li t6, 0xdead
    la t1, 2f
2:  c.ebreak
    TEST_CASE 2, s1, 3
    sub t0, s3, t1
    seqz t0, t0
    beqz t0, 3f
    j 4f
3:  seqz t0, s3
4:  TEST_CASE 3, t0, 1

    call stop_by_fault

.align 2
trap_handler:
    csrr s1, mcause
    csrr s3, mtval
    csrr t0, mepc
    addi t0, t0, 2
    csrw mepc, t0
    mret
