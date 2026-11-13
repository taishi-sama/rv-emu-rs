.include "common.s"

.text
    .global __start
__start:
    la a3, operand

    li a5, 0xdeadbeef
    sc.w a4, a5, 0(a3)
    snez a4, a4
    TEST_CASE 2, a4, 1
    lw a4, 0(a3)
    TEST_CASE 3, a4, 0

    li t0, 0x11223344
    sw t0, 0(a3)
    lr.w a4, 0(a3)
    TEST_CASE 4, a4, 0x11223344

    li t1, 0x55667788
    sc.w a5, t1, 0(a3)
    seqz a5, a5
    TEST_CASE 5, a5, 1
    lw a4, 0(a3)
    TEST_CASE 6, a4, 0x55667788

    li t1, 0x99aabbcc
    sc.w a5, t1, 0(a3)
    snez a5, a5
    TEST_CASE 7, a5, 1
    lw a4, 0(a3)
    TEST_CASE 8, a4, 0x55667788

    sc.w a5, t1, 0(a3)
    snez a5, a5
    TEST_CASE 9, a5, 1

    sw zero, 0(a3)
    li s1, 10
1:  lr.w a4, 0(a3)
    addi a4, a4, 1
    sc.w a5, a4, 0(a3)
    bnez a5, 1b
    addi s1, s1, -1
    bnez s1, 1b
    lw a4, 0(a3)
    TEST_CASE 10, a4, 10

    lr.w x0, 0(a3)
    TEST_CASE 11, zero, 0

    li t0, 0x0badf00d
    sw t0, 0(a3)
    lr.w.aq a4, 0(a3)
    TEST_CASE 12, a4, 0x0badf00d
    li t1, 0x0badf00e
    sc.w.rl a5, t1, 0(a3)
    seqz a5, a5
    TEST_CASE 13, a5, 1
    lw a4, 0(a3)
    TEST_CASE 14, a4, 0x0badf00e

    lr.w a4, 0(a3)
    sc.w t1, t1, 0(a3)
    seqz t1, t1
    TEST_CASE 15, t1, 1
    lw a4, 0(a3)
    TEST_CASE 16, a4, 0x0badf00e

    lr.w a4, 0(a3)
    sc.w a3, t1, 0(a3)
    seqz a3, a3
    TEST_CASE 17, a3, 1

    call stop_by_fault

.data
.align 2
operand:
    .word 0
