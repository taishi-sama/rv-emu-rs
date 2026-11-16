.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    la s0, data

    li t0, 0x3f800000
    sw t0, 0(s0)
    sw zero, 4(s0)
    fld f1, 0(s0)

    fadd.s f2, f1, f1
    fmv.x.w a3, f2
    TEST_CASE 2, a3, 0x7fc00000

    fclass.s a3, f1
    TEST_CASE 3, a3, 0x200

    fsgnj.s f2, f1, f1
    fmv.x.w a3, f2
    TEST_CASE 4, a3, 0x7fc00000

    fsw f1, 8(s0)
    lw a3, 8(s0)
    TEST_CASE 5, a3, 0x3f800000

    load_s f1, 0x7fc00001
    load_s f2, 0x3f800000
    fadd.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 6, a3, 0x7fc00000

    load_s f2, 0xbf800000
    fsgnj.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 7, a3, 0xffc00001

    fsgnjx.s f3, f1, f1
    fmv.x.w a3, f3
    TEST_CASE 8, a3, 0x7fc00001

    fld f4, 0(s0)
    fmv.x.w a3, f4
    TEST_CASE 9, a3, 0x3f800000

    li t0, 0x40490fdb
    sw t0, 12(s0)
    flw f1, 12(s0)
    fsd f1, 16(s0)
    lw a3, 16(s0)
    TEST_CASE 10, a3, 0x40490fdb
    lw a3, 20(s0)
    TEST_CASE 11, a3, 0xffffffff

    li t0, 0x3f800000
    fmv.w.x f5, t0
    fsd f5, 24(s0)
    lw a3, 24(s0)
    TEST_CASE 12, a3, 0x3f800000
    lw a3, 28(s0)
    TEST_CASE 13, a3, 0xffffffff

    call stop_by_fault

.data
.align 3
data:
    .zero 32
