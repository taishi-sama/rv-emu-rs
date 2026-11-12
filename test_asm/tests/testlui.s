.include "common.s"

.text
    .global __start
__start:
    lui t0, 0x00000
    TEST_CASE 2, t0, 0x00000000

    lui t0, 0xfffff
    sra t0, t0, 1
    TEST_CASE 3, t0, 0xfffff800

    lui t0, 0x7ffff
    sra t0, t0, 20
    TEST_CASE 4, t0, 0x000007ff

    lui t0, 0x80000
    sra t0, t0, 20
    TEST_CASE 5, t0, 0xfffff800

    lui x0, 0x80000
    TEST_CASE 6, x0, 0

    lui t0, 0x12345
    TEST_CASE 7, t0, 0x12345000

    call stop_by_fault
