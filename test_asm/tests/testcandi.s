.include "common.s"

.text
    .global __start
__start:
    li s0, 0x0ff00ff0
    c.andi s0, 0x1f
    TEST_CASE 2, s0, 0x10

    li s0, 0xffffffff
    c.andi s0, -32
    TEST_CASE 3, s0, 0xffffffe0

    li s0, 0x0f
    c.andi s0, -1
    TEST_CASE 4, s0, 0x0f

    call stop_by_fault
