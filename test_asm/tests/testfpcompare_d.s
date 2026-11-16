.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0x40040000

    feq.d a3, f1, f1
    TEST_CASE 2, a3, 1
    feq.d a3, f1, f2
    TEST_CASE 3, a3, 0

    flt.d a3, f1, f2
    TEST_CASE 4, a3, 1
    flt.d a3, f2, f1
    TEST_CASE 5, a3, 0

    load_d f3, 0, 0x80000000
    load_d f4, 0, 0
    flt.d a3, f3, f4
    TEST_CASE 6, a3, 0

    fle.d a3, f1, f1
    TEST_CASE 7, a3, 1
    fle.d a3, f2, f1
    TEST_CASE 8, a3, 0

    csrw fflags, zero
    load_d f3, 1, 0x7ff80000
    feq.d a3, f3, f1
    TEST_CASE 9, a3, 0
    csrr a3, fflags
    TEST_CASE 10, a3, 0

    csrw fflags, zero
    load_d f3, 1, 0x7ff00000
    feq.d a3, f3, f1
    TEST_CASE 11, a3, 0
    csrr a3, fflags
    TEST_CASE 12, a3, 0x10

    csrw fflags, zero
    load_d f3, 1, 0x7ff80000
    flt.d a3, f3, f1
    TEST_CASE 13, a3, 0
    csrr a3, fflags
    TEST_CASE 14, a3, 0x10

    csrw fflags, zero
    fle.d a3, f3, f1
    TEST_CASE 15, a3, 0
    csrr a3, fflags
    TEST_CASE 16, a3, 0x10

    call stop_by_fault
