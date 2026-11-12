.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, mul, 0x00000000, 0x00000000, 0x00000000
    TEST_RR_OP 3, mul, 0x00000001, 0x00000001, 0x00000001
    TEST_RR_OP 4, mul, 0x00000015, 0x00000003, 0x00000007

    TEST_RR_OP 5, mul, 0x00000000, 0x00000000, 0xffff8000
    TEST_RR_OP 6, mul, 0x00000000, 0x80000000, 0x00000000
    TEST_RR_OP 7, mul, 0x00000000, 0x80000000, 0xffff8000

    TEST_RR_OP 30, mul, 0x0000ff7f, 0xaaaaaaab, 0x0002fe7d
    TEST_RR_OP 31, mul, 0x0000ff7f, 0x0002fe7d, 0xaaaaaaab
    TEST_RR_OP 32, mul, 0x00001200, 0x00007e00, 0xb6db6db7
    TEST_RR_OP 33, mul, 0x00001240, 0x00007fc0, 0xb6db6db7

    TEST_RR_OP 34, mul, 0x00000000, 0xff000000, 0xff000000
    TEST_RR_OP 35, mul, 0x00000001, 0xffffffff, 0xffffffff
    TEST_RR_OP 36, mul, 0xffffffff, 0xffffffff, 0x00000001
    TEST_RR_OP 37, mul, 0xffffffff, 0x00000001, 0xffffffff

    call stop_by_fault
