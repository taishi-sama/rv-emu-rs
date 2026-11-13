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
    seqz t0, t0
    beqz t0, 2f
    j 3f
2:  seqz t0, s3
3:  TEST_CASE 5, t0, 1

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

    li s1, 0
    la t1, 6f
6:  .word 0x102525af
    TEST_CASE 8, s1, 2
    li t0, 0x102525af
    sub t0, s3, t0
    seqz t0, t0
    beqz t0, 2f
    j 3f
2:  seqz t0, s3
3:  TEST_CASE 9, t0, 1

    la a3, atomic_operand
    addi t2, a3, 2
    addi a3, a3, 2
    lr.w a4, 0(a3)
    andi t0, s1, -2
    TEST_CASE 10, t0, 4
    sub t0, s3, t2
    TEST_CASE 11, t0, 0

    amoadd.w a4, a1, 0(a3)
    andi t0, s1, -2
    TEST_CASE 12, t0, 6
    sub t0, s3, t2
    TEST_CASE 13, t0, 0

    sc.w a4, a1, 0(a3)
    andi t0, s1, -2
    TEST_CASE 14, t0, 6

    li a3, 0
    lr.w a4, 0(a3)
    TEST_CASE 15, s1, 5
    TEST_CASE 16, s3, 0

    li s3, 1
    amoadd.w a4, a1, 0(a3)
    TEST_CASE 17, s1, 7
    TEST_CASE 18, s3, 0

    la t0, __ram_end
    addi a3, t0, -1
    lw a4, 0(a3)
    andi t0, s1, -2
    TEST_CASE 19, t0, 4
    sub t0, s3, a3
    TEST_CASE 20, t0, 0

    sw a1, 0(a3)
    andi t0, s1, -2
    TEST_CASE 21, t0, 6
    sub t0, s3, a3
    TEST_CASE 22, t0, 0

    lh a4, 0(a3)
    andi t0, s1, -2
    TEST_CASE 23, t0, 4
    sh a1, 0(a3)
    andi t0, s1, -2
    TEST_CASE 24, t0, 6

    li s3, 7
    li a3, 0
    sc.w a4, a1, 0(a3)
    TEST_CASE 25, s1, 7
    TEST_CASE 26, s3, 0

    li a4, 0x1234
    amoadd.w a4, a1, 0(a3)
    TEST_CASE 27, a4, 0x1234

    la t0, trap_handler
    ori t0, t0, 2
    csrw mtvec, t0
    li s1, 0
    ecall
    TEST_CASE 28, s1, 11

    call stop_by_fault

trap_handler:
    csrr s1, mcause
    csrr s2, mepc
    csrr s3, mtval
    addi t0, s2, 4
    csrw mepc, t0
    mret

.data
.align 2
atomic_operand:
    .word 0
