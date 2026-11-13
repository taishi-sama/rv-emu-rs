.include "common.s"

.text
    .global __start
__start:
    la a1, data
    li a0, 0xfeedface
    c.sw a0, 8(a1)
    lw t0, 8(a1)
    TEST_CASE 2, t0, 0xfeedface

    li a0, 0x0badc0de
    c.sw a0, 124(a1)
    lw t0, 124(a1)
    TEST_CASE 3, t0, 0x0badc0de

    li a0, 0x1234abcd
    c.sw a0, 0(a1)
    lw t0, 0(a1)
    TEST_CASE 4, t0, 0x1234abcd

    call stop_by_fault

.data
.align 2
data:
    .zero 128
