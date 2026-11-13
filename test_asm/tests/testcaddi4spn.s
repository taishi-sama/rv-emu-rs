.include "common.s"

.text
    .global __start
__start:
    mv s0, sp
    li sp, 0x1234
    c.addi4spn a0, sp, 1020
    c.addi4spn a1, sp, 4
    mv sp, s0
    TEST_CASE 2, a0, 0x1630
    TEST_CASE 3, a1, 0x1238

    call stop_by_fault
