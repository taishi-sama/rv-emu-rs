.include "common.s"

.text
    .global __start
__start:
    TEST_ST_OP 2, lb, sb, 0xffffffaa, 0xffffffaa, 0, tdat
    TEST_ST_OP 3, lb, sb, 0x00000000, 0x00000000, 1, tdat
    TEST_ST_OP 4, lb, sb, 0xffffffa0, 0xffffffa0, 2, tdat
    TEST_ST_OP 5, lb, sb, 0x0000000a, 0x0000000a, 3, tdat

    TEST_ST_OP 6, lb, sb, 0xffffffaa, 0xffffffaa, -3, tdat8
    TEST_ST_OP 7, lb, sb, 0x00000000, 0x00000000, -2, tdat8
    TEST_ST_OP 8, lb, sb, 0xffffffa0, 0xffffffa0, -1, tdat8
    TEST_ST_OP 9, lb, sb, 0x0000000a, 0x0000000a, 0, tdat8

    la t2, tdat9
    li t1, 0x12345678
    addi t3, t2, -32
    sb t1, 32(t3)
    lb t0, 0(t2)
    TEST_CASE 10, t0, 0x00000078

    la t2, tdat9
    li t1, 0x00003098
    addi t2, t2, -6
    sb t1, 7(t2)
    la t3, tdat10
    lb t0, 0(t3)
    TEST_CASE 11, t0, 0xffffff98

    call stop_by_fault

.data
.align 2
tdat:
tdat1: .byte 0xef
tdat2: .byte 0xef
tdat3: .byte 0xef
tdat4: .byte 0xef
tdat5: .byte 0xef
tdat6: .byte 0xef
tdat7: .byte 0xef
tdat8: .byte 0xef
tdat9: .byte 0xef
tdat10: .byte 0xef
