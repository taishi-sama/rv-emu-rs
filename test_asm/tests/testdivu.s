.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, divu, 0x00000003, 20, 6
    TEST_RR_OP 3, divu, 0x2aaaaaa7, -20, 6
    TEST_RR_OP 4, divu, 0x00000000, 20, -6
    TEST_RR_OP 5, divu, 0x00000000, -20, -6

    TEST_RR_OP 6, divu, 0x80000000, 0x80000000, 1
    TEST_RR_OP 7, divu, 0x00000000, 0x80000000, -1

    TEST_RR_OP 8, divu, 0xffffffff, 0x80000000, 0
    TEST_RR_OP 9, divu, 0xffffffff, 1, 0
    TEST_RR_OP 10, divu, 0xffffffff, 0, 0

    call stop_by_fault
