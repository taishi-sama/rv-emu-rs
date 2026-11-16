.include "common.s"

.text
    .global __start
__start:
    la t0, trap_handler
    csrw mtvec, t0
    la s0, data
    li t0, 0x6000
    csrs mstatus, t0

    li t0, 0x6000
    csrc mstatus, t0

    li s1, 0
    la t1, .Lflw
.Lflw:
    c.flw f8, 0(s0)
    TEST_CASE 2, s1, 2
    sub t0, s2, t1
    TEST_CASE 3, t0, 0

    li s1, 0
    la t1, .Lfsw
.Lfsw:
    c.fsw f8, 0(s0)
    TEST_CASE 4, s1, 2
    sub t0, s2, t1
    TEST_CASE 5, t0, 0

    li s1, 0
    la t1, .Lfld
.Lfld:
    c.fld f8, 0(s0)
    TEST_CASE 6, s1, 2
    sub t0, s2, t1
    TEST_CASE 7, t0, 0

    li s1, 0
    la t1, .Lfsd
.Lfsd:
    c.fsd f8, 0(s0)
    TEST_CASE 8, s1, 2
    sub t0, s2, t1
    TEST_CASE 9, t0, 0

    li s1, 0
    la t1, .Lflwsp
.Lflwsp:
    c.flwsp f8, 0(sp)
    TEST_CASE 10, s1, 2
    sub t0, s2, t1
    TEST_CASE 11, t0, 0

    li s1, 0
    la t1, .Lfswsp
.Lfswsp:
    c.fswsp f8, 0(sp)
    TEST_CASE 12, s1, 2
    sub t0, s2, t1
    TEST_CASE 13, t0, 0

    li s1, 0
    la t1, .Lfldsp
.Lfldsp:
    c.fldsp f8, 0(sp)
    TEST_CASE 14, s1, 2
    sub t0, s2, t1
    TEST_CASE 15, t0, 0

    li s1, 0
    la t1, .Lfsdsp
.Lfsdsp:
    c.fsdsp f8, 0(sp)
    TEST_CASE 16, s1, 2
    sub t0, s2, t1
    TEST_CASE 17, t0, 0

    li t0, 0x6000
    csrs mstatus, t0
    la sp, data

    li t0, 0x3f800000
    sw t0, 0(sp)
    c.flwsp f0, 0(sp)
    fmv.x.w a3, f0
    TEST_CASE 18, a3, 0x3f800000

    sw zero, 8(sp)
    li t0, 0x3ff00000
    sw t0, 12(sp)
    c.fldsp f0, 8(sp)
    c.fsdsp f0, 16(sp)
    lw t0, 16(sp)
    TEST_CASE 19, t0, 0
    lw t0, 20(sp)
    TEST_CASE 20, t0, 0x3ff00000

    load_s f0, 0x40490fdb
    c.fswsp f0, 24(sp)
    lw t0, 24(sp)
    TEST_CASE 21, t0, 0x40490fdb

    sw zero, 32(sp)
    li t0, 0x3fe00000
    sw t0, 36(sp)
    c.fldsp f0, 32(sp)
    c.fsdsp f0, 40(sp)
    lw t0, 40(sp)
    TEST_CASE 22, t0, 0
    lw t0, 44(sp)
    TEST_CASE 23, t0, 0x3fe00000

    li s0, 0
    li s1, 0
    la t1, .Lflw_fault
.Lflw_fault:
    c.flw f8, 0(s0)
    TEST_CASE 24, s1, 5
    sub t0, s2, t1
    TEST_CASE 25, t0, 0

    li s1, 0
    la t1, .Lfsw_fault
.Lfsw_fault:
    c.fsw f8, 0(s0)
    TEST_CASE 26, s1, 7
    sub t0, s2, t1
    TEST_CASE 27, t0, 0

    li s1, 0
    la t1, .Lfld_fault
.Lfld_fault:
    c.fld f8, 0(s0)
    TEST_CASE 28, s1, 5
    sub t0, s2, t1
    TEST_CASE 29, t0, 0

    li s1, 0
    la t1, .Lfsd_fault
.Lfsd_fault:
    c.fsd f8, 0(s0)
    TEST_CASE 30, s1, 7
    sub t0, s2, t1
    TEST_CASE 31, t0, 0

    call stop_by_fault

.align 2
trap_handler:
    csrr s1, mcause
    csrr s2, mepc
    addi t0, s2, 2
    csrw mepc, t0
    mret

.data
.align 3
    .zero 16
data:
    .zero 64
