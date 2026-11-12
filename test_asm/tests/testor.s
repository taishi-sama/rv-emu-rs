.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, or, 0xff0fff0f, 0xff00ff00, 0x0f0f0f0f
    TEST_RR_OP 3, or, 0xfff0fff0, 0x0ff00ff0, 0xf0f0f0f0
    TEST_RR_OP 4, or, 0x0fff0fff, 0x00ff00ff, 0x0f0f0f0f
    TEST_RR_OP 5, or, 0xf0fff0ff, 0xf00ff00f, 0xf0f0f0f0

    TEST_RR_OP 6, or, 0xffffffff, 0xffffffff, 0x00000000
    TEST_RR_OP 7, or, 0x80000000, 0x80000000, 0x00000000
    TEST_RR_OP 8, or, 0xffffffff, 0x80000000, 0x7fffffff

    call stop_by_fault
