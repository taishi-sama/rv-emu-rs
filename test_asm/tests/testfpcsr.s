.include "common.s"
.text
.global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    addi sp, sp, -32

    # frm = RTZ
    li t0, 1
    csrw frm, t0
    li a1, 0x40600000       # 3.5
    fmv.w.x f1, a1
    fcvt.w.s a3, f1
    TEST_CASE 2, a3, 3

    # fflags.NX should be set, end result 0x1
    csrr a3, fflags
    TEST_CASE 3, a3, 0x1

    # fcsr = NX | (frm=1)
    csrr a3, fcsr
    TEST_CASE 4, a3, 0x21

    # mstatus FS=Dirty and SD
    csrr a3, mstatus
    li t6, 0x80006000
    and a3, a3, t6
    TEST_CASE 5, a3, 0x80006000

    # explicit rm=RNE overrides frm=RTZ: 3.5 -> 4
    fcvt.w.s a3, f1, rne
    TEST_CASE 6, a3, 4

    # fflags gets NX again (sticky)
    csrr a3, fflags
    TEST_CASE 7, a3, 0x1

    # sstatus view includes FS and SD
    csrr a3, sstatus
    li t6, 0x80006000
    and a3, a3, t6
    TEST_CASE 8, a3, 0x80006000

    # misa advertises F and D
    csrr a3, misa
    li t6, 0x28
    and a3, a3, t6
    TEST_CASE 9, a3, 0x28

    call stop_by_fault
