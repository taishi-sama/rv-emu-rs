.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2, slti, 0, 0x00000000, 0x000
    TEST_IMM_OP 3, slti, 0, 0x00000001, 0x001
    TEST_IMM_OP 4, slti, 1, 0x00000003, 0x007
    TEST_IMM_OP 5, slti, 0, 0x00000007, 0x003

    TEST_IMM_OP 6, slti, 0, 0x00000000, 0x800
    TEST_IMM_OP 7, slti, 1, 0x80000000, 0x000
    TEST_IMM_OP 8, slti, 1, 0x80000000, 0x800

    TEST_IMM_OP 9, slti, 1, 0x00000000, 0x7ff
    TEST_IMM_OP 10, slti, 0, 0x7fffffff, 0x000
    TEST_IMM_OP 11, slti, 0, 0x7fffffff, 0x7ff

    TEST_IMM_OP 12, slti, 1, 0x80000000, 0x7ff
    TEST_IMM_OP 13, slti, 0, 0x7fffffff, 0x800

    TEST_IMM_OP 14, slti, 0, 0x00000000, 0xfff
    TEST_IMM_OP 15, slti, 1, 0xffffffff, 0x001
    TEST_IMM_OP 16, slti, 0, 0xffffffff, 0xfff

    call stop_by_fault
