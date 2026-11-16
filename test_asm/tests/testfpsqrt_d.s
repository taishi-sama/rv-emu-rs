.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x40220000
    fsqrt.d f2, f1
    test_d_case 2, 3, f2, 0, 0x40080000

    load_d f1, 0, 0x40000000
    fsqrt.d f2, f1
    test_d_case 4, 5, f2, 0x667f3bcd, 0x3ff6a09e

    csrw fflags, zero
    load_d f1, 0, 0xbff00000
    fsqrt.d f2, f1
    test_d_case 6, 7, f2, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 8, a3, 0x10

    csrw fflags, zero
    load_d f1, 0, 0x80000000
    fsqrt.d f2, f1
    test_d_case 9, 10, f2, 0, 0x80000000
    csrr a3, fflags
    TEST_CASE 11, a3, 0

    call stop_by_fault
