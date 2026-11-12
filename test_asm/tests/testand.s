.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, and, 0x0f000f00, 0xff00ff00, 0x0f0f0f0f
    TEST_RR_OP 3, and, 0x00f000f0, 0x0ff00ff0, 0xf0f0f0f0
    TEST_RR_OP 4, and, 0x000f000f, 0x00ff00ff, 0x0f0f0f0f
    TEST_RR_OP 5, and, 0xf000f000, 0xf00ff00f, 0xf0f0f0f0

    TEST_RR_OP 6, and, 0x00000000, 0xffffffff, 0x00000000
    TEST_RR_OP 7, and, 0x80000000, 0xffffffff, 0x80000000
    TEST_RR_OP 8, and, 0xffffffff, 0xffffffff, 0xffffffff

    call stop_by_fault
