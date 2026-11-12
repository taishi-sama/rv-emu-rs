.include "common.s"

.text
    .global __start
__start:
    TEST_BR2_OP_TAKEN 2, blt, 0, 1
    TEST_BR2_OP_TAKEN 3, blt, -1, 1
    TEST_BR2_OP_TAKEN 4, blt, -2, -1

    TEST_BR2_OP_NOTTAKEN 5, blt, 1, 0
    TEST_BR2_OP_NOTTAKEN 6, blt, 1, -1
    TEST_BR2_OP_NOTTAKEN 7, blt, -1, -2
    TEST_BR2_OP_NOTTAKEN 8, blt, 1, -2

    li t0, 1
    blt x0, t0, 2f
    addi t0, t0, 1
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 9, t0, 2

    call stop_by_fault
