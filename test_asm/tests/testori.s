.include "common.s"

.text
    .global __start
__start:
    TEST_IMM_OP 2, ori, 0xffffff0f, 0xff00ff00, 0xf0f
    TEST_IMM_OP 3, ori, 0x0ff00ff0, 0x0ff00ff0, 0x0f0
    TEST_IMM_OP 4, ori, 0x00ff07ff, 0x00ff00ff, 0x70f
    TEST_IMM_OP 5, ori, 0xf00ff0ff, 0xf00ff00f, 0x0f0

    TEST_IMM_OP 6, ori, 0x0ff00ff0, 0x0ff00ff0, 0x0f0
    TEST_IMM_OP 7, ori, 0x00ff07ff, 0x00ff00ff, 0x70f
    TEST_IMM_OP 8, ori, 0xffffffff, 0x00ff00ff, 0xf0f

    TEST_IMM_OP 9, ori, 0x000000f0, 0x00000000, 0x0f0

    call stop_by_fault
