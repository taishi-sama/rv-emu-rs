.include "common.s"

.text
    .global __start
__start:
    li s0, 0x80000000
    c.srai s0, 31
    TEST_CASE 2, s0, 0xffffffff

    li s0, 0x81818181
    c.srai s0, 14
    TEST_CASE 3, s0, 0xfffe0606

    call stop_by_fault
