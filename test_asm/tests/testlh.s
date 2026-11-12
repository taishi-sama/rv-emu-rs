.include "common.s"

.text
    .global __start
__start:
    TEST_LD_OP 2, lh, 0x000000ff, 0, tdat
    TEST_LD_OP 3, lh, 0xffffff00, 2, tdat
    TEST_LD_OP 4, lh, 0x00000ff0, 4, tdat
    TEST_LD_OP 5, lh, 0xfffff00f, 6, tdat

    TEST_LD_OP 6, lh, 0x000000ff, -6, tdat4
    TEST_LD_OP 7, lh, 0xffffff00, -4, tdat4
    TEST_LD_OP 8, lh, 0x00000ff0, -2, tdat4
    TEST_LD_OP 9, lh, 0xfffff00f, 0, tdat4

    la t2, tdat
    addi t2, t2, -32
    lh t0, 32(t2)
    TEST_CASE 10, t0, 0x000000ff

    la t2, tdat
    addi t2, t2, -5
    lh t0, 7(t2)
    TEST_CASE 11, t0, 0xffffff00

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .half 0x00ff
tdat2: .half 0xff00
tdat3: .half 0x0ff0
tdat4: .half 0xf00f
