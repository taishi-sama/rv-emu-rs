.include "common.s"

.text
    .global __start
__start:
    li a0, 0x5a5
    c.nop
    TEST_CASE 2, a0, 0x5a5

    li a0, 0x5a5
    c.nop
    c.nop
    TEST_CASE 3, a0, 0x5a5

    call stop_by_fault
