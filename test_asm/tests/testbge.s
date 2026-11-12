.include "common.s"

.text
    .global __start
__start:
    TEST_BR2_OP_TAKEN 2, bge, 0, 0
    TEST_BR2_OP_TAKEN 3, bge, 1, 1
    TEST_BR2_OP_TAKEN 4, bge, -1, -1
    TEST_BR2_OP_TAKEN 5, bge, 1, 0
    TEST_BR2_OP_TAKEN 6, bge, 1, -1
    TEST_BR2_OP_TAKEN 7, bge, -1, -2

    TEST_BR2_OP_NOTTAKEN 8, bge, 0, 1
    TEST_BR2_OP_NOTTAKEN 9, bge, -1, 1
    TEST_BR2_OP_NOTTAKEN 10, bge, -2, -1
    TEST_BR2_OP_NOTTAKEN 11, bge, -2, 1

    li t0, 1
    bge t0, x0, 2f
    addi t0, t0, 1
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 12, t0, 2

    call stop_by_fault
