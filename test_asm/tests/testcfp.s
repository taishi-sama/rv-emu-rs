.include "common.s"
.text
.global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    addi sp, sp, -32
    andi sp, sp, -16
    li s0, 0x80010000

    # 2: fadd.s 1.0 + 2.0
    li a1, 0x3f800000
    fmv.w.x f1, a1
    li a2, 0x40000000
    fmv.w.x f2, a2
    fadd.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 2, a3, 0x40400000

    # 3: fsub.s 5.0 - 2.0
    li a1, 0x40a00000
    fmv.w.x f1, a1
    fsub.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 3, a3, 0x40400000

    # 4: fmul.s 2.0 * 3.0
    li a1, 0x40000000
    fmv.w.x f1, a1
    li a2, 0x40400000
    fmv.w.x f2, a2
    fmul.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 4, a3, 0x40c00000

    # 5: fdiv.s 6.0 / 2.0
    li a1, 0x40c00000
    fmv.w.x f1, a1
    li a2, 0x40000000
    fmv.w.x f2, a2
    fdiv.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 5, a3, 0x40400000

    # 6: fsqrt.s 9.0
    li a1, 0x41100000
    fmv.w.x f1, a1
    fsqrt.s f3, f1
    fmv.x.w a3, f3
    TEST_CASE 6, a3, 0x40400000

    # 7: fsgnj.s 1.0 with -2.0 sign -> -1.0
    li a1, 0x3f800000
    fmv.w.x f1, a1
    li a2, 0xc0000000
    fmv.w.x f2, a2
    fsgnj.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 7, a3, 0xbf800000

    # 8: fsgnjn.s -> +1.0
    fsgnjn.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 8, a3, 0x3f800000

    # 9: fsgnjx.s -> -1.0
    fsgnjx.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 9, a3, 0xbf800000

    # 10: fmin.s(-0,+0) = -0
    li a1, 0x80000000
    fmv.w.x f1, a1
    li a2, 0x00000000
    fmv.w.x f2, a2
    fmin.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 10, a3, 0x80000000

    # 11: fmax.s(-0,+0) = +0
    fmax.s f3, f1, f2
    fmv.x.w a3, f3
    TEST_CASE 11, a3, 0x00000000

    # 12: feq.s 1.0,1.0
    li a1, 0x3f800000
    fmv.w.x f1, a1
    fmv.w.x f2, a1
    feq.s a3, f1, f2
    TEST_CASE 12, a3, 1

    # 13: flt.s 1.0,2.0
    li a2, 0x40000000
    fmv.w.x f2, a2
    flt.s a3, f1, f2
    TEST_CASE 13, a3, 1

    # 14: fle.s 2.0,1.0
    fle.s a3, f2, f1
    TEST_CASE 14, a3, 0

    # 15: fcvt.w.s 3.5 rne = 4
    li a1, 0x40600000
    fmv.w.x f1, a1
    fcvt.w.s a3, f1
    TEST_CASE 15, a3, 4

    # 16: fcvt.wu.s -1 = 0 with NV (saturated)
    li a1, 0xbf800000
    fmv.w.x f1, a1
    fcvt.wu.s a3, f1
    TEST_CASE 16, a3, 0

    # 17: fcvt.s.w -7
    li a1, -7
    fcvt.s.w f1, a1
    fmv.x.w a3, f1
    TEST_CASE 17, a3, 0xc0e00000

    # 18: fcvt.s.wu 0xffffffff
    li a1, -1
    fcvt.s.wu f1, a1
    fmv.x.w a3, f1
    TEST_CASE 18, a3, 0x4f800000

    # 19: fmv.w.x / fmv.x.w raw bits
    li a1, 0xdeadbeef
    fmv.w.x f1, a1
    fmv.x.w a3, f1
    TEST_CASE 19, a3, 0xdeadbeef

    # 20: fclass.s +0 -> 0x10
    li a1, 0x00000000
    fmv.w.x f1, a1
    fclass.s a3, f1
    TEST_CASE 20, a3, 0x10

    # 21: fclass.s -inf -> 0x01
    li a1, 0xff800000
    fmv.w.x f1, a1
    fclass.s a3, f1
    TEST_CASE 21, a3, 0x01

    # 22: fclass.s qNaN -> 0x200
    li a1, 0x7fc00000
    fmv.w.x f1, a1
    fclass.s a3, f1
    TEST_CASE 22, a3, 0x200

    # 23: fclass.s sNaN -> 0x100
    li a1, 0x7f800001
    fmv.w.x f1, a1
    fclass.s a3, f1
    TEST_CASE 23, a3, 0x100

    # 24: flw/fsw
    li a1, 0x40490fdb
    sw a1, 0(s0)
    flw f1, 0(s0)
    fsw f1, 4(s0)
    lw a3, 4(s0)
    TEST_CASE 24, a3, 0x40490fdb

    # 25: fadd.d 1.5 + 2.5 = 4.0
    li a1, 1
    fcvt.d.w f1, a1
    li a2, 2
    fcvt.d.w f2, a2
    # 1.5 = 1.0 + 0.5
    li a3, 0x3f000000
    fmv.w.x f3, a3
    fcvt.d.s f3, f3
    fadd.d f1, f1, f3
    # 2.5 = 2.0 + 0.5
    fadd.d f2, f2, f3
    fadd.d f4, f1, f2
    fsd f4, 8(s0)
    lw a3, 8(s0)
    lw a4, 12(s0)
    TEST_CASE 25, a3, 0
    li t6, 0x40100000
    beq a4, t6, 1f
    fail 26
    j 2f
1:  pass 26
2:
    # 27: fcvt.w.d 4.0 = 4
    fcvt.w.d a3, f4
    TEST_CASE 27, a3, 4

    # 28: fcvt.d.w 7
    li a1, 7
    fcvt.d.w f5, a1
    fsd f5, 16(s0)
    lw a3, 16(s0)
    lw a4, 20(s0)
    TEST_CASE 28, a3, 0
    li t6, 0x401c0000
    beq a4, t6, 3f
    fail 29
    j 4f
3:  pass 29
4:
    # 30: fcvt.s.d 4.0 -> 4.0f
    fcvt.s.d f6, f4
    fmv.x.w a3, f6
    TEST_CASE 30, a3, 0x40800000

    # 31: fmadd.s 2*3+4 = 10
    li a1, 0x40000000
    fmv.w.x f1, a1
    li a2, 0x40400000
    fmv.w.x f2, a2
    li a3, 0x40800000
    fmv.w.x f3, a3
    fmadd.s f4, f1, f2, f3
    fmv.x.w a3, f4
    TEST_CASE 31, a3, 0x41200000

    # 32: fmsub.s 2*3-4 = 2
    fmsub.s f4, f1, f2, f3
    fmv.x.w a3, f4
    TEST_CASE 32, a3, 0x40000000

    # 33: fnmsub.s -(2*3)+4 = -2
    fnmsub.s f4, f1, f2, f3
    fmv.x.w a3, f4
    TEST_CASE 33, a3, 0xc0000000

    # 34: fnmadd.s -(2*3)-4 = -10
    fnmadd.s f4, f1, f2, f3
    fmv.x.w a3, f4
    TEST_CASE 34, a3, 0xc1200000

    # 35: compressed flw/fsw
    li a1, 0x3f800000
    sw a1, 0(s0)
    c.flw f8, 0(s0)
    c.fsw f8, 4(s0)
    lw a3, 4(s0)
    TEST_CASE 35, a3, 0x3f800000

    # 36: compressed fld/fsd
    li a1, 0
    sw a1, 8(s0)
    li a1, 0x3ff00000
    sw a1, 12(s0)
    c.fld f8, 8(s0)
    c.fsd f8, 16(s0)
    lw a3, 16(s0)
    lw a4, 20(s0)
    TEST_CASE 36, a3, 0
    li t6, 0x3ff00000
    beq a4, t6, 5f
    fail 37
    j 6f
5:  pass 37
6:
    # 38: compressed stack forms
    li a1, 0x40000000
    c.swsp a1, 0(sp)
    c.flwsp f9, 0(sp)
    c.fswsp f9, 4(sp)
    c.lwsp a3, 4(sp)
    TEST_CASE 38, a3, 0x40000000

    li a1, 0x40000000
    sw a1, 0(sp)
    li a1, 0x40100000
    sw a1, 4(sp)
    c.fldsp f9, 0(sp)
    c.fsdsp f9, 8(sp)
    c.lwsp a3, 8(sp)
    c.lwsp a4, 12(sp)
    TEST_CASE 39, a3, 0x40000000
    li t6, 0x40100000
    beq a4, t6, 7f
    fail 40
    j 8f
7:  pass 40
8:
    call stop_by_fault
