.include "common.s"

.text
    .global __start
__start:
    TEST_BR2_OP_TAKEN 2, bne, 0, 1
    TEST_BR2_OP_TAKEN 3, bne, 1, 0
    TEST_BR2_OP_TAKEN 4, bne, -1, 1
    TEST_BR2_OP_TAKEN 5, bne, 1, -1

    TEST_BR2_OP_NOTTAKEN 6, bne, 0, 0
    TEST_BR2_OP_NOTTAKEN 7, bne, 1, 1
    TEST_BR2_OP_NOTTAKEN 8, bne, -1, -1

    li t0, 1
    bne t0, x0, 2f
    addi t0, t0, 1
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 9, t0, 2

    call stop_by_fault
