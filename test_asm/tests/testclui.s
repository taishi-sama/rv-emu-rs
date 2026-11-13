.include "common.s"

.text
    .global __start
__start:
    c.lui a0, 0x1f
    TEST_CASE 2, a0, 0x1f000

    c.lui a0, 0xfffe1
    TEST_CASE 3, a0, 0xfffe1000

    c.lui zero, 1
    TEST_CASE 4, zero, 0

    call stop_by_fault
