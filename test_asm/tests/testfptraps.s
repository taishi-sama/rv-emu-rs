.include "common.s"

.text
    .global __start
__start:
    la t0, trap_handler
    csrw mtvec, t0
    la s0, data
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    li t0, 0x6000
    csrc mstatus, t0

    li s1, 0
    la t1, .Lfadd_off
.Lfadd_off:
    fadd.s f1, f2, f3
    TEST_CASE 2, s1, 2
    sub t0, s2, t1
    TEST_CASE 3, t0, 0

    li s1, 0
    la t1, .Lflw_off
.Lflw_off:
    flw f1, 0(s0)
    TEST_CASE 4, s1, 2
    sub t0, s2, t1
    TEST_CASE 5, t0, 0

    li s1, 0
    la t1, .Lfsw_off
.Lfsw_off:
    fsw f1, 0(s0)
    TEST_CASE 6, s1, 2
    sub t0, s2, t1
    TEST_CASE 7, t0, 0

    li s1, 0
    la t1, .Lfld_off
.Lfld_off:
    fld f1, 0(s0)
    TEST_CASE 8, s1, 2
    sub t0, s2, t1
    TEST_CASE 9, t0, 0

    li s1, 0
    la t1, .Lfsd_off
.Lfsd_off:
    fsd f1, 0(s0)
    TEST_CASE 10, s1, 2
    sub t0, s2, t1
    TEST_CASE 11, t0, 0

    li s1, 0
    la t1, .Lfmadd_off
.Lfmadd_off:
    fmadd.s f1, f2, f3, f4
    TEST_CASE 12, s1, 2
    sub t0, s2, t1
    TEST_CASE 13, t0, 0

    li s1, 0
    la t1, .Lcsrr_off
.Lcsrr_off:
    csrr a3, fcsr
    TEST_CASE 14, s1, 2
    sub t0, s2, t1
    TEST_CASE 15, t0, 0

    li s1, 0
    la t1, .Lcsrw_off
.Lcsrw_off:
    csrw frm, zero
    TEST_CASE 16, s1, 2
    sub t0, s2, t1
    TEST_CASE 17, t0, 0

    li t0, 0x6000
    csrs mstatus, t0
    load_s f1, 0x3f800000
    load_s f2, 0x40000000
    li t0, 0x6000
    csrc mstatus, t0
    li t0, 0x2000
    csrs mstatus, t0
    fadd.s f3, f1, f2
    csrr s3, mstatus
    fmv.x.w a3, f3
    TEST_CASE 18, a3, 0x40400000
    srli a3, s3, 13
    andi a3, a3, 3
    TEST_CASE 19, a3, 3

    li s1, 0
    la t1, .Lrm5
.Lrm5:
    .word 0x003150d3
    TEST_CASE 20, s1, 2
    sub t0, s2, t1
    TEST_CASE 21, t0, 0

    li s1, 0
    la t1, .Lrm6
.Lrm6:
    .word 0x003160d3
    TEST_CASE 22, s1, 2
    sub t0, s2, t1
    TEST_CASE 23, t0, 0

    li s1, 0
    la t1, .Lfmt2
.Lfmt2:
    .word 0x043100d3
    TEST_CASE 24, s1, 2
    sub t0, s2, t1
    TEST_CASE 25, t0, 0

    li s1, 0
    la t1, .Lfmafmt2
.Lfmafmt2:
    .word 0x243100c3
    TEST_CASE 26, s1, 2
    sub t0, s2, t1
    TEST_CASE 27, t0, 0

    li s1, 0
    la t1, .Lcvtwi
.Lcvtwi:
    .word 0xc0208553
    TEST_CASE 28, s1, 2
    sub t0, s2, t1
    TEST_CASE 29, t0, 0

    li s1, 0
    la t1, .Lcvtiw
.Lcvtiw:
    .word 0xd02500d3
    TEST_CASE 30, s1, 2
    sub t0, s2, t1
    TEST_CASE 31, t0, 0

    li s1, 0
    la t1, .Lfmvdx
.Lfmvdx:
    .word 0xf20500d3
    TEST_CASE 32, s1, 2
    sub t0, s2, t1
    TEST_CASE 33, t0, 0

    li s1, 0
    la t1, .Lfmvxd
.Lfmvxd:
    .word 0xe2008553
    TEST_CASE 34, s1, 2
    sub t0, s2, t1
    TEST_CASE 35, t0, 0

    li s1, 0
    la t1, .Lsgnj3
.Lsgnj3:
    .word 0x203130d3
    TEST_CASE 36, s1, 2
    sub t0, s2, t1
    TEST_CASE 37, t0, 0

    li s1, 0
    la t1, .Lmin2
.Lmin2:
    .word 0x283120d3
    TEST_CASE 38, s1, 2
    sub t0, s2, t1
    TEST_CASE 39, t0, 0

    li s1, 0
    la t1, .Lfeq3
.Lfeq3:
    .word 0xa020b553
    TEST_CASE 40, s1, 2
    sub t0, s2, t1
    TEST_CASE 41, t0, 0

    li s1, 0
    la t1, .Lclass2
.Lclass2:
    .word 0xe000a553
    TEST_CASE 42, s1, 2
    sub t0, s2, t1
    TEST_CASE 43, t0, 0

    li s1, 0
    la t1, .Lflw1
.Lflw1:
    .word 0x00051087
    TEST_CASE 44, s1, 2
    sub t0, s2, t1
    TEST_CASE 45, t0, 0

    li s1, 0
    la t1, .Lfsw1
.Lfsw1:
    .word 0x00151027
    TEST_CASE 46, s1, 2
    sub t0, s2, t1
    TEST_CASE 47, t0, 0

    li s0, 0
    li s1, 0
    la t1, .Lflw_fault
.Lflw_fault:
    flw f1, 0(s0)
    TEST_CASE 48, s1, 5
    sub t0, s2, t1
    TEST_CASE 49, t0, 0

    li s1, 0
    la t1, .Lfsw_fault
.Lfsw_fault:
    fsw f1, 0(s0)
    TEST_CASE 50, s1, 7
    sub t0, s2, t1
    TEST_CASE 51, t0, 0

    li s1, 0
    la t1, .Lfld_fault
.Lfld_fault:
    fld f1, 0(s0)
    TEST_CASE 52, s1, 5
    sub t0, s2, t1
    TEST_CASE 53, t0, 0

    li s1, 0
    la t1, .Lfsd_fault
.Lfsd_fault:
    fsd f1, 0(s0)
    TEST_CASE 54, s1, 7
    sub t0, s2, t1
    TEST_CASE 55, t0, 0

    la s0, data
    load_s f1, 0x3f800000
    li t0, 0x11223344
    sw t0, 0(s0)

    li t0, 0x6000
    csrc mstatus, t0
    li s1, 0
    la t1, .Lfsw_no_effect
.Lfsw_no_effect:
    fsw f1, 0(s0)
    TEST_CASE 56, s1, 2
    sub t0, s2, t1
    TEST_CASE 57, t0, 0
    lw t0, 0(s0)
    TEST_CASE 58, t0, 0x11223344
    csrr a3, mstatus
    li t0, 0x6000
    and a3, a3, t0
    TEST_CASE 59, a3, 0

    li t0, 0x6000
    csrs mstatus, t0
    load_d f1, 0, 0x40040000
    li t0, 0x6000
    csrc mstatus, t0
    li s1, 0
    la t1, .Lfld_no_effect
.Lfld_no_effect:
    fld f1, 0(s0)
    TEST_CASE 60, s1, 2
    sub t0, s2, t1
    TEST_CASE 61, t0, 0
    li t0, 0x6000
    csrs mstatus, t0
    fsd f1, 8(s0)
    lw t0, 8(s0)
    TEST_CASE 62, t0, 0
    lw t0, 12(s0)
    TEST_CASE 63, t0, 0x40040000

    li t0, 2
    csrw frm, t0
    li t0, 0x6000
    csrc mstatus, t0
    li s1, 0
    la t1, .Lfrm_no_effect
.Lfrm_no_effect:
    csrw frm, zero
    TEST_CASE 64, s1, 2
    sub t0, s2, t1
    TEST_CASE 65, t0, 0
    li t0, 0x6000
    csrs mstatus, t0
    csrr a3, frm
    TEST_CASE 66, a3, 2

    li t0, 7
    csrw frm, t0
    li s1, 0
    la t1, .Ldyn_rm
.Ldyn_rm:
    fadd.s f3, f1, f1
    TEST_CASE 67, s1, 2
    sub t0, s2, t1
    TEST_CASE 68, t0, 0
    csrr a3, frm
    TEST_CASE 69, a3, 7

    li t0, 0x6000
    csrc mstatus, t0
    li t0, 0x2000
    csrs mstatus, t0
    flw f4, 0(s0)
    csrr s3, mstatus
    srli s3, s3, 13
    andi s3, s3, 3
    TEST_CASE 70, s3, 3

    li t0, 0x6000
    csrc mstatus, t0
    li t0, 0x2000
    csrs mstatus, t0
    fsw f4, 4(s0)
    csrr s3, mstatus
    srli s3, s3, 13
    andi s3, s3, 3
    TEST_CASE 71, s3, 3

    li t0, 0x6000
    csrc mstatus, t0
    csrr a3, mstatus
    li t0, 0x80000000
    and a3, a3, t0
    TEST_CASE 72, a3, 0

    call stop_by_fault

.align 2
trap_handler:
    csrr s1, mcause
    csrr s2, mepc
    addi t0, s2, 4
    csrw mepc, t0
    mret

.data
.align 3
data:
    .zero 16
