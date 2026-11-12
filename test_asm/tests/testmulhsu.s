.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, mulhsu, 0x00000000, 0x00000000, 0x00000000
    TEST_RR_OP 3, mulhsu, 0x00000000, 0x00000001, 0x00000001
    TEST_RR_OP 4, mulhsu, 0x00000000, 0x00000003, 0x00000007

    TEST_RR_OP 5, mulhsu, 0x00000000, 0x00000000, 0xffff8000
    TEST_RR_OP 6, mulhsu, 0x00000000, 0x80000000, 0x00000000
    TEST_RR_OP 7, mulhsu, 0x80004000, 0x80000000, 0xffff8000

    TEST_RR_OP 30, mulhsu, 0xffff0081, 0xaaaaaaab, 0x0002fe7d
    TEST_RR_OP 31, mulhsu, 0x0001fefe, 0x0002fe7d, 0xaaaaaaab

    TEST_RR_OP 32, mulhsu, 0xff010000, 0xff000000, 0xff000000

    TEST_RR_OP 33, mulhsu, 0xffffffff, 0xffffffff, 0xffffffff
    TEST_RR_OP 34, mulhsu, 0xffffffff, 0xffffffff, 0x00000001
    TEST_RR_OP 35, mulhsu, 0x00000000, 0x00000001, 0xffffffff

    call stop_by_fault
