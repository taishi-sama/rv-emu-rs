.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, rem, 0x00000002, 20, 6
    TEST_RR_OP 3, rem, 0xfffffffe, -20, 6
    TEST_RR_OP 4, rem, 0x00000002, 20, -6
    TEST_RR_OP 5, rem, 0xfffffffe, -20, -6

    TEST_RR_OP 6, rem, 0x00000000, 0x80000000, 1
    TEST_RR_OP 7, rem, 0x00000000, 0x80000000, -1

    TEST_RR_OP 8, rem, 0x80000000, 0x80000000, 0
    TEST_RR_OP 9, rem, 0x00000001, 1, 0
    TEST_RR_OP 10, rem, 0x00000000, 0, 0

    call stop_by_fault
