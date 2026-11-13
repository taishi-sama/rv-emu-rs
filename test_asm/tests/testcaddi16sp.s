.include "common.s"

.text
    .global __start
__start:
    mv s0, sp
    c.addi16sp sp, 496
    sub s1, sp, s0
    c.addi16sp sp, -512
    sub s2, sp, s0
    mv sp, s0
    TEST_CASE 2, s1, 496
    TEST_CASE 3, s2, -16

    call stop_by_fault
