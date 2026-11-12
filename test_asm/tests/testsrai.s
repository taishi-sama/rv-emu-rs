.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2, srai, 0x80000000, 0x80000000, 0
    TEST_IMM_OP 3, srai, 0xc0000000, 0x80000000, 1
    TEST_IMM_OP 4, srai, 0xff000000, 0x80000000, 7
    TEST_IMM_OP 5, srai, 0xfffe0000, 0x80000000, 14
    TEST_IMM_OP 6, srai, 0xffffffff, 0x80000001, 31

    TEST_IMM_OP 7, srai, 0x7fffffff, 0x7fffffff, 0
    TEST_IMM_OP 8, srai, 0x3fffffff, 0x7fffffff, 1
    TEST_IMM_OP 9, srai, 0x00ffffff, 0x7fffffff, 7
    TEST_IMM_OP 10, srai, 0x0001ffff, 0x7fffffff, 14
    TEST_IMM_OP 11, srai, 0x00000000, 0x7fffffff, 31

    TEST_IMM_OP 12, srai, 0x81818181, 0x81818181, 0
    TEST_IMM_OP 13, srai, 0xc0c0c0c0, 0x81818181, 1
    TEST_IMM_OP 14, srai, 0xff030303, 0x81818181, 7
    TEST_IMM_OP 15, srai, 0xfffe0606, 0x81818181, 14
    TEST_IMM_OP 16, srai, 0xffffffff, 0x81818181, 31

    call stop_by_fault
