.include "common.s"

.text
    .global __start
__start:
    TEST_LD_OP 2, lhu, 0x000000ff, 0, tdat
    TEST_LD_OP 3, lhu, 0x0000ff00, 2, tdat
    TEST_LD_OP 4, lhu, 0x00000ff0, 4, tdat
    TEST_LD_OP 5, lhu, 0x0000f00f, 6, tdat

    TEST_LD_OP 6, lhu, 0x000000ff, -6, tdat4
    TEST_LD_OP 7, lhu, 0x0000ff00, -4, tdat4
    TEST_LD_OP 8, lhu, 0x00000ff0, -2, tdat4
    TEST_LD_OP 9, lhu, 0x0000f00f, 0, tdat4

    la t2, tdat
    addi t2, t2, -32
    lhu t0, 32(t2)
    TEST_CASE 10, t0, 0x000000ff

    la t2, tdat
    addi t2, t2, -5
    lhu t0, 7(t2)
    TEST_CASE 11, t0, 0x0000ff00

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .half 0x00ff
tdat2: .half 0xff00
tdat3: .half 0x0ff0
tdat4: .half 0xf00f
