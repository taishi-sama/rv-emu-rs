.include "common.s"

.text
    .global __start
__start:
    TEST_BR2_OP_TAKEN 2, bgeu, 0x00000000, 0x00000000
    TEST_BR2_OP_TAKEN 3, bgeu, 0x00000001, 0x00000001
    TEST_BR2_OP_TAKEN 4, bgeu, 0xffffffff, 0xffffffff
    TEST_BR2_OP_TAKEN 5, bgeu, 0x00000001, 0x00000000
    TEST_BR2_OP_TAKEN 6, bgeu, 0xffffffff, 0xfffffffe
    TEST_BR2_OP_TAKEN 7, bgeu, 0xffffffff, 0x00000000

    TEST_BR2_OP_NOTTAKEN 8, bgeu, 0x00000000, 0x00000001
    TEST_BR2_OP_NOTTAKEN 9, bgeu, 0xfffffffe, 0xffffffff
    TEST_BR2_OP_NOTTAKEN 10, bgeu, 0x00000000, 0xffffffff
    TEST_BR2_OP_NOTTAKEN 11, bgeu, 0x7fffffff, 0x80000000

    li t0, 1
    bgeu t0, x0, 2f
    addi t0, t0, 1
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 12, t0, 2

    call stop_by_fault
