.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2, andi, 0xff00ff00, 0xff00ff00, 0xf0f
    TEST_IMM_OP 3, andi, 0x000000f0, 0x0ff00ff0, 0x0f0
    TEST_IMM_OP 4, andi, 0x0000000f, 0x00ff00ff, 0x70f
    TEST_IMM_OP 5, andi, 0x00000000, 0xf00ff00f, 0x0f0

    TEST_IMM_OP 6, andi, 0x00000700, 0x0ff00ff0, 0x70f
    TEST_IMM_OP 7, andi, 0x000000f0, 0x00ff00ff, 0x0f0
    TEST_IMM_OP 8, andi, 0xf00ff00f, 0xf00ff00f, 0xf0f

    TEST_IMM_OP 9, andi, 0x00000000, 0x00000000, 0x0f0

    call stop_by_fault
