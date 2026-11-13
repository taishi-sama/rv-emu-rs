.include "common.s"

.text
    .global __start
__start:
    li s0, 0x80000000
    c.srli s0, 31
    TEST_CASE 2, s0, 1

    li s0, 0xffffffff
    c.srli s0, 7
    TEST_CASE 3, s0, 0x01ffffff

    call stop_by_fault
