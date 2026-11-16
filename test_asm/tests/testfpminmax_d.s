.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0x40040000
    fmin.d f3, f1, f2
    test_d_case 2, 3, f3, 0, 0x3ff80000
    fmax.d f3, f1, f2
    test_d_case 4, 5, f3, 0, 0x40040000

    load_d f1, 0, 0x80000000
    load_d f2, 0, 0
    fmin.d f3, f1, f2
    test_d_case 6, 7, f3, 0, 0x80000000
    fmax.d f3, f1, f2
    test_d_case 8, 9, f3, 0, 0

    csrw fflags, zero
    load_d f1, 1, 0x7ff80000
    load_d f2, 0, 0x3ff80000
    fmin.d f3, f1, f2
    test_d_case 10, 11, f3, 0, 0x3ff80000
    csrr a3, fflags
    TEST_CASE 12, a3, 0

    csrw fflags, zero
    load_d f1, 1, 0x7ff00000
    fmin.d f3, f1, f2
    test_d_case 13, 14, f3, 0, 0x3ff80000
    csrr a3, fflags
    TEST_CASE 15, a3, 0x10

    csrw fflags, zero
    load_d f2, 1, 0x7ff80000
    fmin.d f3, f1, f2
    test_d_case 16, 17, f3, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 18, a3, 0x10

    csrw fflags, zero
    load_d f2, 0, 0x3ff80000
    fmax.d f3, f2, f1
    test_d_case 19, 20, f3, 0, 0x3ff80000
    csrr a3, fflags
    TEST_CASE 21, a3, 0x10

    csrw fflags, zero
    load_d f1, 0, 0x7ff80000
    load_d f2, 0x123, 0x7ff80000
    fmin.d f3, f1, f2
    test_d_case 22, 23, f3, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 24, a3, 0
    fmax.d f3, f1, f2
    test_d_case 25, 26, f3, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 27, a3, 0

    load_s f1, 0x3f800000
    load_s f2, 0x40000000
    fmin.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 28, a3, 0x3f800000
    fmax.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 29, a3, 0x40000000

    csrw fflags, zero
    load_s f2, 0x7fc00001
    fmin.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 30, a3, 0x3f800000
    csrr a3, fflags
    TEST_CASE 31, a3, 0

    csrw fflags, zero
    load_s f2, 0x7f800001
    fmax.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 32, a3, 0x3f800000
    csrr a3, fflags
    TEST_CASE 33, a3, 0x10

    csrw fflags, zero
    load_d f4, 0, 0
    fmin.s f3, f4, f1
    fmv.x.w a3, f3
    TEST_CASE 34, a3, 0x3f800000
    csrr a3, fflags
    TEST_CASE 35, a3, 0

    csrw fflags, zero
    load_s f1, 0x7fc00001
    load_s f2, 0x7fc00002
    fmin.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 36, a3, 0x7fc00000
    csrr a3, fflags
    TEST_CASE 37, a3, 0
    fmax.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 38, a3, 0x7fc00000

    call stop_by_fault
