use std::sync::{atomic::AtomicBool, Arc};

use crate::{
    errors::EmulatorError, mmu::{MMU, RAM_ADDRESS_END}, ops_decode::{
        get_compressed_func3, get_csr_num, get_funct3, get_funct7, get_imm_b_type, get_imm_i_type, get_imm_j_type, get_imm_s_type, get_imm_u_type, get_opcode, get_rd, get_rs1, get_rs2, get_rs3
    }, traps::{Trap, TrapType}
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
    pub mstatus: u32,
    pub mstatush: u32,
    pub cyclel: u32,
    pub cycleh: u32,
    pub timerl: u32,
    pub timerh: u32,
    pub timermatchl: u32,
    pub timermatchh: u32,

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
        x[2] = RAM_ADDRESS_END - 0x4; // - 0x4000;
        CPU {
            x,
            pc: 0,
            mmu,
            mstatus: 0,
            mstatush: 0,
            cyclel: 0,
            cycleh: 0,
            timerl: 0,
            timerh: 0,
            timermatchl: 0,
            timermatchh: 0,
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
        self.pc += instr_len;
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
                todo!()
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

    fn fetch(&mut self) -> Result<u32, Trap> {
        self.mmu.fetch_word(self.pc)
    }
    fn execute(&mut self, instr: u32) -> Result<u8, Trap> {
        let opcode = get_opcode(instr);
        let micro_opcode = opcode & 0b11;
        if micro_opcode == 0b11 {
            //Обработка обычных инструкций
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
                        1 => self.rem(instr),
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
            let compressed = instr as u16;
            match micro_opcode {
                0b00 => match get_compressed_func3(compressed) {
                    0b000 => match compressed {
                        0 => Err(Trap {
                            tcause: crate::traps::TrapType::IllegalInstruction,
                            tval: instr,
                        }),
                        _ => todo!(),
                    },
                    _ => todo!(),
                },
                0b01 => todo!("0b{compressed:00$b}| opcode {micro_opcode}", 16),
                0b10 => todo!("0b{compressed:00$b}| opcode {micro_opcode}", 16),
                _ => unreachable!(),
            }
        }
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
            tval: self.pc,
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
            0xC00 => self.cyclel,
            0xC80 => self.cycleh,
            0x344 => self.mip,
            0x341 => self.mepc,
            0x300 => self.mstatus, //mstatus
            0x342 => self.mcause,
            0x343 => self.mtval,
            0xf11 => 0xff0ff0ff,                              //vendorId
            0xf12 => 0x0,                                     //marchid
            0xf13 => 0x0,                                     //mimpid
            0xf14 => 0x0,                                     //mhartid
            0xf15 => 0x0,                                     //mconfigptr
            0x301 => 0b01_0_10000_00000000_00010001_00000001, //misa, XLEN=32, IMA+X

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
            0x300 => self.mstatus = new_val, //mstatus
            0x310 => self.mstatush = new_val,
            0x342 => self.mcause = new_val,
            0x343 => self.mtval = new_val,
            0xf11..=0xf15 => {
                return Ok(false);
            } //vendorId
            0x301 => {
                return Ok(false);
            } //misa, XLEN=32, IMA+X
            _ => todo!("CSR 0x{csr:0x} not implemented"),
        };
        Ok(true)
    }
    fn sret(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn mret(&mut self, instr: u32) -> Result<(), Trap> {
        //TODO Do csr register shenenigans when implement multiple levels of priveleges.
        self.pc = self.mepc.wrapping_sub(4);
        Ok(())
    }
    fn wfi(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
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
        todo!()
    }
    fn csrrc(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
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
            if !self.set_csr(csr, self.get_x(rs1))? {
                todo!("Handle write in read-only registers")
            }
        }
        Ok(())  
    }
    fn csrrsi(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn csrrci(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
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
            u32::MAX
        };
        self.set_x(rd, m);
        Ok(())
    }
    fn lr_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn sc_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amoswap_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amoadd_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amoxor_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amoand_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amoor_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amomin_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amomax_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amominu_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }
    fn amomaxu_w(&mut self, instr: u32) -> Result<(), Trap> {
        todo!()
    }

    fn c_addi4spn(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_lw(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_sw(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_nop(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_addi(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_jal(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_li(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_addi16sp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_lui(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_srli(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_srai(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_andi(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_sub(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_xor(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_or(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_and(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_j(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_beqz(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_bnez(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_slli(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_lwsp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_jr(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_mv(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_ebreak(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_jalr(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_add(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
    fn c_swsp(&mut self, instr: u16) -> Result<(), Trap> {
        todo!()
    }
}
