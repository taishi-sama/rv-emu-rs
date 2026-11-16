.include "common.s"

.text
    .global __start
__start:
    li t0, 0x6000
    csrs mstatus, t0
    andi sp, sp, -16

    load_s f1, 0xff800000
    fclass.s a3, f1
    TEST_CASE 2, a3, 0x001
    load_s f1, 0xbf800000
    fclass.s a3, f1
    TEST_CASE 3, a3, 0x002
    load_s f1, 0x80000001
    fclass.s a3, f1
    TEST_CASE 4, a3, 0x004
    load_s f1, 0x80000000
    fclass.s a3, f1
    TEST_CASE 5, a3, 0x008
    load_s f1, 0x00000000
    fclass.s a3, f1
    TEST_CASE 6, a3, 0x010
    load_s f1, 0x00000001
    fclass.s a3, f1
    TEST_CASE 7, a3, 0x020
    load_s f1, 0x3f800000
    fclass.s a3, f1
    TEST_CASE 8, a3, 0x040
    load_s f1, 0x7f800000
    fclass.s a3, f1
    TEST_CASE 9, a3, 0x080
    load_s f1, 0x7f800001
    fclass.s a3, f1
    TEST_CASE 10, a3, 0x100
    load_s f1, 0x7fc00000
    fclass.s a3, f1
    TEST_CASE 11, a3, 0x200

    load_d f1, 0, 0xfff00000
    fclass.d a3, f1
    TEST_CASE 12, a3, 0x001
    load_d f1, 0, 0xbff00000
    fclass.d a3, f1
    TEST_CASE 13, a3, 0x002
    load_d f1, 1, 0x80000000
    fclass.d a3, f1
    TEST_CASE 14, a3, 0x004
    load_d f1, 0, 0x80000000
    fclass.d a3, f1
    TEST_CASE 15, a3, 0x008
    load_d f1, 0, 0
    fclass.d a3, f1
    TEST_CASE 16, a3, 0x010
    load_d f1, 1, 0
    fclass.d a3, f1
    TEST_CASE 17, a3, 0x020
    load_d f1, 0, 0x3ff00000
    fclass.d a3, f1
    TEST_CASE 18, a3, 0x040
    load_d f1, 0, 0x7ff00000
    fclass.d a3, f1
    TEST_CASE 19, a3, 0x080
    load_d f1, 1, 0x7ff00000
    fclass.d a3, f1
    TEST_CASE 20, a3, 0x100
    load_d f1, 0, 0x7ff80000
    fclass.d a3, f1
    TEST_CASE 21, a3, 0x200

    call stop_by_fault
