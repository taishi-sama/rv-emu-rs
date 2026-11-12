.include "common.s"

.text
    .global __start
__start:
    TEST_RR_OP 2, srl, 0x80000000, 0x80000000, 0
    TEST_RR_OP 3, srl, 0x40000000, 0x80000000, 1
    TEST_RR_OP 4, srl, 0x01000000, 0x80000000, 7
    TEST_RR_OP 5, srl, 0x00020000, 0x80000000, 14
    TEST_RR_OP 6, srl, 0x00000001, 0x80000001, 31

    TEST_RR_OP 7, srl, 0xffffffff, 0xffffffff, 0
    TEST_RR_OP 8, srl, 0x7fffffff, 0xffffffff, 1
    TEST_RR_OP 9, srl, 0x01ffffff, 0xffffffff, 7
    TEST_RR_OP 10, srl, 0x0003ffff, 0xffffffff, 14
    TEST_RR_OP 11, srl, 0x00000001, 0xffffffff, 31

    TEST_RR_OP 12, srl, 0x21212121, 0x21212121, 0
    TEST_RR_OP 13, srl, 0x10909090, 0x21212121, 1
    TEST_RR_OP 14, srl, 0x00424242, 0x21212121, 7
    TEST_RR_OP 15, srl, 0x00008484, 0x21212121, 14
    TEST_RR_OP 16, srl, 0x00000000, 0x21212121, 31

    TEST_RR_OP 17, srl, 0x21212121, 0x21212121, 0xffffffc0
    TEST_RR_OP 18, srl, 0x10909090, 0x21212121, 0xffffffc1
    TEST_RR_OP 19, srl, 0x00424242, 0x21212121, 0xffffffc7
    TEST_RR_OP 20, srl, 0x00008484, 0x21212121, 0xffffffce
    TEST_RR_OP 21, srl, 0x00000000, 0x21212121, 0xffffffff

    call stop_by_fault
