.include "common.s"

.text
    .global __start
__start:
    la a1, data
    li t0, 0x11223344
    sw t0, 4(a1)
    li t0, 0x55667788
    sw t0, 124(a1)

    c.lw a0, 4(a1)
    TEST_CASE 2, a0, 0x11223344
    c.lw a0, 124(a1)
    TEST_CASE 3, a0, 0x55667788
    c.lw a0, 0(a1)
    TEST_CASE 4, a0, 0

    call stop_by_fault

.data
.align 2
data:
    .zero 128
