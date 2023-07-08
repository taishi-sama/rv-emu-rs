.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2,  addi, 0x00000000, 0x00000000, 0x000;
    TEST_IMM_OP 3,  addi, 0x00000002, 0x00000001, 0x001;
    TEST_IMM_OP 4,  addi, 0x0000000a, 0x00000003, 0x007;
    TEST_IMM_OP 5,  addi, 0xfffffffffffff800, 0x0000000000000000, 0x800;
    TEST_IMM_OP 6,  addi, 0xffffffff80000000, 0xffffffff80000000, 0x000;
    TEST_IMM_OP 7,  addi, 0xffffffff7ffff800, 0xffffffff80000000, 0x800;
    TEST_IMM_OP 8,  addi, 0x00000000000007ff, 0x00000000, 0x7ff;
    TEST_IMM_OP 9,  addi, 0x000000007fffffff, 0x7fffffff, 0x000;
    TEST_IMM_OP 10, addi, 0x00000000800007fe, 0x7fffffff, 0x7ff;
    TEST_IMM_OP 11, addi, 0xffffffff800007ff, 0xffffffff80000000, 0x7ff;
    TEST_IMM_OP 12, addi, 0x000000007ffff7ff, 0x000000007fffffff, 0x800;
    TEST_IMM_OP 13, addi, 0xffffffffffffffff, 0x0000000000000000, 0xfff;
    TEST_IMM_OP 14, addi, 0x0000000000000000, 0xffffffffffffffff, 0x001;
    TEST_IMM_OP 15, addi, 0xfffffffffffffffe, 0xffffffffffffffff, 0xfff;
    TEST_IMM_OP 16, addi, 0x0000000080000000, 0x7fffffff, 0x001;
    call stop_by_fault
