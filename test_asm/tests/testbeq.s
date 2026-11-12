.include "common.s"

.text
    .global __start
__start:
    TEST_BR2_OP_TAKEN 2, beq, 0, 0
    TEST_BR2_OP_TAKEN 3, beq, 1, 1
    TEST_BR2_OP_TAKEN 4, beq, -1, -1

    TEST_BR2_OP_NOTTAKEN 5, beq, 0, 1
    TEST_BR2_OP_NOTTAKEN 6, beq, 1, 0
    TEST_BR2_OP_NOTTAKEN 7, beq, -1, 1
    TEST_BR2_OP_NOTTAKEN 8, beq, 1, -1

    li t0, 1
    beq x0, x0, 2f
    addi t0, t0, 1
    addi t0, t0, 1
2:  addi t0, t0, 1
    TEST_CASE 9, t0, 2

    call stop_by_fault
