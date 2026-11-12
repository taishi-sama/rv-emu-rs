.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, sub, 0x00000000, 0x00000000, 0x00000000
    TEST_RR_OP 3, sub, 0x00000000, 0x00000001, 0x00000001
    TEST_RR_OP 4, sub, 0xfffffffc, 0x00000003, 0x00000007

    TEST_RR_OP 5, sub, 0x00008000, 0x00000000, 0xffff8000
    TEST_RR_OP 6, sub, 0x80000000, 0x80000000, 0x00000000
    TEST_RR_OP 7, sub, 0x80008000, 0x80000000, 0xffff8000

    TEST_RR_OP 8, sub, 0xffff8001, 0x00000000, 0x00007fff
    TEST_RR_OP 9, sub, 0x7fffffff, 0x7fffffff, 0x00000000
    TEST_RR_OP 10, sub, 0x7fff8000, 0x7fffffff, 0x00007fff

    TEST_RR_OP 11, sub, 0x7fff8001, 0x80000000, 0x00007fff
    TEST_RR_OP 12, sub, 0x80007fff, 0x7fffffff, 0xffff8000

    TEST_RR_OP 13, sub, 0x00000001, 0x00000000, 0xffffffff
    TEST_RR_OP 14, sub, 0xfffffffe, 0xffffffff, 0x00000001
    TEST_RR_OP 15, sub, 0x00000000, 0xffffffff, 0xffffffff

    call stop_by_fault
