.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, div, 0x00000003, 20, 6
    TEST_RR_OP 3, div, 0xfffffffd, -20, 6
    TEST_RR_OP 4, div, 0xfffffffd, 20, -6
    TEST_RR_OP 5, div, 0x00000003, -20, -6

    TEST_RR_OP 6, div, 0x80000000, 0x80000000, 1
    TEST_RR_OP 7, div, 0x80000000, 0x80000000, -1

    TEST_RR_OP 8, div, 0xffffffff, 0x80000000, 0
    TEST_RR_OP 9, div, 0xffffffff, 1, 0
    TEST_RR_OP 10, div, 0xffffffff, 0, 0

    call stop_by_fault
