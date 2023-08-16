.text

sent_to_uart:
    addi sp, sp, -4; # sp = sp + -4
    sw ra, 4(sp)

    li ra, 0x10000000 # ra = 0x10000000
    sb a0, 0(ra) # Send to UART 

    lw ra, 4(sp) #     
    addi sp, sp, 4; # sp = sp + 4
    ret

stop_by_fault:
    li a0, 0x0
    lw x0, 0(a0)

.macro test_insert_nops amount
.if \amount
    nop
test_insert_nops \amount-1
.endif
.endm

.macro fail test_num
    li a0, 'n'
    call sent_to_uart
    li a0, \test_num
    call sent_to_uart
.endm

.macro pass test_num
    li a0, 'y'
    call sent_to_uart
    li a0, \test_num
    call sent_to_uart
.endm


.macro test_rr_op test_num, instr, result, first, second
test_\test_num:
    li a1, \first
    li a2, \second
    li a3, \result
    \instr t0, a1, a2
    beq t0, a3, 1f; # if t0 == a3 then 1f
    fail \test_num
    j 2f  # jump to 2f
1:
    pass \test_num
2:
    nop
.endm

.macro sext_imm x:req sym:req
.set \sym, ((\x) | (-(((\x)>> 11) & 1) << 11))
.endm



.macro test_imm_op test_num, instr, result, first, imm
test_\test_num:
    li a1, \first
    li a3, \result
    sext_imm \imm x
    \instr t0, a1, x #\imm
    
    beq t0, a3, 1f; # if t0 == a3 then 1f
    fail \test_num
    j 2f  # jump to 2f
1:
    pass \test_num
2:
    nop
.endm

.macro test_br2_op_taken test_num, instr, val1, val2
test_\test_num:
    li a1, \val1
    li a2, \val2
    \instr a1, a2, 2f
    fail \test_num
    j 3f
1:
    pass \test_num
    j 3f
2:
    \instr a1, a2, 1b
    fail \test_num 
3:  nop
.endm

.macro test_br2_op_nottaken test_num, instr, val1, val2
test_\test_num:
    li a1, \val1
    li a2, \val2
    \instr a1, a2, 1f
    j 2f
1:    
    fail \test_num
    j 3f
2:
    \instr a1, a2, 1b
    pass \test_num
3:  nop
.endm
