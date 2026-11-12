.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, xor, 0xf00ff00f, 0xff00ff00, 0x0f0f0f0f
    TEST_RR_OP 3, xor, 0xff00ff00, 0x0ff00ff0, 0xf0f0f0f0
    TEST_RR_OP 4, xor, 0x0ff00ff0, 0x00ff00ff, 0x0f0f0f0f
    TEST_RR_OP 5, xor, 0x00ff00ff, 0xf00ff00f, 0xf0f0f0f0

    TEST_RR_OP 6, xor, 0x00000000, 0xffffffff, 0xffffffff
    TEST_RR_OP 7, xor, 0xffffffff, 0x00000000, 0xffffffff
    TEST_RR_OP 8, xor, 0x7fffffff, 0xffffffff, 0x80000000
    TEST_RR_OP 9, xor, 0x80000000, 0x80000000, 0x00000000

    call stop_by_fault
