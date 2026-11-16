.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x40180000
    load_d f2, 0, 0x40000000
    fdiv.d f3, f1, f2
    test_d_case 2, 3, f3, 0, 0x40080000

    csrw fflags, zero
    load_d f1, 0, 0x3ff00000
    load_d f2, 0, 0
    fdiv.d f3, f1, f2
    test_d_case 4, 5, f3, 0, 0x7ff00000
    csrr a3, fflags
    TEST_CASE 6, a3, 0x08

    load_d f1, 0, 0xbff00000
    fdiv.d f3, f1, f2
    test_d_case 7, 8, f3, 0, 0xfff00000

    csrw fflags, zero
    load_d f1, 0, 0
    fdiv.d f3, f1, f2
    test_d_case 9, 10, f3, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 11, a3, 0x10

    call stop_by_fault
