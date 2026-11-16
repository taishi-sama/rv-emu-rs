.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x40000000
    load_d f2, 0, 0x40080000
    load_d f3, 0, 0x40100000
    fmadd.d f4, f1, f2, f3
    test_d_case 2, 3, f4, 0, 0x40240000

    fmsub.d f4, f1, f2, f3
    test_d_case 4, 5, f4, 0, 0x40000000

    fnmsub.d f4, f1, f2, f3
    test_d_case 6, 7, f4, 0, 0xc0000000

    fnmadd.d f4, f1, f2, f3
    test_d_case 8, 9, f4, 0, 0xc0240000

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0x40040000
    load_d f3, 0, 0x3fe00000
    fmadd.d f4, f1, f2, f3
    test_d_case 10, 11, f4, 0, 0x40110000

    csrw fflags, zero
    load_d f1, 0, 0
    load_d f2, 0, 0x7ff00000
    load_d f3, 0, 0x3ff00000
    fmadd.d f4, f1, f2, f3
    test_d_case 12, 13, f4, 0, 0x7ff80000
    csrr a3, fflags
    TEST_CASE 14, a3, 0x10

    call stop_by_fault
