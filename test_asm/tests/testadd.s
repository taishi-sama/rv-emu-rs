.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2,  add, 0x00000000, 0x00000000, 0x00000000;
    TEST_RR_OP 3,  add, 0x00000002, 0x00000001, 0x00000001;
    TEST_RR_OP 4,  add, 0x0000000a, 0x00000003, 0x00000007;

    TEST_RR_OP 5,  add, 0xffffffffffff8000, 0x0000000000000000, 0xffffffffffff8000;
    TEST_RR_OP 6,  add, 0xffffffff80000000, 0xffffffff80000000, 0x00000000;
    TEST_RR_OP 7,  add, 0xffffffff7fff8000, 0xffffffff80000000, 0xffffffffffff8000;
    TEST_RR_OP 8,  add, 0x0000000000007fff, 0x0000000000000000, 0x0000000000007fff;
    TEST_RR_OP 9,  add, 0x000000007fffffff, 0x000000007fffffff, 0x0000000000000000;
    TEST_RR_OP 10, add, 0x0000000080007ffe, 0x000000007fffffff, 0x0000000000007fff;
    TEST_RR_OP 11, add, 0xffffffff80007fff, 0xffffffff80000000, 0x0000000000007fff;
    TEST_RR_OP 12, add, 0x000000007fff7fff, 0x000000007fffffff, 0xffffffffffff8000;
    TEST_RR_OP 13, add, 0xffffffffffffffff, 0x0000000000000000, 0xffffffffffffffff;
    TEST_RR_OP 14, add, 0x0000000000000000, 0xffffffffffffffff, 0x0000000000000001;
    TEST_RR_OP 15, add, 0xfffffffffffffffe, 0xffffffffffffffff, 0xffffffffffffffff;
    TEST_RR_OP 16, add, 0x0000000080000000, 0x0000000000000001, 0x000000007fffffff;
    call stop_by_fault
