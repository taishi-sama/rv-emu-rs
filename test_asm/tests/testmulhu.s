.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, mulhu, 0x00000000, 0x00000000, 0x00000000
    TEST_RR_OP 3, mulhu, 0x00000000, 0x00000001, 0x00000001
    TEST_RR_OP 4, mulhu, 0x00000000, 0x00000003, 0x00000007

    TEST_RR_OP 5, mulhu, 0x00000000, 0x00000000, 0xffff8000
    TEST_RR_OP 6, mulhu, 0x00000000, 0x80000000, 0x00000000
    TEST_RR_OP 7, mulhu, 0x7fffc000, 0x80000000, 0xffff8000

    TEST_RR_OP 30, mulhu, 0x0001fefe, 0xaaaaaaab, 0x0002fe7d
    TEST_RR_OP 31, mulhu, 0x0001fefe, 0x0002fe7d, 0xaaaaaaab

    TEST_RR_OP 32, mulhu, 0xfe010000, 0xff000000, 0xff000000

    TEST_RR_OP 33, mulhu, 0xfffffffe, 0xffffffff, 0xffffffff
    TEST_RR_OP 34, mulhu, 0x00000000, 0xffffffff, 0x00000001
    TEST_RR_OP 35, mulhu, 0x00000000, 0x00000001, 0xffffffff

    call stop_by_fault
