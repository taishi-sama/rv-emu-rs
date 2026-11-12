.include "common.s"

.text
    .global __start
__start:
    TEST_LD_OP 2, lbu, 0x000000ff, 0, tdat
    TEST_LD_OP 3, lbu, 0x00000000, 1, tdat
    TEST_LD_OP 4, lbu, 0x000000f0, 2, tdat
    TEST_LD_OP 5, lbu, 0x0000000f, 3, tdat

    TEST_LD_OP 6, lbu, 0x000000ff, -3, tdat4
    TEST_LD_OP 7, lbu, 0x00000000, -2, tdat4
    TEST_LD_OP 8, lbu, 0x000000f0, -1, tdat4
    TEST_LD_OP 9, lbu, 0x0000000f, 0, tdat4

    la t2, tdat
    addi t2, t2, -32
    lbu t0, 32(t2)
    TEST_CASE 10, t0, 0x000000ff

    la t2, tdat
    addi t2, t2, -6
    lbu t0, 7(t2)
    TEST_CASE 11, t0, 0x00000000

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .byte 0xff
tdat2: .byte 0x00
tdat3: .byte 0xf0
tdat4: .byte 0x0f
