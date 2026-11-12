.include "common.s"

.text
    .global __start
__start:
    la t0, scratch
    li t1, 0x12345678
    sw t1, 0(t0)
    fence
    lw t2, 0(t0)
    TEST_CASE 2, t2, 0x12345678

    li t1, 0xdeadbeef
    sw t1, 4(t0)
    fence.i
    lw t2, 4(t0)
    TEST_CASE 3, t2, 0xdeadbeef

    fence iorw, iorw
    fence.i
    TEST_CASE 4, zero, 0

    call stop_by_fault

.data
.align 2
scratch:
    .word 0x00000000
    .word 0x00000000
