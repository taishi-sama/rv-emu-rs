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

.macro test_rr_op test_num, instr, result, first, second
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

.macro sext_imm x 
((x) | (-(((x)>> 11) & 1) << 11))
.endm

# SEXT_IMM(x) ((x) | (-(((x) >> 11) & 1) << 11))

.macro test_imm_op test_num, instr, result, first, imm
    li a1, \first
    li a3, \result
    \instr t0, a1, ((\imm) | (-(((\imm) >> 11) & 1) << 11));
    beq t0, a3, 1f; # if t0 == a3 then 1f
    fail \test_num
    j 2f  # jump to 2f
1:
    pass \test_num
2:
    nop
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
