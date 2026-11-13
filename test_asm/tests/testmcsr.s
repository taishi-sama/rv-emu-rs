.include "common.s"

.text
    .global __start
__start:
    li t0, 0x12345678
    csrrw t1, mscratch, t0
    TEST_CASE 2, t1, 0
    csrr t1, mscratch
    TEST_CASE 3, t1, 0x12345678

    li t0, 0xdeadbeef
    csrrw t1, mscratch, t0
    TEST_CASE 4, t1, 0x12345678

    li t0, 0x0f0f0f0f
    csrw mscratch, t0
    li t1, 0x00ff00ff
    csrrs t2, mscratch, t1
    TEST_CASE 5, t2, 0x0f0f0f0f
    csrr t2, mscratch
    TEST_CASE 6, t2, 0x0fff0fff

    csrrs t2, mscratch, x0
    TEST_CASE 7, t2, 0x0fff0fff

    csrrc t2, mscratch, t1
    TEST_CASE 8, t2, 0x0fff0fff
    csrr t2, mscratch
    TEST_CASE 9, t2, 0x0f000f00

    csrrc t2, mscratch, x0
    TEST_CASE 10, t2, 0x0f000f00

    csrrwi t2, mscratch, 0x1f
    TEST_CASE 11, t2, 0x0f000f00
    csrr t2, mscratch
    TEST_CASE 12, t2, 0x0000001f

    li t0, 0x55
    csrrw x0, mscratch, t0
    csrr t2, mscratch
    TEST_CASE 13, t2, 0x00000055

    li t0, 0x0f0
    csrw mscratch, t0
    li t1, 0xf0f
    csrrs x0, mscratch, t1
    csrr t2, mscratch
    TEST_CASE 14, t2, 0x00000fff

    csrrc x0, mscratch, t1
    csrr t2, mscratch
    TEST_CASE 15, t2, 0x000000f0

    csrrwi x0, mscratch, 3
    csrr t2, mscratch
    TEST_CASE 16, t2, 3

    li t0, 0x0f0f0f0f
    csrw mscratch, t0
    csrrsi t2, mscratch, 0x1f
    TEST_CASE 17, t2, 0x0f0f0f0f
    csrr t2, mscratch
    TEST_CASE 18, t2, 0x0f0f0f1f

    csrrci t2, mscratch, 0x0f
    TEST_CASE 19, t2, 0x0f0f0f1f
    csrr t2, mscratch
    TEST_CASE 20, t2, 0x0f0f0f10

    csrrsi t2, mscratch, 0
    TEST_CASE 21, t2, 0x0f0f0f10

    csrrci t2, mscratch, 0
    TEST_CASE 22, t2, 0x0f0f0f10

    li t0, 0xfffffffe
    csrw mscratch, t0
    csrrsi x0, mscratch, 0x1
    csrr t2, mscratch
    TEST_CASE 23, t2, 0xffffffff

    csrrci x0, mscratch, 0x10
    csrr t2, mscratch
    TEST_CASE 24, t2, 0xffffffef

    la s2, __start
    csrw mepc, s2
    csrr t2, mepc
    sub t0, t2, s2
    TEST_CASE 25, t0, 0

    li t0, 0x0000000b
    csrw mcause, t0
    csrr t2, mcause
    TEST_CASE 26, t2, 0x0000000b

    la s2, __start
    csrw mtval, s2
    csrr t2, mtval
    sub t0, t2, s2
    TEST_CASE 27, t0, 0

    li t0, 0x00001888
    csrw mstatus, t0
    csrr t2, mstatus
    TEST_CASE 28, t2, 0x00001888

    csrrsi t2, misa, 0
    srli t2, t2, 30
    TEST_CASE 29, t2, 1

    csrr t2, time
    csrr t3, time
    bgeu t3, t2, 1f
    fail 30
    j 2f
1:  pass 30
2:
    li t0, 0x0f0f0f0f
    csrw mscratch, t0
    csrrci t2, mscratch, 0x1f
    TEST_CASE 31, t2, 0x0f0f0f0f
    csrr t2, mscratch
    TEST_CASE 32, t2, 0x0f0f0f00

    li t0, 0x00001888
    csrw mstatus, t0
    csrrci t2, mstatus, 0x8
    TEST_CASE 33, t2, 0x00001888
    csrr t2, mstatus
    TEST_CASE 34, t2, 0x00001880
    csrrsi t2, mstatus, 0x8
    TEST_CASE 35, t2, 0x00001880
    csrr t2, mstatus
    TEST_CASE 36, t2, 0x00001888

    csrrci t2, misa, 0
    srli t2, t2, 30
    TEST_CASE 37, t2, 1

    csrr t2, cycleh
    csrr t2, cycle

    call stop_by_fault
