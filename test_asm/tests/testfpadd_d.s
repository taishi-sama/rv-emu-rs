.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0x40040000
    fadd.d f3, f1, f2
    test_d_case 2, 3, f3, 0, 0x40100000

    load_d f1, 0, 0xbff80000
    load_d f2, 0, 0x3ff80000
    fadd.d f3, f1, f2
    test_d_case 4, 5, f3, 0, 0

    csrw fflags, zero
    load_d f1, 0, 0x7ff00000
    load_d f2, 0, 0xfff00000
    fadd.d f3, f1, f2
    test_d_case 6, 7, f3, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 8, a3, 0x10

    call stop_by_fault
