.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0

    load_s f1, 0x3f800000
    load_s f2, 0x33800000
    fadd.s f3, f1, f2, rne
    fmv.x.w a3, f3
    TEST_CASE 2, a3, 0x3f800000
    fadd.s f3, f1, f2, rtz
    fmv.x.w a3, f3
    TEST_CASE 3, a3, 0x3f800000
    fadd.s f3, f1, f2, rdn
    fmv.x.w a3, f3
    TEST_CASE 4, a3, 0x3f800000
    fadd.s f3, f1, f2, rup
    fmv.x.w a3, f3
    TEST_CASE 5, a3, 0x3f800001
    fadd.s f3, f1, f2, rmm
    fmv.x.w a3, f3
    TEST_CASE 6, a3, 0x3f800001

    load_s f1, 0xbf800000
    load_s f2, 0xb3800000
    fadd.s f3, f1, f2, rne
    fmv.x.w a3, f3
    TEST_CASE 7, a3, 0xbf800000
    fadd.s f3, f1, f2, rdn
    fmv.x.w a3, f3
    TEST_CASE 8, a3, 0xbf800001
    fadd.s f3, f1, f2, rup
    fmv.x.w a3, f3
    TEST_CASE 9, a3, 0xbf800000
    fadd.s f3, f1, f2, rmm
    fmv.x.w a3, f3
    TEST_CASE 10, a3, 0xbf800001

    load_s f1, 0x40200000
    fcvt.w.s a3, f1, rne
    TEST_CASE 11, a3, 2
    fcvt.w.s a3, f1, rtz
    TEST_CASE 12, a3, 2
    fcvt.w.s a3, f1, rdn
    TEST_CASE 13, a3, 2
    fcvt.w.s a3, f1, rup
    TEST_CASE 14, a3, 3
    fcvt.w.s a3, f1, rmm
    TEST_CASE 15, a3, 3

    load_s f1, 0xc0200000
    fcvt.w.s a3, f1, rne
    TEST_CASE 16, a3, -2
    fcvt.w.s a3, f1, rdn
    TEST_CASE 17, a3, -3
    fcvt.w.s a3, f1, rup
    TEST_CASE 18, a3, -2
    fcvt.w.s a3, f1, rmm
    TEST_CASE 19, a3, -3

    load_s f1, 0x3f800000
    load_s f2, 0x33800000
    li t0, 3
    csrw frm, t0
    fadd.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 20, a3, 0x3f800001
    fadd.s f3, f1, f2, rtz
    fmv.x.w a3, f3
    TEST_CASE 21, a3, 0x3f800000

    li a1, 16777217
    fcvt.s.w f2, a1, rne
    fmv.x.w a3, f2
    TEST_CASE 22, a3, 0x4b800000
    fcvt.s.w f2, a1, rup
    fmv.x.w a3, f2
    TEST_CASE 23, a3, 0x4b800001
    fcvt.s.w f2, a1, rtz
    fmv.x.w a3, f2
    TEST_CASE 24, a3, 0x4b800000

    call stop_by_fault
