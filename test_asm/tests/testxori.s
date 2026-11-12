.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2, xori, 0xff00f00f, 0x00ff0f00, 0xf0f
    TEST_IMM_OP 3, xori, 0x0ff00f00, 0x0ff00ff0, 0x0f0
    TEST_IMM_OP 4, xori, 0x00ff0ff0, 0x00ff08ff, 0x70f
    TEST_IMM_OP 5, xori, 0xf00ff0ff, 0xf00ff00f, 0x0f0

    TEST_IMM_OP 6, xori, 0xff00f00f, 0xff00f700, 0x70f
    TEST_IMM_OP 7, xori, 0x0ff00f00, 0x0ff00ff0, 0x0f0
    TEST_IMM_OP 8, xori, 0x00ff0ff0, 0x00ff0fff, 0x00f
    TEST_IMM_OP 9, xori, 0xf00ff0ff, 0xf00ff00f, 0x0f0

    TEST_IMM_OP 10, xori, 0x000000f0, 0x00000000, 0x0f0

    call stop_by_fault
