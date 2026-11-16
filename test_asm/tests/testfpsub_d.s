.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_d f1, 0, 0x40140000
    load_d f2, 0, 0x40000000
    fsub.d f3, f1, f2
    test_d_case 2, 3, f3, 0, 0x40080000

    load_d f1, 0, 0x40000000
    load_d f2, 0, 0x40140000
    fsub.d f3, f1, f2
    test_d_case 4, 5, f3, 0, 0xc0080000

    load_d f1, 0, 0x3ff80000
    load_d f2, 0, 0x3ff80000
    fsub.d f3, f1, f2
    test_d_case 6, 7, f3, 0, 0

    load_d f1, 0, 0x80000000
    load_d f2, 0, 0
    fsub.d f3, f1, f2
    test_d_case 8, 9, f3, 0, 0x80000000

    call stop_by_fault
