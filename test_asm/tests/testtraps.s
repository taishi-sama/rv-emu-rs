 .include "common.s"

.text
    .global __start
__start:
    la a0, table
    csrw mtvec, a0 
    ecall
    nop
    nop
    nop
    li a0, 0
    csrrw a0, mtvec, a0
    ecall
    nop
    nop

table: 
    li a0, 'I'
    call sent_to_uart
    li a0, '\n'
    call sent_to_uart
    li a0, 0 
    csrrw a0, mepc, a0
    addi a0, a0, 4
    csrrw a0, mepc, a0
    mret
