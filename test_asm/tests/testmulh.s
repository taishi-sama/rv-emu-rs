.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, mulh, 0x00000000, 0x00000000, 0x00000000
    TEST_RR_OP 3, mulh, 0x00000000, 0x00000001, 0x00000001
    TEST_RR_OP 4, mulh, 0x00000000, 0x00000003, 0x00000007

    TEST_RR_OP 5, mulh, 0x00000000, 0x00000000, 0xffff8000
    TEST_RR_OP 6, mulh, 0x00000000, 0x80000000, 0x00000000
    TEST_RR_OP 7, mulh, 0x00004000, 0x80000000, 0xffff8000

    TEST_RR_OP 30, mulh, 0xffff0081, 0xaaaaaaab, 0x0002fe7d
    TEST_RR_OP 31, mulh, 0xffff0081, 0x0002fe7d, 0xaaaaaaab

    TEST_RR_OP 32, mulh, 0x00010000, 0xff000000, 0xff000000

    TEST_RR_OP 33, mulh, 0x00000000, 0xffffffff, 0xffffffff
    TEST_RR_OP 34, mulh, 0xffffffff, 0xffffffff, 0x00000001
    TEST_RR_OP 35, mulh, 0xffffffff, 0x00000001, 0xffffffff

    call stop_by_fault
