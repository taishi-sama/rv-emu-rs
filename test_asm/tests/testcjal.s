.include "common.s"

.text
    .global __start
__start:
    la t1, 4f
    li ra, 0
    c.jal 2f
4:  li ra, 0xbad
2:  sub ra, ra, t1
    TEST_CASE 2, ra, 0

    call stop_by_fault
