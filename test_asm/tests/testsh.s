.include "common.s"

.text
    .global __start
__start:
    TEST_ST_OP 2, lh, sh, 0x000000aa, 0x000000aa, 0, tdat
    TEST_ST_OP 3, lh, sh, 0xffffaa00, 0xffffaa00, 2, tdat
    TEST_ST_OP 4, lh, sh, 0xbeef0aa0, 0x00000aa0, 4, tdat
    TEST_ST_OP 5, lh, sh, 0xffffa00a, 0xffffa00a, 6, tdat

    TEST_ST_OP 6, lh, sh, 0x000000aa, 0x000000aa, -6, tdat8
    TEST_ST_OP 7, lh, sh, 0xffffaa00, 0xffffaa00, -4, tdat8
    TEST_ST_OP 8, lh, sh, 0x00000aa0, 0x00000aa0, -2, tdat8
    TEST_ST_OP 9, lh, sh, 0xffffa00a, 0xffffa00a, 0, tdat8

    la t2, tdat9
    li t1, 0x12345678
    addi t3, t2, -32
    sh t1, 32(t3)
    lh t0, 0(t2)
    TEST_CASE 10, t0, 0x00005678

    la t2, tdat9
    li t1, 0x00003098
    addi t2, t2, -5
    sh t1, 7(t2)
    la t3, tdat10
    lh t0, 0(t3)
    TEST_CASE 11, t0, 0x00003098

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .half 0xbeef
tdat2: .half 0xbeef
tdat3: .half 0xbeef
tdat4: .half 0xbeef
tdat5: .half 0xbeef
tdat6: .half 0xbeef
tdat7: .half 0xbeef
tdat8: .half 0xbeef
tdat9: .half 0xbeef
tdat10: .half 0xbeef
