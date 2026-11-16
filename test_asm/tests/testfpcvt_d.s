.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    csrw fflags, zero
    load_d f1, 0, 0x41edcd65
    fcvt.wu.d a3, f1
    TEST_CASE 2, a3, 4000000000
    csrr a3, fflags
    TEST_CASE 3, a3, 0

    csrw fflags, zero
    load_d f1, 0, 0xbff00000
    fcvt.wu.d a3, f1
    TEST_CASE 4, a3, 0
    csrr a3, fflags
    TEST_CASE 5, a3, 0x10

    load_d f1, 0xffe00000, 0x41efffff
    fcvt.wu.d a3, f1
    TEST_CASE 6, a3, 0xffffffff

    li a1, -1
    fcvt.d.wu f2, a1
    test_d_case 7, 8, f2, 0xffe00000, 0x41efffff

    csrw fflags, zero
    load_d f1, 0, 0xc00c0000
    fcvt.w.d a3, f1
    TEST_CASE 9, a3, -4
    csrr a3, fflags
    TEST_CASE 10, a3, 0x01

    load_d f1, 0, 0x400c0000
    fcvt.w.d a3, f1
    TEST_CASE 11, a3, 4

    li a1, -1
    fcvt.d.w f2, a1
    test_d_case 12, 13, f2, 0, 0xbff00000

    li a1, 7
    fcvt.d.w f2, a1
    test_d_case 14, 15, f2, 0, 0x401c0000

    load_d f1, 0, 0x3ff80000
    fcvt.s.d f2, f1
    fmv.x.w a3, f2
    TEST_CASE 16, a3, 0x3fc00000

    load_s f1, 0x3fc00000
    fcvt.d.s f2, f1
    test_d_case 17, 18, f2, 0, 0x3ff80000

    csrw fflags, zero
    load_d f1, 0, 0x36900000
    fcvt.s.d f2, f1
    fmv.x.w a3, f2
    TEST_CASE 19, a3, 0
    csrr a3, fflags
    TEST_CASE 20, a3, 0x03

    csrw fflags, zero
    load_d f1, 0x8800759c, 0x7e37e43c
    fcvt.s.d f2, f1
    fmv.x.w a3, f2
    TEST_CASE 21, a3, 0x7f800000
    csrr a3, fflags
    TEST_CASE 22, a3, 0x05

    csrw fflags, zero
    load_s f1, 0xbf800000
    fcvt.wu.s a3, f1
    TEST_CASE 23, a3, 0
    csrr a3, fflags
    TEST_CASE 24, a3, 0x10

    csrw fflags, zero
    load_s f1, 0x7fc00000
    fcvt.w.s a3, f1
    TEST_CASE 25, a3, 0x7fffffff
    csrr a3, fflags
    TEST_CASE 26, a3, 0x10

    csrw fflags, zero
    load_s f1, 0x60ad78ec
    fcvt.w.s a3, f1
    TEST_CASE 27, a3, 0x7fffffff
    csrr a3, fflags
    TEST_CASE 28, a3, 0x10

    csrw fflags, zero
    load_s f1, 0xff800000
    fcvt.w.s a3, f1
    TEST_CASE 29, a3, 0x80000000
    csrr a3, fflags
    TEST_CASE 30, a3, 0x10

    csrw fflags, zero
    load_d f1, 0xffe00000, 0x41efffff
    fcvt.wu.d a3, f1
    TEST_CASE 31, a3, 0xffffffff
    csrr a3, fflags
    TEST_CASE 32, a3, 0

    csrw fflags, zero
    load_d f1, 0x20000000, 0x4202a05f
    fcvt.w.d a3, f1
    TEST_CASE 33, a3, 0x7fffffff
    csrr a3, fflags
    TEST_CASE 34, a3, 0x10

    csrw fflags, zero
    fcvt.wu.d a3, f1
    TEST_CASE 35, a3, 0xffffffff
    csrr a3, fflags
    TEST_CASE 36, a3, 0x10

    csrw fflags, zero
    load_d f1, 0, 0x7ff00000
    fcvt.w.d a3, f1
    TEST_CASE 37, a3, 0x7fffffff
    csrr a3, fflags
    TEST_CASE 38, a3, 0x10

    csrw fflags, zero
    load_d f1, 0xf0000000, 0x380fffff
    fcvt.s.d f2, f1
    fmv.x.w a3, f2
    TEST_CASE 39, a3, 0x00800000
    csrr a3, fflags
    TEST_CASE 40, a3, 0x01

    csrw fflags, zero
    load_d f1, 0, 0x7ff00000
    fcvt.wu.d a3, f1
    TEST_CASE 41, a3, 0xffffffff
    csrr a3, fflags
    TEST_CASE 42, a3, 0x10

    call stop_by_fault
