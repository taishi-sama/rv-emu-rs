use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

use crate::{
    errors::EmulatorError,
    mmu::{MMU, RAM_ADDRESS, RAM_SIZE},
    ops_decode::{
        get_compressed_ci_addi16sp_imm, get_compressed_ci_lui_imm, get_compressed_ciw_addi4spn_imm,
        get_compressed_func, get_compressed_func3, get_compressed_func6, get_compressed_rd,
        get_compressed_rs2, get_csr_num, get_funct3, get_funct7, get_imm_b_type, get_imm_i_type,
        get_imm_j_type, get_imm_s_type, get_imm_u_type, get_opcode, get_rd, get_rs1, get_rs2,
        get_rs3,
    },
    traps::{Trap, TrapType},
};

#[derive(Clone, Copy)]
pub enum PrivilegeMode {
    Machine = 0,
    Supervisor = 1,
    Reserved = 2,
    User = 3,
}

// RV32IMA
#[allow(dead_code)]
pub struct CPU {
    x: [u32; 32],
    pub pc: u32,
    pub mmu: MMU,

    //CSRs
    //pub CSRs: [u32; 4096],
    pub mstatus: u64,
    pub cycle: u64,
    pub mtimecmp: u64,

    pub time_crs_start: Instant,

    pub mscratch: u32,
    pub mtvec: u32,
    pub mie: u32,
    pub mip: u32,

    pub mepc: u32,
    pub mtval: u32,
    pub mcause: u32,
    pub privilege: PrivilegeMode,
    pub wfi: bool,
    pub reservation_slot: Option<u32>,
    pub stopflag: Option<Arc<AtomicBool>>,
}

#[allow(dead_code)]
#[allow(unused_variables)]
impl CPU {
    pub fn new(mmu: MMU) -> Self {
        let mut x = [0; 32];
        x[2] = RAM_ADDRESS + RAM_SIZE as u32 - 0x4;
        CPU {
            x,
            pc: 0,
            mmu,
            mstatus: 0,
            cycle: 0,
            mtimecmp: 0,
            mscratch: 0,
            mtvec: 0,
            mie: 0,
            mip: 0,
            mepc: 0,
            mtval: 0,
            mcause: 0,
            privilege: PrivilegeMode::Machine,
            wfi: false,
            reservation_slot: None,
            stopflag: None,
            time_crs_start: Instant::now(),
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
    fn set_x(&mut self, x: u8, val: u32) {
        if x != 0 {
            self.x[x as usize] = val;
        }
    }
    #[inline(always)]
    fn get_x(&self, x: u8) -> u32 {
        self.x[x as usize]
    }
    //pub fn run_debugger(&mut self) {}
    pub fn execute_instruction(&mut self) -> Result<u32, Trap> {
        let instr = self.fetch()?;
        let instr_len = self.execute(instr)? as u32;
        self.pc = self.pc.wrapping_add(instr_len);
        Ok(instr)
    }
    pub fn step(&mut self) -> Result<u32, EmulatorError> {
        let res = self.execute_instruction();
        match res {
            Ok(v) => {
                return Ok(v);
            }
            Err(t) => {
                self.process_trap(t)?;
                return Ok(0);
            }
        }
    }
    pub fn process_trap(&mut self, trap: Trap) -> Result<(), EmulatorError> {
        // println!("TRAP!\nTRAP!\nTRAP!\nTRAP!\n{}", trap);
        if self.mtvec == 0 {
            return Err(EmulatorError::UnsetTrapHandler);
        }
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

    //Instruction width is encoded in the two least significant bits: a 16 bit (compressed)
    //instruction has them set to something other than 0b11, otherwise the instruction continues
    //into the following halfword. The upper half of the returned word is zero for compressed
    //instructions.
    fn fetch(&mut self) -> Result<u32, Trap> {
        let lower = self.mmu.fetch_halfword(self.pc)?;
        if lower & 0b11 == 0b11 {
            let upper = self.mmu.fetch_halfword(self.pc.wrapping_add(2))?;
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
                0b0110111 => self.lui(instr),
                0b0010111 => self.auipc(instr),
                0b1101111 => self.jal(instr),
                0b1100111 => match get_funct3(instr) {
                    0 => self.jalr(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b1100011 => match get_funct3(instr) {
                    0b000 => self.beq(instr),
                    0b001 => self.bne(instr),
                    0b100 => self.blt(instr),
                    0b101 => self.bge(instr),
                    0b110 => self.bltu(instr),
                    0b111 => self.bgeu(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0000011 => match get_funct3(instr) {
                    0b000 => self.lb(instr),
                    0b001 => self.lh(instr),
                    0b010 => self.lw(instr),
                    0b100 => self.lbu(instr),
                    0b101 => self.lhu(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0100011 => match get_funct3(instr) {
                    0b000 => self.sb(instr),
                    0b001 => self.sh(instr),
                    0b010 => self.sw(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0010011 => match get_funct3(instr) {
                    0b000 => self.addi(instr),
                    0b010 => self.slti(instr),
                    0b011 => self.sltiu(instr),
                    0b100 => self.xori(instr),
                    0b110 => self.ori(instr),
                    0b111 => self.andi(instr),
                    0b001 => match get_funct7(instr) {
                        0 => self.slli(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b101 => match get_funct7(instr) {
                        0 => self.srli(instr),
                        0b0100000 => self.srai(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    _ => unreachable!(),
                },
                0b0110011 => match get_funct3(instr) {
                    0b000 => match get_funct7(instr) {
                        0 => self.add(instr),
                        1 => self.mul(instr),
                        0b0100000 => self.sub(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b001 => match get_funct7(instr) {
                        0 => self.sll(instr),
                        1 => self.mulh(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b010 => match get_funct7(instr) {
                        0 => self.slt(instr),
                        1 => self.mulhsu(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b011 => match get_funct7(instr) {
                        0 => self.sltu(instr),
                        1 => self.mulhu(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b100 => match get_funct7(instr) {
                        0 => self.xor(instr),
                        1 => self.div(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b101 => match get_funct7(instr) {
                        0 => self.srl(instr),
                        1 => self.divu(instr),
                        0b0100000 => self.sra(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b110 => match get_funct7(instr) {
                        0 => self.or(instr),
                        1 => self.rem(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    0b111 => match get_funct7(instr) {
                        0 => self.and(instr),
                        1 => self.remu(instr),
                        _ => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                    },
                    _ => unreachable!(),
                },
                0b0001111 => match get_funct3(instr) {
                    0b000 => self.fence(instr),
                    0b001 => self.fence_i(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b1110011 => match get_funct3(instr) {
                    0b000 => match get_imm_i_type(instr) {
                        0 => self.ecall(instr),
                        1 => self.ebreak(instr),

                        a @ _ => {
                            if get_rs1(instr) == 0 && get_rd(instr) == 0 {
                                match a {
                                    0b000100000010 => self.sret(instr),
                                    0b001100000010 => self.mret(instr),
                                    0b000100000101 => self.wfi(instr),
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
                    0b001 => self.csrrw(instr),
                    0b010 => self.csrrs(instr),
                    0b011 => self.csrrc(instr),
                    0b101 => self.csrrwi(instr),
                    0b110 => self.csrrsi(instr),
                    0b111 => self.csrrci(instr),
                    _ => Err(Trap {
                        tcause: crate::traps::TrapType::IllegalInstruction,
                        tval: instr,
                    }),
                },
                0b0101111 => match get_funct3(instr) {
                    0b010 => match get_rs3(instr) {
                        0b00010 => self.lr_w(instr),
                        0b00011 => self.sc_w(instr),
                        0b00001 => self.amoswap_w(instr),
                        0b00000 => self.amoadd_w(instr),
                        0b00100 => self.amoxor_w(instr),
                        0b01100 => self.amoand_w(instr),
                        0b01000 => self.amoor_w(instr),
                        0b10000 => self.amomin_w(instr),
                        0b10100 => self.amomax_w(instr),
                        0b11000 => self.amominu_w(instr),
                        0b11100 => self.amomaxu_w(instr),
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
                    _ => self.c_addi4spn(compressed),
                },
                0b010 => self.c_lw(compressed),
                0b110 => self.c_sw(compressed),
                //0b001/0b011/0b101/0b111 are the F/D load and store forms, 0b100 is reserved
                _ => Err(illegal_instruction(instr)),
            },
            //Quadrant 1
            0b01 => match get_compressed_func3(compressed) {
                0b000 => match get_compressed_rd(compressed) {
                    0 => self.c_nop(compressed),
                    _ => self.c_addi(compressed),
                },
                0b001 => self.c_jal(compressed),
                0b010 => self.c_li(compressed),
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
                            0 => self.c_nop(compressed),
                            2 => self.c_addi16sp(compressed),
                            _ => self.c_lui(compressed),
                        }
                    }
                }
                0b100 => match get_compressed_func6(compressed) {
                    0b100000 => self.c_srli(compressed),
                    0b100001 => self.c_srai(compressed),
                    //C.ANDI: bit 12 is imm[5], so both variants are the same instruction
                    0b100010 | 0b100110 => self.c_andi(compressed),
                    0b100011 => match get_compressed_func(compressed) {
                        0b00 => self.c_sub(compressed),
                        0b01 => self.c_xor(compressed),
                        0b10 => self.c_or(compressed),
                        0b11 => self.c_and(compressed),
                        _ => unreachable!(),
                    },
                    //0b100111 is C.SUBW/C.ADDW, which is RV64 only, while 0b100100/0b100101
                    //carry shamt[5], which must be zero for RV32C
                    _ => Err(illegal_instruction(instr)),
                },
                0b101 => self.c_j(compressed),
                0b110 => self.c_beqz(compressed),
                0b111 => self.c_bnez(compressed),
                _ => unreachable!(),
            },
            //Quadrant 2
            0b10 => match get_compressed_func3(compressed) {
                0b000 => match (compressed >> 12) & 1 {
                    //shamt[5] must be zero for RV32C
                    0 => self.c_slli(compressed),
                    _ => Err(illegal_instruction(instr)),
                },
                0b010 => match get_compressed_rd(compressed) {
                    //rd == 0 is a reserved encoding
                    0 => Err(illegal_instruction(instr)),
                    _ => self.c_lwsp(compressed),
                },
                0b100 => {
                    let rd = get_compressed_rd(compressed);
                    let rs2 = get_compressed_rs2(compressed);
                    //Bit 12 splits the register form from the arithmetic form
                    if (compressed >> 12) & 1 == 0 {
                        match (rd, rs2) {
                            //C.JR with rs1 == 0 is a reserved encoding
                            (0, 0) => Err(illegal_instruction(instr)),
                            (_, 0) => self.c_jr(compressed),
                            _ => self.c_mv(compressed),
                        }
                    } else {
                        match (rd, rs2) {
                            (0, 0) => self.c_ebreak(compressed),
                            (_, 0) => self.c_jalr(compressed),
                            _ => self.c_add(compressed),
                        }
                    }
                }
                0b110 => self.c_swsp(compressed),
                //0b001/0b011/0b101/0b111 are the F/D forms
                _ => Err(illegal_instruction(instr)),
            },
            //0b11 is a 32 bit instruction and never reaches this point
            _ => unreachable!(),
        }
    }
    fn timer_reader(&self) -> u64 {
        self.time_crs_start.elapsed().as_micros() as u64
    }
    fn lui(&mut self, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_u_type(instr);
        self.set_x(rd, imm);
        Ok(())
    }
    fn auipc(&mut self, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_u_type(instr);
        self.set_x(rd, imm.wrapping_add(self.pc));
        Ok(())
    }
    fn jal(&mut self, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let imm = get_imm_j_type(instr);
        let t = self.pc.wrapping_add(imm);
        //if t & 0b11 == 0 {
        self.set_x(rd, self.pc.wrapping_add(4));
        self.pc = t.wrapping_sub(4);
        Ok(())
        //} else {
        //    Err(Trap {
        //        trap_type: crate::traps::TrapType::InstructionAddressMisaligned,
        //        value: self.pc,
        //    })
        //}
    }
    fn jalr(&mut self, instr: u32) -> Result<(), Trap> {
        let rd = get_rd(instr);
        let rs1 = get_rs1(instr);
        let imm = get_imm_i_type(instr);
        let t = (self.get_x(rs1).wrapping_add(imm)) & !1;
        //if t & 0b11 == 0 {
        self.set_x(rd, self.pc + 4);
        self.pc = t.wrapping_sub(4);
        Ok(())
        //} else {
        //    Err(Trap {
        //        trap_type: crate::traps::TrapType::InstructionAddressMisaligned,
        //        value: self.pc,
        //    })
        //}
    }
    fn beq(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if self.get_x(rs1) == self.get_x(rs2) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bne(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if self.get_x(rs1) != self.get_x(rs2) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn blt(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if (self.get_x(rs1) as i32) < (self.get_x(rs2) as i32) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bge(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if (self.get_x(rs1) as i32) >= (self.get_x(rs2) as i32) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bltu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if self.get_x(rs1) < self.get_x(rs2) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn bgeu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_b_type(instr);
        if self.get_x(rs1) >= self.get_x(rs2) {
            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
        }
        Ok(())
    }
    fn lb(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        let res = self.mmu.read_byte(address)? as i8 as i32 as u32;
        self.set_x(rd, res);
        Ok(())
    }
    fn lh(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        let res = self.mmu.read_halfword(address)? as i16 as i32 as u32;
        self.set_x(rd, res);
        Ok(())
    }
    fn lw(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        let res = self.mmu.read_word(address)?;
        self.set_x(rd, res);
        Ok(())
    }
    fn lbu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        let res = self.mmu.read_byte(address)? as u32;
        self.set_x(rd, res);
        Ok(())
    }
    fn lhu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        let res = self.mmu.read_halfword(address)? as u32;
        self.set_x(rd, res);
        Ok(())
    }
    fn sb(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        self.mmu.write_byte(address, self.get_x(rs2) as _)
    }
    fn sh(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        self.mmu.write_halfword(address, self.get_x(rs2) as _)
    }
    fn sw(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let imm = get_imm_s_type(instr);
        let address = self.get_x(rs1).wrapping_add(imm);
        self.mmu.write_word(address, self.get_x(rs2) as _)
    }
    fn addi(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(rd, self.get_x(rs1).wrapping_add(imm));
        Ok(())
    }
    fn slti(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(
            rd,
            if (self.get_x(rs1) as i32) < (imm as i32) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn sltiu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(rd, if self.get_x(rs1) < imm { 1 } else { 0 });
        Ok(())
    }
    fn xori(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(rd, self.get_x(rs1) ^ imm);
        Ok(())
    }
    fn ori(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(rd, self.get_x(rs1) | imm);
        Ok(())
    }
    fn andi(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        self.set_x(rd, self.get_x(rs1) & imm);
        Ok(())
    }
    fn slli(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        self.set_x(rd, self.get_x(rs1).wrapping_shl(shamt));
        Ok(())
    }
    fn srli(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        self.set_x(rd, self.get_x(rs1).wrapping_shr(shamt));
        Ok(())
    }
    fn srai(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        let imm = get_imm_i_type(instr);
        let shamt = imm & 0b11111;
        self.set_x(rd, (self.get_x(rs1) as i32).wrapping_shr(shamt) as u32);
        Ok(())
    }
    fn add(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1).wrapping_add(self.get_x(rs2)));
        Ok(())
    }
    fn sub(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1).wrapping_sub(self.get_x(rs2)));
        Ok(())
    }

    fn slt(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(
            rd,
            if (self.get_x(rs1) as i32) < (self.get_x(rs2) as i32) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn sltu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(
            rd,
            if self.get_x(rs1) < self.get_x(rs2) {
                1
            } else {
                0
            },
        );
        Ok(())
    }
    fn xor(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1) ^ self.get_x(rs2));
        Ok(())
    }
    fn sll(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1).wrapping_shl(self.get_x(rs2) & 0b11111));
        Ok(())
    }
    fn srl(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1).wrapping_shr(self.get_x(rs2) & 0b11111));
        Ok(())
    }
    fn sra(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(
            rd,
            (self.get_x(rs1) as i32).wrapping_shr(self.get_x(rs2) & 0b11111) as u32,
        );
        Ok(())
    }
    fn or(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1) | self.get_x(rs2));
        Ok(())
    }
    fn and(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        self.set_x(rd, self.get_x(rs1) & self.get_x(rs2));
        Ok(())
    }
    fn fence(&mut self, instr: u32) -> Result<(), Trap> {
        Ok(())
    }
    fn fence_i(&mut self, instr: u32) -> Result<(), Trap> {
        Ok(())
    }
    fn ecall(&mut self, instr: u32) -> Result<(), Trap> {
        let exception_type = TrapType::EnvironmentCallFromMMode; //TODO: Expand when adding privileges
        return Err(Trap {
            tcause: exception_type,
            tval: 0,
        });
    }
    fn ebreak(&mut self, instr: u32) -> Result<(), Trap> {
        let exception_type = TrapType::Breakpoint;
        return Err(Trap {
            tcause: exception_type,
            tval: self.pc,
        });
    }

    fn get_csr(&self, csr: u16) -> Result<u32, Trap> {
        //TODO: privileges check
        Ok(match csr {
            0x340 => self.mscratch,
            0x305 => self.mtvec,
            0x304 => self.mie,

            0x344 => self.mip,
            0x341 => self.mepc,
            0x342 => self.mcause,
            0x343 => self.mtval,
            0xf11 => 0xff0ff0ff,                              //vendorId
            0xf12 => 0x0,                                     //marchid
            0xf13 => 0x0,                                     //mimpid
            0xf14 => 0x0,                                     //mhartid
            0xf15 => 0x0,                                     //mconfigptr
            0x301 => 0b01_0_10000_00000000_00010001_00000101, //misa, XLEN=32, IMAC

            0x300 => self.mstatus as u32,         //mstatus
            0x310 => (self.mstatus >> 32) as u32, //mstatus

            0xC00 => self.cycle as u32,
            0xC80 => (self.cycle >> 32) as u32,

            0xC01 => self.timer_reader() as u32,         //time
            0xC81 => (self.timer_reader() >> 32) as u32, //timeh
            _ => todo!("CSR {csr:0x} not implemented"),
        })
    }
    //Is register writable
    fn set_csr(&mut self, csr: u16, new_val: u32) -> Result<bool, Trap> {
        //TODO: privileges check
        match csr {
            0x340 => self.mscratch = new_val,
            0x305 => self.mtvec = new_val,
            0x304 => self.mie = new_val,
            0xC00 => {
                return Ok(false);
            } //cyclel
            0xC80 => {
                return Ok(false);
            } //cycleh
            0x344 => self.mip = new_val,
            0x341 => self.mepc = new_val,
            0x300 => {
                self.mstatus &= !(u32::MAX as u64);
                self.mstatus |= new_val as u64
            } //mstatus
            0x310 => {
                self.mstatus &= u32::MAX as u64;
                self.mstatus |= (new_val as u64) << 32
            }
            0x342 => self.mcause = new_val,
            0x343 => self.mtval = new_val,
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
    fn sret(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("SRET is not implemented")
    }
    fn mret(&mut self, instr: u32) -> Result<(), Trap> {
        //TODO Do csr register shenenigans when implement multiple levels of priveleges.
        self.pc = self.mepc.wrapping_sub(4);
        Ok(())
    }
    fn wfi(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("WFI is not implemented")
    }
    fn csrrw(&mut self, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rd != 0 {
            let val = self.get_csr(csr)?;
            if !self.set_csr(csr, self.get_x(rs1))? {
                todo!("Handle write in read-only registers")
            }
            self.set_x(rd, val)
        } else {
            if !self.set_csr(csr, self.get_x(rs1))? {
                todo!("Handle write in read-only registers")
            }
        }
        Ok(())
    }
    fn csrrs(&mut self, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rs1 != 0 {
            let val = self.get_csr(csr)?;
            let set_bits = val | self.get_x(rs1);
            if !self.set_csr(csr, set_bits)? {
                todo!("Handle read in write-only registers")
            }
            self.set_x(rd, val)
        } else {
            let val = self.get_csr(csr)?;
            self.set_x(rd, val)
        }
        Ok(())
    }
    fn csrrc(&mut self, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rs1 != 0 {
            let val = self.get_csr(csr)?;
            let set_bits = val & !self.get_x(rs1);
            if !self.set_csr(csr, set_bits)? {
                todo!("Handle read in write-only registers")
            }
            self.set_x(rd, val)
        } else {
            let val = self.get_csr(csr)?;
            self.set_x(rd, val)
        }
        Ok(())
    }
    fn csrrwi(&mut self, instr: u32) -> Result<(), Trap> {
        let csr = get_csr_num(instr);
        let rs1 = get_rs1(instr);
        let rd = get_rd(instr);
        if rd != 0 {
            let val = self.get_csr(csr)?;
            if !self.set_csr(csr, rs1 as u32)? {
                todo!("Handle write in read-only registers")
            }
            self.set_x(rd, val)
        } else {
            if !self.set_csr(csr, rs1 as u32)? {
                todo!("Handle write in read-only registers")
            }
        }
        Ok(())  
    }
    fn csrrsi(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("CSRRSI is not implemented")
    }
    fn csrrci(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("CSRRCI is not implemented")
        // Ok(())
    }
    fn mul(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (self.get_x(rs1) as i32).wrapping_mul(self.get_x(rs2) as i32);
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn mulh(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (self.get_x(rs1) as i32 as i64).wrapping_mul(self.get_x(rs2) as i32 as i64);
        let m = (m as u64) >> 32;
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn mulhsu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (self.get_x(rs1) as i32 as i64).wrapping_mul(self.get_x(rs2) as u64 as i64);
        let m = m >> 32;
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn mulhu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let m = (self.get_x(rs1) as u64).wrapping_mul(self.get_x(rs2) as u64);
        let m = m >> 32;
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn div(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = self.get_x(rs1);
        let rs2 = self.get_x(rs2);

        let m = if rs2 != 0 {
            (rs1 as i32).wrapping_div(rs2 as i32)
        } else {
            -1
        };
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn divu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = self.get_x(rs1);
        let rs2 = self.get_x(rs2);

        let m = if rs2 != 0 {
            rs1.wrapping_div(rs2)
        } else {
            u32::MAX
        };
        self.set_x(rd, m);
        Ok(())
    }
    fn rem(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = self.get_x(rs1);
        let rs2 = self.get_x(rs2);

        let m = if rs2 != 0 {
            (rs1 as i32).wrapping_rem(rs2 as i32)
        } else {
            rs1 as i32
        };
        self.set_x(rd, m as u32);
        Ok(())
    }
    fn remu(&mut self, instr: u32) -> Result<(), Trap> {
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        let rd = get_rd(instr);
        let rs1 = self.get_x(rs1);
        let rs2 = self.get_x(rs2);

        let m = if rs2 != 0 {
            rs1.wrapping_rem(rs2)
        } else {
            rs1
        };
        self.set_x(rd, m);
        Ok(())
    }
    fn lr_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("LR.W is not implemented")
    }
    fn sc_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("SC.W is not implemented")
    }
    fn amoswap_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOSWAP.W is not implemented")
    }
    fn amoadd_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOADD.W is not implemented")
    }
    fn amoxor_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOXOR.W is not implemented")
    }
    fn amoand_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOAND.W is not implemented")
    }
    fn amoor_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOOR.W is not implemented")
    }
    fn amomin_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOMIN.W is not implemented")
    }
    fn amomax_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOMAX.W is not implemented")
    }
    fn amominu_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOMINU.W is not implemented")
    }
    fn amomaxu_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!("AMOMAXU.W is not implemented")
    }

    fn c_addi4spn(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.ADDI4SPN is not implemented")
    }
    fn c_lw(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.LW is not implemented")
    }
    fn c_sw(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SW is not implemented")
    }
    fn c_nop(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.NOP is not implemented")
    }
    fn c_addi(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.ADDI is not implemented")
    }
    fn c_jal(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.JAL is not implemented")
    }
    fn c_li(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.LI is not implemented")
    }
    fn c_addi16sp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.ADDI16SP is not implemented")
    }
    fn c_lui(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.LUI is not implemented")
    }
    fn c_srli(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SRLI is not implemented")
    }
    fn c_srai(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SRAI is not implemented")
    }
    fn c_andi(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.ANDI is not implemented")
    }
    fn c_sub(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SUB is not implemented")
    }
    fn c_xor(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.XOR is not implemented")
    }
    fn c_or(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.OR is not implemented")
    }
    fn c_and(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.AND is not implemented")
    }
    fn c_j(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.J is not implemented")
    }
    fn c_beqz(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.BEQZ is not implemented")
    }
    fn c_bnez(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.BNEZ is not implemented")
    }
    fn c_slli(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SLLI is not implemented")
    }
    fn c_lwsp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.LWSP is not implemented")
    }
    fn c_jr(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.JR is not implemented")
    }
    fn c_mv(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.MV is not implemented")
    }
    fn c_ebreak(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.EBREAK is not implemented")
    }
    fn c_jalr(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.JALR is not implemented")
    }
    fn c_add(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.ADD is not implemented")
    }
    fn c_swsp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!("C.SWSP is not implemented")
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
        mmu::{MMU, RAM_ADDRESS, RAM_ADDRESS_END},
        ops_decode::encode_r_type,
    };

    //Builds a CPU with `bytes` placed at `address` and the pc pointing at them.
    fn cpu_with_bytes_at(address: u32, bytes: &[u8]) -> CPU {
        let (mut mmu, _audio) = MMU::new();
        for (offset, byte) in bytes.iter().enumerate() {
            assert!(mmu.write_raw_to_ram(address + offset as u32, *byte));
        }
        let mut cpu = CPU::new(mmu);
        cpu.pc = address;
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
        let mut regs = cpu.get_registers();
        regs[6] = 0xFFFF_FFF9; //-7
        regs[7] = 3;
        cpu.set_registers(regs);

        cpu.execute_instruction().unwrap();

        //0xFFFF_FFF9 % 3 == 0 unsigned, while -7 % 3 == -1 signed
        assert_eq!(cpu.get_registers()[5], 0);
        assert_eq!(cpu.pc, RAM_ADDRESS + 4);
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
            assert_eq!(cpu.pc, RAM_ADDRESS);
        }
    }

    //C.ANDI carries imm[5] in bit 12, so the negative immediates must not be mistaken for
    //reserved encodings.
    #[test]
    #[should_panic(expected = "C.ANDI is not implemented")]
    fn c_andi_accepts_negative_immediates() {
        //c.andi a0,-1
        let mut cpu = cpu_with_instruction(&0x997du16.to_le_bytes());
        let _ = cpu.execute_instruction();
    }
}
