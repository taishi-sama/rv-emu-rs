.include "common.s"

.text
    .global __start
__start:
    TEST_LD_OP 2, lw, 0x00ff00ff, 0, tdat
    TEST_LD_OP 3, lw, 0xff00ff00, 4, tdat
    TEST_LD_OP 4, lw, 0x0ff00ff0, 8, tdat
    TEST_LD_OP 5, lw, 0xf00ff00f, 12, tdat

    TEST_LD_OP 6, lw, 0x00ff00ff, -12, tdat4
    TEST_LD_OP 7, lw, 0xff00ff00, -8, tdat4
    TEST_LD_OP 8, lw, 0x0ff00ff0, -4, tdat4
    TEST_LD_OP 9, lw, 0xf00ff00f, 0, tdat4

    la t2, tdat
    addi t2, t2, -32
    lw t0, 32(t2)
    TEST_CASE 10, t0, 0x00ff00ff

    la t2, tdat
    addi t2, t2, -3
    lw t0, 7(t2)
    TEST_CASE 11, t0, 0xff00ff00

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .word 0x00ff00ff
tdat2: .word 0xff00ff00
tdat3: .word 0x0ff00ff0
tdat4: .word 0xf00ff00f
