.include "common.s"

.text
    .global __start
__start:
    TEST_ST_OP 2, lw, sw, 0x00aa00aa, 0x00aa00aa, 0, tdat
    TEST_ST_OP 3, lw, sw, 0xaa00aa00, 0xaa00aa00, 4, tdat
    TEST_ST_OP 4, lw, sw, 0x0aa00aa0, 0x0aa00aa0, 8, tdat
    TEST_ST_OP 5, lw, sw, 0xa00aa00a, 0xa00aa00a, 12, tdat

    TEST_ST_OP 6, lw, sw, 0x00aa00aa, 0x00aa00aa, -12, tdat8
    TEST_ST_OP 7, lw, sw, 0xaa00aa00, 0xaa00aa00, -8, tdat8
    TEST_ST_OP 8, lw, sw, 0x0aa00aa0, 0x0aa00aa0, -4, tdat8
    TEST_ST_OP 9, lw, sw, 0xa00aa00a, 0xa00aa00a, 0, tdat8

    la t2, tdat9
    li t1, 0x12345678
    addi t3, t2, -32
    sw t1, 32(t3)
    lw t0, 0(t2)
    TEST_CASE 10, t0, 0x12345678

    la t2, tdat9
    li t1, 0x58213098
    addi t2, t2, -3
    sw t1, 7(t2)
    la t3, tdat10
    lw t0, 0(t3)
    TEST_CASE 11, t0, 0x58213098

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .word 0xdeadbeef
tdat2: .word 0xdeadbeef
tdat3: .word 0xdeadbeef
tdat4: .word 0xdeadbeef
tdat5: .word 0xdeadbeef
tdat6: .word 0xdeadbeef
tdat7: .word 0xdeadbeef
tdat8: .word 0xdeadbeef
tdat9: .word 0xdeadbeef
tdat10: .word 0xdeadbeef
