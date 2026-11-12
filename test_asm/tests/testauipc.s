.include "common.s"

.text
    .global __start
__start:
    auipc t0, 0
    auipc t1, 0
    sub t0, t1, t0
    TEST_CASE 2, t0, 4

    auipc t0, 1
    auipc t1, 0
    sub t0, t0, t1
    TEST_CASE 3, t0, 0xffc

    lla a0, 1f + 10000
    jal a1, 1f
1:  sub a0, a0, a1
    TEST_CASE 4, a0, 10000

    lla a0, 2f - 10000
    jal a1, 2f
2:  sub a0, a0, a1
    TEST_CASE 5, a0, -10000

    auipc x0, 0xfffff
    TEST_CASE 6, x0, 0

    call stop_by_fault
