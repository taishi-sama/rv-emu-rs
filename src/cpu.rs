use std::sync::{atomic::AtomicBool, Arc};

use crate::{
    bus::{AtomicAccess, RAM_ADDRESS},
    errors::EmulatorError,
    mmu::MMU,
    ops_decode::{
        get_compressed_cb_and_imm, get_compressed_cb_branch_imm, get_compressed_cb_shift_imm,
        get_compressed_ci_addi16sp_imm, get_compressed_ci_li_addi_imm, get_compressed_ci_lui_imm,
        get_compressed_ci_stack_load_32_imm, get_compressed_ciw_addi4spn_imm,
        get_compressed_cj_jump_imm, get_compressed_cl_mem_load_32_imm,
        get_compressed_cs_mem_store_32_imm, get_compressed_css_stack_write_32_imm,
        get_compressed_func, get_compressed_func3, get_compressed_func6, get_compressed_rd,
        get_compressed_rdc, get_compressed_rs1c, get_compressed_rs2, get_csr_num, get_funct3,
        get_funct7, get_imm_b_type, get_imm_i_type, get_imm_j_type, get_imm_s_type, get_imm_u_type,
        get_opcode, get_rd, get_rs1, get_rs2, get_rs3,
    },
    ram::RAM_SIZE,
    traps::{Trap, TrapType},
};

#[derive(Clone, Copy)]
pub enum PrivilegeMode {
    //Discriminants match the mstatus.MPP encoding.
    Machine = 3,
    Supervisor = 1,
    Reserved = 2,
    User = 0,
}

const MSTATUS_MIE: u32 = 1 << 3;
const MSTATUS_MPIE: u32 = 1 << 7;
const MSTATUS_MPP_SHIFT: u32 = 11;
const MIE_MTIE: u32 = 1 << 7;

// RV32IMAC
// Everything that is not memory-related
pub struct CPUState {
    x: [u32; 32],
    pub pc: u32,

    //CSRs
    //pub CSRs: [u32; 4096],
    pub mstatus: u64,
    pub cycle: u64,

    pub mscratch: u32,
    pub mtvec: u32,
    pub mie: u32,
    pub mip: u32,

    pub mepc: u32,
    pub mtval: u32,
    pub mcause: u32,
    //PMP configuration and address registers. The values are stored but not enforced yet.
    pub pmpcfg: [u32; 4],
    pub pmpaddr: [u32; 16],
    pub privilege: PrivilegeMode,
    pub wfi: bool,
    //Address of the word reserved by LR.W, accepted by SC.W afterwards.
    pub reservation_slot: Option<u32>,
    pub stopflag: Option<Arc<AtomicBool>>,
}

#[allow(dead_code)]
pub struct CPU {
    pub state: CPUState,
    pub mmu: MMU,
}

#[allow(dead_code)]
#[allow(unused_variables)]
impl CPU {
    pub fn new(mmu: MMU) -> Self {
        let mut state = CPUState::new();
        //Start at the top of RAM.
        state.set_x(2, RAM_ADDRESS + RAM_SIZE as u32 - 0x4);
        CPU { state, mmu }
    }
    //pub fn run_debugger(&mut self) {}
    pub fn execute_instruction(&mut self) -> Result<u32, Trap> {
        let instr = self.fetch()?;
        let instr_len = self.execute(instr)? as u32;
        self.state.pc = self.state.pc.wrapping_add(instr_len);
        Ok(instr)
    }
    pub fn step(&mut self) -> Result<u32, EmulatorError> {
        //Waking from WFI and taking the interrupt happen together in handle_clint, which runs
        //before every step. A halted hart simply retires nothing.
        if self.state.wfi {
            return Ok(0);
        }
        let res = self.execute_instruction();
        match res {
            Ok(v) => return Ok(v),
            Err(t) => {
                self.state.process_trap(t)?;
                return Ok(0);
            }
        }
    }
    //Wake the hart from WFI and deliver a pending machine timer interrupt.
    pub fn handle_clint(&mut self) -> Result<(), EmulatorError> {
        let pending = Self::pending_mip(&self.state, &mut self.mmu);
        //WFI wakes on any locally enabled interrupt, even when mstatus.MIE is clear.
        if self.state.wfi && self.state.mie & pending != 0 {
            self.state.wfi = false;
        }
        if self.state.mie & pending & MIE_MTIE != 0 && self.state.mstatus as u32 & MSTATUS_MIE != 0
        {
            self.state.process_trap(Trap {
                tcause: TrapType::MachineTimerInterrupt,
                tval: 0,
            })?;
        }
        Ok(())
    }
    // Fetch the first 16 bits, then if the instruction is not compressed fetch the remaining 16 bits.
    fn fetch(&mut self) -> Result<u32, Trap> {
        let pc = self.state.pc;
        let lower = self.mmu.fetch_halfword(pc)?;
        if lower & 0b11 == 0b11 {
            let upper = self.mmu.fetch_halfword(pc.wrapping_add(2))?;
            Ok(lower as u32 | (upper as u32) << 16)
        } else {
            Ok(lower as u32)
        }
    }
    fn execute(&mut self, instr: u32) -> Result<u8, Trap> {
        let opcode = get_opcode(instr);
        let micro_opcode = opcode & 0b11;
        if micro_opcode == 0b11 {
            //Common instructions processing
            match opcode {
                0b0110111 => Self::lui(&mut self.state, instr),
                0b0010111 => Self::auipc(&mut self.state, instr),
                0b1101111 => Self::jal(&mut self.state, instr),
                0b1100111 => match get_funct3(instr) {
                    0 => Self::jalr(&mut self.state, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b1100011 => match get_funct3(instr) {
                    0b000 => Self::beq(&mut self.state, instr),
                    0b001 => Self::bne(&mut self.state, instr),
                    0b100 => Self::blt(&mut self.state, instr),
                    0b101 => Self::bge(&mut self.state, instr),
                    0b110 => Self::bltu(&mut self.state, instr),
                    0b111 => Self::bgeu(&mut self.state, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0000011 => match get_funct3(instr) {
                    0b000 => Self::lb(&mut self.state, &mut self.mmu, instr),
                    0b001 => Self::lh(&mut self.state, &mut self.mmu, instr),
                    0b010 => Self::lw(&mut self.state, &mut self.mmu, instr),
                    0b100 => Self::lbu(&mut self.state, &mut self.mmu, instr),
                    0b101 => Self::lhu(&mut self.state, &mut self.mmu, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0100011 => match get_funct3(instr) {
                    0b000 => Self::sb(&mut self.state, &mut self.mmu, instr),
                    0b001 => Self::sh(&mut self.state, &mut self.mmu, instr),
                    0b010 => Self::sw(&mut self.state, &mut self.mmu, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0010011 => match get_funct3(instr) {
                    0b000 => Self::addi(&mut self.state, instr),
                    0b010 => Self::slti(&mut self.state, instr),
                    0b011 => Self::sltiu(&mut self.state, instr),
                    0b100 => Self::xori(&mut self.state, instr),
                    0b110 => Self::ori(&mut self.state, instr),
                    0b111 => Self::andi(&mut self.state, instr),
                    0b001 => match get_funct7(instr) {
                        0 => Self::slli(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b101 => match get_funct7(instr) {
                        0 => Self::srli(&mut self.state, instr),
                        0b0100000 => Self::srai(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    _ => unreachable!(),
                },
                0b0110011 => match get_funct3(instr) {
                    0b000 => match get_funct7(instr) {
                        0 => Self::add(&mut self.state, instr),
                        1 => Self::mul(&mut self.state, instr),
                        0b0100000 => Self::sub(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b001 => match get_funct7(instr) {
                        0 => Self::sll(&mut self.state, instr),
                        1 => Self::mulh(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b010 => match get_funct7(instr) {
                        0 => Self::slt(&mut self.state, instr),
                        1 => Self::mulhsu(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b011 => match get_funct7(instr) {
                        0 => Self::sltu(&mut self.state, instr),
                        1 => Self::mulhu(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b100 => match get_funct7(instr) {
                        0 => Self::xor(&mut self.state, instr),
                        1 => Self::div(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b101 => match get_funct7(instr) {
                        0 => Self::srl(&mut self.state, instr),
                        1 => Self::divu(&mut self.state, instr),
                        0b0100000 => Self::sra(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b110 => match get_funct7(instr) {
                        0 => Self::or(&mut self.state, instr),
                        1 => Self::rem(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b111 => match get_funct7(instr) {
                        0 => Self::and(&mut self.state, instr),
                        1 => Self::remu(&mut self.state, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    _ => unreachable!(),
                },
                0b0001111 => match get_funct3(instr) {
                    0b000 => Self::fence(&mut self.state, instr),
                    0b001 => Self::fence_i(&mut self.state, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b1110011 => match get_funct3(instr) {
                    0b000 => match get_imm_i_type(instr) {
                        0 => Self::ecall(&mut self.state, instr),
                        1 => Self::ebreak(&mut self.state, instr),

                        a @ _ => {
                            if get_rs1(instr) == 0 && get_rd(instr) == 0 {
                                match a {
                                    0b000100000010 => Self::sret(&mut self.state, instr),
                                    0b001100000010 => Self::mret(&mut self.state, instr),
                                    0b000100000101 => Self::wfi(&mut self.state, instr),
                                    _ => Err(Trap {
                                        tcause: crate::traps::TrapType::IllegalInstruction,
                                        tval: instr,
                                    }),
                                }
                            } else {
                                Err(Trap {
                                    tcause: crate::traps::TrapType::IllegalInstruction,
                                    tval: instr,
                                })
                            }
                        }
                    },
                    0b001 => Self::csrrw(&mut self.state, &mut self.mmu, instr),
                    0b010 => Self::csrrs(&mut self.state, &mut self.mmu, instr),
                    0b011 => Self::csrrc(&mut self.state, &mut self.mmu, instr),
                    0b101 => Self::csrrwi(&mut self.state, &mut self.mmu, instr),
                    0b110 => Self::csrrsi(&mut self.state, &mut self.mmu, instr),
                    0b111 => Self::csrrci(&mut self.state, &mut self.mmu, instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0101111 => match get_funct3(instr) {
                    0b010 => match get_rs3(instr) {
                        0b00010 => Self::lr_w(&mut self.state, &mut self.mmu, instr),
                        0b00011 => Self::sc_w(&mut self.state, &mut self.mmu, instr),
                        0b00001 => Self::amoswap_w(&mut self.state, &mut self.mmu, instr),
                        0b00000 => Self::amoadd_w(&mut self.state, &mut self.mmu, instr),
                        0b00100 => Self::amoxor_w(&mut self.state, &mut self.mmu, instr),
                        0b01100 => Self::amoand_w(&mut self.state, &mut self.mmu, instr),
                        0b01000 => Self::amoor_w(&mut self.state, &mut self.mmu, instr),
                        0b10000 => Self::amomin_w(&mut self.state, &mut self.mmu, instr),
                        0b10100 => Self::amomax_w(&mut self.state, &mut self.mmu, instr),
                        0b11000 => Self::amominu_w(&mut self.state, &mut self.mmu, instr),
                        0b11100 => Self::amomaxu_w(&mut self.state, &mut self.mmu, instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                _ => Err(Trap {
                    tcause: crate::traps::TrapType::IllegalInstruction,
                    tval: instr,
                }),
            }
            .map(|_| 4)
        } else {
            //Compressed instructions are 16 bits wide
            self.execute_compressed(instr).map(|_| 2)
        }
    }
    fn execute_compressed(&mut self, instr: u32) -> Result<(), Trap> {
        let compressed = instr as u16;
        match compressed & 0b11 {
            //Quadrant 0
            0b00 => match get_compressed_func3(compressed) {
                0b000 => match get_compressed_ciw_addi4spn_imm(compressed) {
                    //nzuimm == 0 is a reserved encoding
                    0 => Err(illegal_instruction(instr)),
                    _ => Self::c_addi4spn(&mut self.state, compressed),
                },
                0b010 => Self::c_lw(&mut self.state, &mut self.mmu, compressed),
                0b110 => Self::c_sw(&mut self.state, &mut self.mmu, compressed),
                //0b001/0b011/0b101/0b111 are the F/D load and store forms, 0b100 is reserved
                _ => Err(illegal_instruction(instr)),
            },
            //Quadrant 1
            0b01 => match get_compressed_func3(compressed) {
                0b000 => match get_compressed_rd(compressed) {
                    0 => Self::c_nop(&mut self.state, compressed),
                    _ => Self::c_addi(&mut self.state, compressed),
                },
                0b001 => Self::c_jal(&mut self.state, compressed),
                0b010 => Self::c_li(&mut self.state, compressed),
                0b011 => {
                    let rd = get_compressed_rd(compressed);
                    //C.ADDI16SP and C.LUI share the encoding, rd == 2 selects C.ADDI16SP
                    let nzimm = if rd == 2 {
                        get_compressed_ci_addi16sp_imm(compressed)
                    } else {
                        get_compressed_ci_lui_imm(compressed)
                    };
                    if nzimm == 0 {
                        //nzimm == 0 is a reserved encoding for both instructions
                        Err(illegal_instruction(instr))
                    } else {
                        match rd {
                            //rd == 0 is a HINT for C.LUI and executes as a no-op
                            0 => Self::c_nop(&mut self.state, compressed),
                            2 => Self::c_addi16sp(&mut self.state, compressed),
                            _ => Self::c_lui(&mut self.state, compressed),
                        }
                    }
                }
                0b100 => match get_compressed_func6(compressed) {
                    0b100000 => Self::c_srli(&mut self.state, compressed),
                    0b100001 => Self::c_srai(&mut self.state, compressed),
                    //C.ANDI: bit 12 is imm[5], so both variants are the same instruction
                    0b100010 | 0b100110 => Self::c_andi(&mut self.state, compressed),
                    0b100011 => match get_compressed_func(compressed) {
                        0b00 => Self::c_sub(&mut self.state, compressed),
                        0b01 => Self::c_xor(&mut self.state, compressed),
                        0b10 => Self::c_or(&mut self.state, compressed),
                        0b11 => Self::c_and(&mut self.state, compressed),
                        _ => unreachable!(),
                    },
                    //0b100111 is C.SUBW/C.ADDW, which is RV64 only, while 0b100100/0b100101
                    //carry shamt[5], which must be zero for RV32C
                    _ => Err(illegal_instruction(instr)),
                },
                0b101 => Self::c_j(&mut self.state, compressed),
                0b110 => Self::c_beqz(&mut self.state, compressed),
                0b111 => Self::c_bnez(&mut self.state, compressed),
                _ => unreachable!(),
            },
            //Quadrant 2
            0b10 => match get_compressed_func3(compressed) {
                0b000 => match (compressed >> 12) & 1 {
                    //shamt[5] must be zero for RV32C
                    0 => Self::c_slli(&mut self.state, compressed),
                    _ => Err(illegal_instruction(instr)),
                },
                0b010 => match get_compressed_rd(compressed) {
                    //rd == 0 is a reserved encoding
                    0 => Err(illegal_instruction(instr)),
                    _ => Self::c_lwsp(&mut self.state, &mut self.mmu, compressed),
                },
                0b100 => {
                    let rd = get_compressed_rd(compressed);
                    let rs2 = get_compressed_rs2(compressed);
                    //Bit 12 splits the register form from the arithmetic form
                    if (compressed >> 12) & 1 == 0 {
                        match (rd, rs2) {
                            //C.JR with rs1 == 0 is a reserved encoding
                            (0, 0) => Err(illegal_instruction(instr)),
                            (_, 0) => Self::c_jr(&mut self.state, compressed),
                            _ => Self::c_mv(&mut self.state, compressed),
                        }
                    } else {
                        match (rd, rs2) {
                            (0, 0) => Self::c_ebreak(&mut self.state, compressed),
                            (_, 0) => Self::c_jalr(&mut self.state, compressed),
                            _ => Self::c_add(&mut self.state, compressed),
                        }
                    }
                }
                0b110 => Self::c_swsp(&mut self.state, &mut self.mmu, compressed),
                //0b001/0b011/0b101/0b111 are the F/D forms
                _ => Err(illegal_instruction(instr)),
            },
            //0b11 is a 32 bit instruction and never reaches this point
            _ => unreachable!(),
        }
    }
    fn lui(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_u_type(instr);
        state.set_x(rd, imm);
        Ok(())
    }
    fn auipc(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_u_type(instr);
        state.set_x(rd, imm.wrapping_add(state.pc));
        Ok(())
    }
    fn jal(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_j_type(instr);
        let t = state.pc.wrapping_add(imm);
        //if t & 0b11 == 0 {
        state.set_x(rd, state.pc.wrapping_add(4));
        state.pc = t.wrapping_sub(4);
        Ok(())
        //} else {
        //    Err(Trap {
        //        trap_type: crate::traps::TrapType::InstructionAddressMisaligned,
        //        value: state.pc,
        //    })
        //}
    }
    fn jalr(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let rs1 = get_rs1(instr);
        let imm = get_imm_i_type(instr);
        let t = (state.get_x(rs1).wrapping_add(imm)) & !1;
        //if t & 0b11 == 0 {
        state.set_x(rd, state.pc.wrapping_add(4));
        state.pc = t.wrapping_sub(4);
        Ok(())
        //} else {
        //    Err(Trap {
        //        trap_type: crate::traps::TrapType::InstructionAddressMisaligned,
        //        value: state.pc,
        //    })
        //}
    }
    fn beq(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if state.get_x(rs1) == state.get_x(rs2) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bne(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if state.get_x(rs1) != state.get_x(rs2) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn blt(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if (state.get_x(rs1) as i32) < (state.get_x(rs2) as i32) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bge(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if (state.get_x(rs1) as i32) >= (state.get_x(rs2) as i32) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bltu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if state.get_x(rs1) < state.get_x(rs2) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bgeu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if state.get_x(rs1) >= state.get_x(rs2) {
            state.pc = state.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn lb(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        let res = mmu.read_byte(address)? as i8 as i32 as u32;
        state.set_x(rd, res);
        Ok(())
    }
    fn lh(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        let res = mmu.read_halfword(address)? as i16 as i32 as u32;
        state.set_x(rd, res);
        Ok(())
    }
    fn lw(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        let res = mmu.read_word(address)?;
        state.set_x(rd, res);
        Ok(())
    }
    fn lbu(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        let res = mmu.read_byte(address)? as u32;
        state.set_x(rd, res);
        Ok(())
    }
    fn lhu(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        let res = mmu.read_halfword(address)? as u32;
        state.set_x(rd, res);
        Ok(())
    }
    fn sb(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        mmu.write_byte(address, state.get_x(rs2) as _)
    }
    fn sh(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        mmu.write_halfword(address, state.get_x(rs2) as _)
    }
    fn sw(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = state.get_x(rs1).wrapping_add(imm);
        mmu.write_word(address, state.get_x(rs2) as _)
    }
    fn addi(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(rd, state.get_x(rs1).wrapping_add(imm));
        Ok(())
    }
    fn slti(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(
            rd,
            if (state.get_x(rs1) as i32) < (imm as i32) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn sltiu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(rd, if state.get_x(rs1) < imm { 1 } else { 0 });
        Ok(())
    }
    fn xori(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(rd, state.get_x(rs1) ^ imm);
        Ok(())
    }
    fn ori(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(rd, state.get_x(rs1) | imm);
        Ok(())
    }
    fn andi(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        state.set_x(rd, state.get_x(rs1) & imm);
        Ok(())
    }
    fn slli(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        state.set_x(rd, state.get_x(rs1).wrapping_shl(shamt));
        Ok(())
    }
    fn srli(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        state.set_x(rd, state.get_x(rs1).wrapping_shr(shamt));
        Ok(())
    }
    fn srai(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        state.set_x(rd, (state.get_x(rs1) as i32).wrapping_shr(shamt) as u32);
        Ok(())
    }
    fn add(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(rd, state.get_x(rs1).wrapping_add(state.get_x(rs2)));
        Ok(())
    }
    fn sub(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(rd, state.get_x(rs1).wrapping_sub(state.get_x(rs2)));
        Ok(())
    }

    fn slt(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(
            rd,
            if (state.get_x(rs1) as i32) < (state.get_x(rs2) as i32) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn sltu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(
            rd,
            if state.get_x(rs1) < state.get_x(rs2) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn xor(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(rd, state.get_x(rs1) ^ state.get_x(rs2));
        Ok(())
    }
    fn sll(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(
            rd,
            state.get_x(rs1).wrapping_shl(state.get_x(rs2) & 0b11111),
        );
        Ok(())
    }
    fn srl(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(
            rd,
            state.get_x(rs1).wrapping_shr(state.get_x(rs2) & 0b11111),
        );
        Ok(())
    }
    fn sra(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(
            rd,
            (state.get_x(rs1) as i32).wrapping_shr(state.get_x(rs2) & 0b11111) as u32,
        );
        Ok(())
    }
    fn or(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(rd, state.get_x(rs1) | state.get_x(rs2));
        Ok(())
    }
    fn and(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        state.set_x(rd, state.get_x(rs1) & state.get_x(rs2));
        Ok(())
    }
    fn fence(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        Ok(())
    }
    fn fence_i(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        Ok(())
    }
    fn ecall(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let exception_type = match state.privilege {
            PrivilegeMode::User => TrapType::EnvironmentCallFromUMode,
            PrivilegeMode::Supervisor => TrapType::EnvironmentCallFromSMode,
            PrivilegeMode::Machine | PrivilegeMode::Reserved => TrapType::EnvironmentCallFromMMode,
        };
        return Err(Trap {
            tcause: exception_type,
            tval: 0,
        });
    }
    fn ebreak(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let exception_type = TrapType::Breakpoint;
        return Err(Trap {
            tcause: exception_type,
            tval: state.pc,
        });
    }

    //Machine interrupt pending bits, with MSIP and MTIP driven by the CLINT.
    fn pending_mip(state: &CPUState, mmu: &mut MMU) -> u32 {
        let mut mip = state.mip & !0b1000_1000;
        if mmu.msip() {
            mip |= 1 << 3;
        }
        if mmu.mtip() {
            mip |= 1 << 7;
        }
        mip
    }
    fn get_csr(state: &CPUState, mmu: &mut MMU, csr: u16) -> Result<u32, Trap> {
        //TODO: privileges check
        Ok(match csr {
            0x340 => state.mscratch,
            0x305 => state.mtvec,
            0x304 => state.mie,

            0x344 => Self::pending_mip(state, mmu),
            0x341 => state.mepc,
            0x342 => state.mcause,
            0x343 => state.mtval,
            0x3a0..=0x3a3 => state.pmpcfg[(csr - 0x3a0) as usize],
            0x3b0..=0x3bf => state.pmpaddr[(csr - 0x3b0) as usize],
            0xf11 => 0xff0ff0ff,                              //vendorId
            0xf12 => 0x0,                                     //marchid
            0xf13 => 0x0,                                     //mimpid
            0xf14 => 0x0,                                     //mhartid
            0xf15 => 0x0,                                     //mconfigptr
            0x301 => 0b01_0_10000_00000000_00010001_00000101, //misa, XLEN=32, IMAC

            0x300 => state.mstatus as u32,         //mstatus
            0x310 => (state.mstatus >> 32) as u32, //mstatus

            0xC00 => state.cycle as u32,
            0xC80 => (state.cycle >> 32) as u32,

            0xC01 => mmu.mtime() as u32,         //time
            0xC81 => (mmu.mtime() >> 32) as u32, //timeh
            _ => todo!("CSR {csr:0x} not implemented"),
        })
    }
    //Is register writable
    fn set_csr(state: &mut CPUState, csr: u16, new_val: u32) -> Result<bool, Trap> {
        //TODO: privileges check
        match csr {
            0x340 => state.mscratch = new_val,
            0x305 => state.mtvec = new_val & !0b11, //Only direct mode is implemented
            0x304 => state.mie = new_val,
            0xC00 => {
                return Ok(false);
            } //cyclel
            0xC80 => {
                return Ok(false);
            } //cycleh
            0x344 => state.mip = new_val & !0b1000_1000, //MSIP and MTIP are CLINT-driven
            0x341 => state.mepc = new_val,
            0x3a0..=0x3a3 => state.pmpcfg[(csr - 0x3a0) as usize] = new_val,
            0x3b0..=0x3bf => state.pmpaddr[(csr - 0x3b0) as usize] = new_val,
            0x300 => {
                state.mstatus &= !(u32::MAX as u64);
                state.mstatus |= new_val as u64
            } //mstatus
            0x310 => {
                state.mstatus &= u32::MAX as u64;
                state.mstatus |= (new_val as u64) << 32
            }
            0x342 => state.mcause = new_val,
            0x343 => state.mtval = new_val,
            0xf11..=0xf15 => {
                return Ok(false);
            } //vendorId
            0x301 => {
                return Ok(false);
            } //misa, XLEN=32, IMAC
            _ => todo!("CSR 0x{csr:0x} not implemented"),
        };
        Ok(true)
    }
    fn sret(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        todo!("SRET is not implemented")
    }
    fn mret(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        //MIE = MPIE, MPIE = 1, MPP = M (the least privileged supported mode)
        let mut mstatus = state.mstatus as u32;
        let mpie = mstatus & MSTATUS_MPIE != 0;
        let mpp = (mstatus >> MSTATUS_MPP_SHIFT) & 0b11;
        mstatus &= !(MSTATUS_MIE | MSTATUS_MPIE | (0b11 << MSTATUS_MPP_SHIFT));
        if mpie {
            mstatus |= MSTATUS_MIE;
        }
        mstatus |= MSTATUS_MPIE;
        mstatus |= 3 << MSTATUS_MPP_SHIFT;
        state.mstatus = (state.mstatus & 0xffff_ffff_0000_0000) | mstatus as u64;
        state.privilege = match mpp {
            0 => PrivilegeMode::User,
            1 => PrivilegeMode::Supervisor,
            3 => PrivilegeMode::Machine,
            _ => PrivilegeMode::Reserved,
        };
        state.pc = state.mepc.wrapping_sub(4);
        Ok(())
    }
    fn wfi(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        //CPU::step keeps the hart idle until an enabled interrupt is pending.
        state.wfi = true;
        Ok(())
    }
    fn csrrw(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rd != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            if !Self::set_csr(state, csr, state.get_x(rs1))? {
                todo!("Handle write in read-only registers")
            }
            state.set_x(rd, val)
        } else {
            if !Self::set_csr(state, csr, state.get_x(rs1))? {
                todo!("Handle write in read-only registers")
            }
        }
        Ok(())
    }
    fn csrrs(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rs1 != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            let set_bits = val | state.get_x(rs1);
            if !Self::set_csr(state, csr, set_bits)? {
                todo!("Handle read in write-only registers")
            }
            state.set_x(rd, val)
        } else {
            let val = Self::get_csr(state, mmu, csr)?;
            state.set_x(rd, val)
        }
        Ok(())
    }
    fn csrrc(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rs1 != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            let set_bits = val & !state.get_x(rs1);
            if !Self::set_csr(state, csr, set_bits)? {
                todo!("Handle read in write-only registers")
            }
            state.set_x(rd, val)
        } else {
            let val = Self::get_csr(state, mmu, csr)?;
            state.set_x(rd, val)
        }
        Ok(())
    }
    fn csrrwi(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rd != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            if !Self::set_csr(state, csr, rs1 as u32)? {
                todo!("Handle write in read-only registers")
            }
            state.set_x(rd, val)
        } else {
            if !Self::set_csr(state, csr, rs1 as u32)? {
                todo!("Handle write in read-only registers")
            }
        }
        Ok(())
    }
    fn csrrsi(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let zimm = get_rs1(instr) as u32;
        let rd = get_rd(instr);
        if zimm != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            let set_bits = val | zimm;
            if !Self::set_csr(state, csr, set_bits)? {
                todo!("Handle read in write-only registers")
            }
            state.set_x(rd, val)
        } else {
            let val = Self::get_csr(state, mmu, csr)?;
            state.set_x(rd, val)
        }
        Ok(())
    }
    fn csrrci(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let zimm = get_rs1(instr) as u32;
        let rd = get_rd(instr);
        if zimm != 0 {
            let val = Self::get_csr(state, mmu, csr)?;
            let clear_bits = val & !zimm;
            if !Self::set_csr(state, csr, clear_bits)? {
                todo!("Handle read in write-only registers")
            }
            state.set_x(rd, val)
        } else {
            let val = Self::get_csr(state, mmu, csr)?;
            state.set_x(rd, val)
        }
        Ok(())
    }
    fn mul(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (state.get_x(rs1) as i32).wrapping_mul(state.get_x(rs2) as i32);
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn mulh(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (state.get_x(rs1) as i32 as i64).wrapping_mul(state.get_x(rs2) as i32 as i64);
        let m = (m as u64) >> 32;
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn mulhsu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (state.get_x(rs1) as i32 as i64).wrapping_mul(state.get_x(rs2) as u64 as i64);
        let m = m >> 32;
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn mulhu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (state.get_x(rs1) as u64).wrapping_mul(state.get_x(rs2) as u64);
        let m = m >> 32;
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn div(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = state.get_x(rs1);
        let rs2 = state.get_x(rs2);

        let m = if rs2 != 0 {
            (rs1 as i32).wrapping_div(rs2 as i32)
        } else {
            -1
        };
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn divu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = state.get_x(rs1);
        let rs2 = state.get_x(rs2);

        let m = if rs2 != 0 {
            rs1.wrapping_div(rs2)
        } else {
            u32::MAX
        };
        state.set_x(rd, m);
        Ok(())
    }
    fn rem(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = state.get_x(rs1);
        let rs2 = state.get_x(rs2);

        let m = if rs2 != 0 {
            (rs1 as i32).wrapping_rem(rs2 as i32)
        } else {
            rs1 as i32
        };
        state.set_x(rd, m as u32);
        Ok(())
    }
    fn remu(state: &mut CPUState, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = state.get_x(rs1);
        let rs2 = state.get_x(rs2);

        let m = if rs2 != 0 { rs1.wrapping_rem(rs2) } else { rs1 };
        state.set_x(rd, m);
        Ok(())
    }
    fn lr_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if get_rs2(instr) != 0 {
            //rs2 is reserved for LR.W
            return Err(illegal_instruction(instr));
        }
        let address = state.get_x(rs1);
        let value = mmu.read_word_atomically(address, AtomicAccess::Load)?;
        state.reservation_slot = Some(address);
        state.set_x(rd, value);
        Ok(())
    }
    fn sc_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let reserved = state.reservation_slot == Some(address);
        state.reservation_slot = None;
        if reserved {
            mmu.write_word_atomically(address, state.get_x(rs2))?;
            state.set_x(rd, 0);
        } else {
            //A failed SC.W still accesses its address, so the address check must still happen
            mmu.read_word_atomically(address, AtomicAccess::Store)?;
            state.set_x(rd, 1);
        }
        Ok(())
    }
    fn amoswap_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        mmu.write_word_atomically(address, state.get_x(rs2))?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amoadd_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let new = old.wrapping_add(state.get_x(rs2));
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amoxor_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let new = old ^ state.get_x(rs2);
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amoand_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let new = old & state.get_x(rs2);
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amoor_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let new = old | state.get_x(rs2);
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amomin_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let value = state.get_x(rs2);
        let new = if (old as i32) < (value as i32) {
            old
        } else {
            value
        };
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amomax_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let value = state.get_x(rs2);
        let new = if (old as i32) > (value as i32) {
            old
        } else {
            value
        };
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amominu_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let value = state.get_x(rs2);
        let new = if old < value { old } else { value };
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }
    fn amomaxu_w(state: &mut CPUState, mmu: &mut MMU, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let address = state.get_x(rs1);
        let old = mmu.read_word_atomically(address, AtomicAccess::Store)?;
        let value = state.get_x(rs2);
        let new = if old > value { old } else { value };
        mmu.write_word_atomically(address, new)?;
        state.set_x(rd, old);
        Ok(())
    }

    fn c_addi4spn(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rdc(instr) + 8;
        let nzuimm = get_compressed_ciw_addi4spn_imm(instr);
        state.set_x(rd, state.get_x(2).wrapping_add(nzuimm));
        Ok(())
    }
    fn c_lw(state: &mut CPUState, mmu: &mut MMU, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rdc(instr) + 8;
        let rs1 = get_compressed_rs1c(instr) + 8;
        let uimm = get_compressed_cl_mem_load_32_imm(instr);
        let address = state.get_x(rs1).wrapping_add(uimm);
        let value = mmu.read_word(address)?;
        state.set_x(rd, value);
        Ok(())
    }
    fn c_sw(state: &mut CPUState, mmu: &mut MMU, instr: u16) -> Result<(), Trap> {
        let rs2 = get_compressed_rdc(instr) + 8;
        let rs1 = get_compressed_rs1c(instr) + 8;
        let uimm = get_compressed_cs_mem_store_32_imm(instr);
        let address = state.get_x(rs1).wrapping_add(uimm);
        mmu.write_word(address, state.get_x(rs2))
    }
    fn c_nop(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        Ok(())
    }
    fn c_addi(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let imm = get_compressed_ci_li_addi_imm(instr);
        state.set_x(rd, state.get_x(rd).wrapping_add(imm));
        Ok(())
    }
    fn c_jal(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let offset = get_compressed_cj_jump_imm(instr);
        state.set_x(1, state.pc.wrapping_add(2));
        state.pc = state.pc.wrapping_add(offset).wrapping_sub(2);
        Ok(())
    }
    fn c_li(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let imm = get_compressed_ci_li_addi_imm(instr);
        state.set_x(rd, imm);
        Ok(())
    }
    fn c_addi16sp(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let imm = get_compressed_ci_addi16sp_imm(instr);
        state.set_x(2, state.get_x(2).wrapping_add(imm));
        Ok(())
    }
    fn c_lui(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let imm = get_compressed_ci_lui_imm(instr);
        state.set_x(rd, imm);
        Ok(())
    }
    fn c_srli(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let shamt = get_compressed_cb_shift_imm(instr);
        state.set_x(rd, state.get_x(rd).wrapping_shr(shamt));
        Ok(())
    }
    fn c_srai(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let shamt = get_compressed_cb_shift_imm(instr);
        state.set_x(rd, (state.get_x(rd) as i32).wrapping_shr(shamt) as u32);
        Ok(())
    }
    fn c_andi(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let imm = get_compressed_cb_and_imm(instr);
        state.set_x(rd, state.get_x(rd) & imm);
        Ok(())
    }
    fn c_sub(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let rs2 = get_compressed_rdc(instr) + 8;
        state.set_x(rd, state.get_x(rd).wrapping_sub(state.get_x(rs2)));
        Ok(())
    }
    fn c_xor(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let rs2 = get_compressed_rdc(instr) + 8;
        state.set_x(rd, state.get_x(rd) ^ state.get_x(rs2));
        Ok(())
    }
    fn c_or(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let rs2 = get_compressed_rdc(instr) + 8;
        state.set_x(rd, state.get_x(rd) | state.get_x(rs2));
        Ok(())
    }
    fn c_and(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rs1c(instr) + 8;
        let rs2 = get_compressed_rdc(instr) + 8;
        state.set_x(rd, state.get_x(rd) & state.get_x(rs2));
        Ok(())
    }
    fn c_j(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let offset = get_compressed_cj_jump_imm(instr);
        state.pc = state.pc.wrapping_add(offset).wrapping_sub(2);
        Ok(())
    }
    fn c_beqz(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rs1 = get_compressed_rs1c(instr) + 8;
        let offset = get_compressed_cb_branch_imm(instr);
        if state.get_x(rs1) == 0 {
            state.pc = state.pc.wrapping_add(offset).wrapping_sub(2);
        }
        Ok(())
    }
    fn c_bnez(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rs1 = get_compressed_rs1c(instr) + 8;
        let offset = get_compressed_cb_branch_imm(instr);
        if state.get_x(rs1) != 0 {
            state.pc = state.pc.wrapping_add(offset).wrapping_sub(2);
        }
        Ok(())
    }
    fn c_slli(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let shamt = get_compressed_cb_shift_imm(instr);
        state.set_x(rd, state.get_x(rd).wrapping_shl(shamt));
        Ok(())
    }
    fn c_lwsp(state: &mut CPUState, mmu: &mut MMU, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let uimm = get_compressed_ci_stack_load_32_imm(instr);
        let address = state.get_x(2).wrapping_add(uimm);
        let value = mmu.read_word(address)?;
        state.set_x(rd, value);
        Ok(())
    }
    fn c_jr(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rs1 = get_compressed_rd(instr);
        let target = state.get_x(rs1) & !1;
        state.pc = target.wrapping_sub(2);
        Ok(())
    }
    fn c_mv(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let rs2 = get_compressed_rs2(instr);
        state.set_x(rd, state.get_x(rs2));
        Ok(())
    }
    fn c_ebreak(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        Err(Trap {
            tcause: TrapType::Breakpoint,
            tval: state.pc,
        })
    }
    fn c_jalr(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rs1 = get_compressed_rd(instr);
        let target = state.get_x(rs1) & !1;
        state.set_x(1, state.pc.wrapping_add(2));
        state.pc = target.wrapping_sub(2);
        Ok(())
    }
    fn c_add(state: &mut CPUState, instr: u16) -> Result<(), Trap> {
        let rd = get_compressed_rd(instr);
        let rs2 = get_compressed_rs2(instr);
        state.set_x(rd, state.get_x(rd).wrapping_add(state.get_x(rs2)));
        Ok(())
    }
    fn c_swsp(state: &mut CPUState, mmu: &mut MMU, instr: u16) -> Result<(), Trap> {
        let rs2 = get_compressed_rs2(instr);
        let uimm = get_compressed_css_stack_write_32_imm(instr);
        let address = state.get_x(2).wrapping_add(uimm);
        mmu.write_word(address, state.get_x(rs2))
    }
}

#[allow(dead_code)]
impl CPUState {
    fn new() -> Self {
        CPUState {
            x: [0; 32],
            pc: 0,
            mstatus: 0,
            cycle: 0,
            mscratch: 0,
            mtvec: 0,
            mie: 0,
            mip: 0,
            mepc: 0,
            mtval: 0,
            mcause: 0,
            pmpcfg: [0; 4],
            pmpaddr: [0; 16],
            privilege: PrivilegeMode::Machine,
            wfi: false,
            reservation_slot: None,
            stopflag: None,
        }
    }
    pub fn get_registers(&self) -> [u32; 32] {
        self.x
    }
    pub fn set_registers(&mut self, mut regs: [u32; 32]) {
        regs[0] = 0;
        self.x = regs
    }
    #[inline(always)]
    pub(crate) fn set_x(&mut self, x: u8, val: u32) {
        if x != 0 {
            self.x[x as usize] = val;
        }
    }
    #[inline(always)]
    fn get_x(&self, x: u8) -> u32 {
        self.x[x as usize]
    }
    pub fn process_trap(&mut self, trap: Trap) -> Result<(), EmulatorError> {
        // println!("TRAP!\nTRAP!\nTRAP!\nTRAP!\n{}", trap);
        if self.mtvec == 0 {
            return Err(EmulatorError::UnsetTrapHandler);
        }
        //A trap ends any WFI stall; the handler has to run.
        self.wfi = false;
        //MPIE = MIE, MIE = 0, MPP = the privilege that was interrupted
        let mut mstatus = self.mstatus as u32;
        let mie = mstatus & MSTATUS_MIE != 0;
        mstatus &= !(MSTATUS_MIE | MSTATUS_MPIE | (0b11 << MSTATUS_MPP_SHIFT));
        if mie {
            mstatus |= MSTATUS_MPIE;
        }
        mstatus |= (self.privilege as u32) << MSTATUS_MPP_SHIFT;
        self.mstatus = (self.mstatus & 0xffff_ffff_0000_0000) | mstatus as u64;
        self.privilege = PrivilegeMode::Machine;
        self.mepc = self.pc;
        self.mcause = trap.tcause as u32;
        self.mtval = trap.tval;
        let mode = self.mtvec & 0b11;

        match mode {
            0 => {
                self.pc = self.mtvec & !0b11;
            }
            1 => {
                todo!("Vectored trap handling is not implemented")
            }
            _ => {
                unreachable!()
            }
        }

        //TODO

        Ok(())

        //match trap.tcause {
        //    TrapType::InstructionAddressMisaligned => todo!(),
        //    TrapType::InstructionAccessFault => todo!(),
        //    TrapType::IllegalInstruction => todo!(),
        //    TrapType::Breakpoint => todo!(),
        //    TrapType::LoadAddressMisaligned => todo!(),
        //    TrapType::LoadAccessFault => todo!(),
        //    TrapType::StoreAddressMisaligned => todo!(),
        //    TrapType::StoreAccessFault => todo!(),
        //    TrapType::EnvironmentCallFromUMode => todo!(),
        //    TrapType::EnvironmentCallFromSMode => todo!(),
        //    TrapType::EnvironmentCallFromMMode => todo!(),
        //    TrapType::InstructionPageFault => todo!(),
        //    TrapType::LoadPageFault => todo!(),
        //    TrapType::StorePageFault => todo!(),
        //    TrapType::UserSoftwareInterrupt => todo!(),
        //    TrapType::SupervisorSoftwareInterrupt => todo!(),
        //    TrapType::MachineSoftwareInterrupt => todo!(),
        //    TrapType::UserTimerInterrupt => todo!(),
        //    TrapType::SupervisorTimerInterrupt => todo!(),
        //    TrapType::MachineTimerInterrupt => todo!(),
        //    TrapType::UserExternalInterrupt => todo!(),
        //    TrapType::SupervisorExternalInterrupt => todo!(),
        //    TrapType::MachineExternalInterrupt => todo!(),
        //}
    }
}

#[inline]
fn illegal_instruction(instr: u32) -> Trap {
    Trap {
        tcause: TrapType::IllegalInstruction,
        tval: instr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bus::{MemoryBus, RAM_ADDRESS, RAM_ADDRESS_END},
        mmu::MMU,
        ops_decode::encode_r_type,
    };

    //Builds a CPU with `bytes` placed at `address` and the pc pointing at them.
    fn cpu_with_bytes_at(address: u32, bytes: &[u8]) -> CPU {
        let (mut bus, _audio) = MemoryBus::new();
        for (offset, byte) in bytes.iter().enumerate() {
            assert!(bus.write_raw_to_ram(address + offset as u32, *byte));
        }
        let mut cpu = CPU::new(MMU::new(bus));
        cpu.state.pc = address;
        cpu
    }

    //Builds a CPU with `bytes` placed at the start of RAM and the pc pointing at them.
    fn cpu_with_instruction(bytes: &[u8]) -> CPU {
        cpu_with_bytes_at(RAM_ADDRESS, bytes)
    }

    //REMU is funct3=0b111 with funct7=1, one decoding slot away from REM (funct3=0b110).
    #[test]
    fn remu_is_dispatched() {
        let mut cpu =
            cpu_with_instruction(&encode_r_type(0b0110011, 5, 0b111, 6, 7, 1).to_le_bytes());
        let mut regs = cpu.state.get_registers();
        regs[6] = 0xFFFF_FFF9; //-7
        regs[7] = 3;
        cpu.state.set_registers(regs);

        cpu.execute_instruction().unwrap();

        //0xFFFF_FFF9 % 3 == 0 unsigned, while -7 % 3 == -1 signed
        assert_eq!(cpu.state.get_registers()[5], 0);
        assert_eq!(cpu.state.pc, RAM_ADDRESS + 4);
    }

    //The instruction width lives in the two least significant bits.
    #[test]
    fn fetch_respects_instruction_width() {
        //c.nop followed by 0xffff: only the first halfword belongs to the instruction
        let mut cpu = cpu_with_instruction(&[0x01, 0x00, 0xff, 0xff]);
        assert_eq!(cpu.fetch().unwrap(), 0x0000_0001);

        //lui with the second halfword 0xffff: both halfwords belong to the instruction
        let mut cpu = cpu_with_instruction(&[0xb7, 0x00, 0xff, 0xff]);
        assert_eq!(cpu.fetch().unwrap(), 0xffff_00b7);
    }

    //A halfword must fit entirely inside RAM: the old word fetch panicked here instead.
    #[test]
    fn fetch_traps_at_the_end_of_ram() {
        //A compressed instruction in the last byte of RAM leaves no room for its halfword
        let mut cpu = cpu_with_bytes_at(RAM_ADDRESS_END, &[]);
        assert_eq!(
            cpu.fetch().unwrap_err().tcause,
            TrapType::InstructionAccessFault
        );

        //A 32 bit instruction whose second halfword runs past the end of RAM
        let mut cpu = cpu_with_bytes_at(RAM_ADDRESS_END - 1, &[0xb7, 0x00]);
        assert_eq!(
            cpu.fetch().unwrap_err().tcause,
            TrapType::InstructionAccessFault
        );
    }

    //0x0000 is C.ADDI4SPN with nzuimm == 0, 0x6101 is C.ADDI16SP with nzimm == 0, 0x6081 is
    //C.LUI with nzimm == 0, 0x4002 is C.LWSP with rd == 0, 0x1002 is C.SLLI with shamt[5] set
    //and 0x8002 is C.JR with rs1 == 0; all of them are reserved encodings. Getting an illegal
    //instruction trap (instead of an unimplemented panic) proves the 16 bit path is reached.
    #[test]
    fn compressed_reserved_encoding_traps() {
        for encoding in [0x0000u16, 0x6101, 0x6081, 0x4002, 0x1002, 0x8002] {
            let mut cpu = cpu_with_instruction(&encoding.to_le_bytes());
            let trap = cpu.execute_instruction().unwrap_err();
            assert_eq!(
                trap.tcause,
                TrapType::IllegalInstruction,
                "encoding 0x{encoding:04x} should be reserved"
            );
            //A trap is raised before the pc is advanced
            assert_eq!(cpu.state.pc, RAM_ADDRESS);
        }
    }
}
