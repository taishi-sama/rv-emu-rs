.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, remu, 0x00000002, 20, 6
    TEST_RR_OP 3, remu, 0x00000002, -20, 6
    TEST_RR_OP 4, remu, 0x00000014, 20, -6
    TEST_RR_OP 5, remu, 0xffffffec, -20, -6

    TEST_RR_OP 6, remu, 0x00000000, 0x80000000, 1
    TEST_RR_OP 7, remu, 0x80000000, 0x80000000, -1

    TEST_RR_OP 8, remu, 0x80000000, 0x80000000, 0
    TEST_RR_OP 9, remu, 0x00000001, 1, 0
    TEST_RR_OP 10, remu, 0x00000000, 0, 0

    call stop_by_fault
