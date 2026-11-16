.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0xc0040000
    fsgnj.d f3, f1, f2
    test_d_case 2, 3, f3, 0, 0xbff80000

    fsgnjn.d f3, f1, f2
    test_d_case 4, 5, f3, 0, 0x3ff80000

    fsgnjx.d f3, f1, f2
    test_d_case 6, 7, f3, 0, 0xbff80000

    load_d f4, 0x00000123, 0x7ff80000
    fsgnj.d f5, f4, f1
    test_d_case 8, 9, f5, 0x00000123, 0x7ff80000

    fsgnj.d f5, f4, f2
    test_d_case 10, 11, f5, 0x00000123, 0xfff80000

    csrr a3, fflags
    TEST_CASE 12, a3, 0

    load_d f4, 1, 0x7ff00000
    fsgnj.d f5, f4, f1
    test_d_case 13, 14, f5, 1, 0x7ff00000
    csrr a3, fflags
    TEST_CASE 15, a3, 0

    call stop_by_fault
