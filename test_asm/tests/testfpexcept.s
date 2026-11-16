.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0

    csrw fflags, zero
    load_s f1, 0x40600000
    fcvt.w.s a3, f1
    csrr a3, fflags
    TEST_CASE 2, a3, 0x01

    load_s f1, 0x3f800000
    load_s f2, 0x00000000
    fdiv.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 3, a3, 0x7f800000
    csrr a3, fflags
    TEST_CASE 4, a3, 0x09

    load_s f1, 0x7f7fffff
    load_s f2, 0x7f7fffff
    fadd.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 5, a3, 0x7f800000
    csrr a3, fflags
    TEST_CASE 6, a3, 0x0d

    load_s f1, 0x00000001
    load_s f2, 0x3f000000
    fmul.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 7, a3, 0
    csrr a3, fflags
    TEST_CASE 8, a3, 0x0f

    load_s f1, 0x00000000
    fdiv.s f3, f1, f1
    fmv.x.w a3, f3
    TEST_CASE 9, a3, 0x7fc00000
    csrr a3, fflags
    TEST_CASE 10, a3, 0x1f

    csrw fflags, zero
    load_s f1, 0x7fc00001
    load_s f2, 0x3f800000
    fadd.s f3, f1, f2
    csrr a3, fflags
    TEST_CASE 11, a3, 0
    fmv.x.w a3, f3
    TEST_CASE 12, a3, 0x7fc00000

    csrw fflags, zero
    load_s f1, 0x7f800001
    fadd.s f3, f1, f2
    csrr a3, fflags
    TEST_CASE 13, a3, 0x10

    csrw fflags, zero
    li t0, 0x1f
    fsflags a3, t0
    TEST_CASE 14, a3, 0
    frflags a3
    TEST_CASE 15, a3, 0x1f

    li t0, 2
    fsrm a3, t0
    TEST_CASE 16, a3, 0
    frrm a3
    TEST_CASE 17, a3, 2

    csrr a3, fcsr
    TEST_CASE 18, a3, 0x5f

    li t0, 0x51
    fscsr a3, t0
    TEST_CASE 19, a3, 0x5f
    frflags a3
    TEST_CASE 20, a3, 0x11
    frrm a3
    TEST_CASE 21, a3, 2
    csrr a3, fcsr
    TEST_CASE 22, a3, 0x51

    li t0, -1
    csrw fcsr, t0
    csrr a3, fcsr
    TEST_CASE 23, a3, 0xff
    csrr a3, fflags
    TEST_CASE 24, a3, 0x1f
    csrr a3, frm
    TEST_CASE 25, a3, 7

    call stop_by_fault
