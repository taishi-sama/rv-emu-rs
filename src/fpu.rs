//! F and D extension support.
//! 
//! IEEE 754 is evil and RISC-V's floating point specification is especially Extra Evil:tm:, so this part is mostly written by LLM and gated behind explicit feature flag.  
//! As it maps to Berkey's soft-float practically 1 to 1, is practically lacking any non-trivial logic except CSR-juggling.
//! 
//! Floating-point registers are kept in their architectural 64-bit storage. Single-precision
//! values are NaN-boxed by setting the upper 32 bits to ones; reading a register that is not
//! properly boxed yields the canonical NaN, as required by the F extension.

use softfloat_pure::softfloat::{defaultNaNF32UI, softfloat_flag_invalid};
use softfloat_pure::{float32_t, float64_t, ExceptionFlags, Float, RoundingMode, FPU};

use crate::{
    cpu::{CPU, MSTATUS_FS},
    ops_decode::{
        get_compressed_c_fld_fsd_offset, get_compressed_c_fldsp_offset,
        get_compressed_c_flw_fsw_offset, get_compressed_c_flwsp_offset,
        get_compressed_c_fsdsp_offset, get_compressed_c_fswsp_offset, get_compressed_func3,
        get_compressed_rd, get_compressed_rdc, get_compressed_rs1c, get_compressed_rs2, get_funct3,
        get_funct7, get_imm_i_type, get_imm_s_type, get_opcode, get_rd, get_rs1, get_rs2, get_rs3,
    },
    traps::{Trap, TrapType},
};

const FFLAGS_MASK: u32 = 0b1_1111;
const FRM_SHIFT: u32 = 5;
const FRM_MASK: u32 = 0b111 << FRM_SHIFT;

//The IEEE 754 formats implemented by the D and F extensions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fmt {
    Single,
    Double,
}

impl Fmt {
    fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Single),
            1 => Some(Self::Double),
            _ => None,
        }
    }

    fn read(self, state: &FpuState, register: u8) -> u64 {
        match self {
            Self::Single => state.f32(register).to_bits() as u64,
            Self::Double => state.f64(register).to_bits(),
        }
    }

    fn write(self, state: &mut FpuState, register: u8, bits: u64) {
        match self {
            Self::Single => state.set_f32(register, float32_t::from_bits(bits as u32)),
            Self::Double => state.set_f64(register, float64_t::from_bits(bits)),
        }
    }

    fn sign_mask(self) -> u64 {
        match self {
            Self::Single => 1 << 31,
            Self::Double => 1 << 63,
        }
    }
}

//The architectural floating-point register file and control/status register.
#[derive(Clone, Copy, Default)]
pub struct FpuState {
    registers: [u64; 32],
    fcsr: u32,
}

impl FpuState {
    //F registers hold single-precision values NaN-boxed in the upper half. A value that is not
    //boxed is interpreted as the canonical NaN instead of its raw bits.
    fn f32(&self, register: u8) -> float32_t {
        let bits = self.registers[register as usize];
        if bits >> 32 == u32::MAX as u64 {
            float32_t::from_bits(bits as u32)
        } else {
            float32_t::from_bits(defaultNaNF32UI)
        }
    }

    fn f64(&self, register: u8) -> float64_t {
        float64_t::from_bits(self.registers[register as usize])
    }

    fn set_f32(&mut self, register: u8, value: float32_t) {
        self.registers[register as usize] = 0xffff_ffff_0000_0000 | value.to_bits() as u64;
    }

    fn set_f64(&mut self, register: u8, value: float64_t) {
        self.registers[register as usize] = value.to_bits();
    }

    //FSW and FMV.X.W look at the low 32 bits without applying the NaN-boxing rule.
    fn f32_raw(&self, register: u8) -> u32 {
        self.registers[register as usize] as u32
    }

    pub(crate) fn frm(&self) -> u8 {
        ((self.fcsr & FRM_MASK) >> FRM_SHIFT) as u8
    }

    pub(crate) fn fcsr(&self) -> u32 {
        self.fcsr & 0xff
    }

    pub(crate) fn set_fflags(&mut self, value: u32) {
        self.fcsr = (self.fcsr & !FFLAGS_MASK) | (value & FFLAGS_MASK);
    }

    pub(crate) fn set_frm(&mut self, value: u32) {
        self.fcsr = (self.fcsr & !FRM_MASK) | ((value & 0b111) << FRM_SHIFT);
    }

    pub(crate) fn set_fcsr(&mut self, value: u32) {
        self.fcsr = value & 0xff;
    }

    fn merge_flags(&mut self, flags: ExceptionFlags) {
        self.fcsr |= flags.to_bits() as u32 & FFLAGS_MASK;
    }
}

#[derive(Clone, Copy)]
enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
}

fn illegal(instr: u32) -> Trap {
    Trap {
        tcause: TrapType::IllegalInstruction,
        tval: instr,
    }
}

impl CPU {
    fn new_fpu() -> FPU {
        //FPU::default() uses RISC-V's after-rounding tininess detection.
        FPU::default()
    }

    fn fp_rounding(&self, rm: u8, instr: u32) -> Result<RoundingMode, Trap> {
        let rm = if rm == 0b111 {
            self.state.fpu.frm()
        } else {
            rm
        };
        match rm {
            0 => Ok(RoundingMode::RneTiesToEven),
            1 => Ok(RoundingMode::RtzTowardZero),
            2 => Ok(RoundingMode::RdnTowardNegative),
            3 => Ok(RoundingMode::RupTowardPositive),
            4 => Ok(RoundingMode::RmmTiesToAway),
            _ => Err(illegal(instr)),
        }
    }

    fn fp_enabled(&self, instr: u32) -> Result<(), Trap> {
        if self.state.mstatus as u32 & MSTATUS_FS == 0 {
            Err(illegal(instr))
        } else {
            Ok(())
        }
    }

    pub(crate) fn mark_fp_dirty(&mut self) {
        let status = self.state.mstatus as u32 | MSTATUS_FS;
        self.state.mstatus = (self.state.mstatus & 0xffff_ffff_0000_0000) | status as u64;
    }

    pub(crate) fn execute_fp(&mut self, instr: u32) -> Result<(), Trap> {
        self.fp_enabled(instr)?;
        self.dispatch_fp(instr)?;
        self.mark_fp_dirty();
        Ok(())
    }

    fn dispatch_fp(&mut self, instr: u32) -> Result<(), Trap> {
        let funct7 = get_funct7(instr);
        let funct3 = get_funct3(instr);
        let rd = get_rd(instr);
        let rs1 = get_rs1(instr);
        let rs2 = get_rs2(instr);
        match funct7 {
            0x00 | 0x01 => self.fp_arith(
                Fmt::from_code(funct7 & 1).unwrap(),
                instr,
                rd,
                rs1,
                rs2,
                ArithmeticOp::Add,
            ),
            0x04 | 0x05 => self.fp_arith(
                Fmt::from_code(funct7 & 1).unwrap(),
                instr,
                rd,
                rs1,
                rs2,
                ArithmeticOp::Sub,
            ),
            0x08 | 0x09 => self.fp_arith(
                Fmt::from_code(funct7 & 1).unwrap(),
                instr,
                rd,
                rs1,
                rs2,
                ArithmeticOp::Mul,
            ),
            0x0c | 0x0d => self.fp_arith(
                Fmt::from_code(funct7 & 1).unwrap(),
                instr,
                rd,
                rs1,
                rs2,
                ArithmeticOp::Div,
            ),
            0x10 | 0x11 => self.fp_sign_inject(
                Fmt::from_code(funct7 & 1).unwrap(),
                funct3,
                rd,
                rs1,
                rs2,
                instr,
            ),
            0x14 | 0x15 => self.fp_minmax(
                Fmt::from_code(funct7 & 1).unwrap(),
                funct3,
                rd,
                rs1,
                rs2,
                instr,
            ),
            0x20 => {
                if rs2 != 1 {
                    return Err(illegal(instr));
                }
                self.fp_cvt_s_d(instr, rd, rs1)
            }
            0x21 => {
                if rs2 != 0 {
                    return Err(illegal(instr));
                }
                self.fp_cvt_d_s(instr, rd, rs1)
            }
            0x2c | 0x2d => {
                if rs2 != 0 {
                    return Err(illegal(instr));
                }
                self.fp_sqrt(Fmt::from_code(funct7 & 1).unwrap(), instr, rd, rs1)
            }
            0x50 | 0x51 => self.fp_compare(
                Fmt::from_code(funct7 & 1).unwrap(),
                funct3,
                rd,
                rs1,
                rs2,
                instr,
            ),
            0x60 | 0x61 => self.fp_to_int(Fmt::from_code(funct7 & 1).unwrap(), rs2, rd, rs1, instr),
            0x68 | 0x69 => {
                self.fp_from_int(Fmt::from_code(funct7 & 1).unwrap(), rs2, rd, rs1, instr)
            }
            0x70 | 0x71 => self.fp_mv_or_class(
                Fmt::from_code(funct7 & 1).unwrap(),
                funct3,
                rs2,
                rd,
                rs1,
                instr,
            ),
            0x78 => {
                if funct3 != 0 || rs2 != 0 {
                    return Err(illegal(instr));
                }
                let value = self.state.get_x(rs1);
                self.state.fpu.set_f32(rd, float32_t::from_bits(value));
                Ok(())
            }
            //FMV.D.X is RV64 only.
            0x79 => Err(illegal(instr)),
            _ => Err(illegal(instr)),
        }
    }

    fn fp_arith(
        &mut self,
        fmt: Fmt,
        instr: u32,
        rd: u8,
        rs1: u8,
        rs2: u8,
        op: ArithmeticOp,
    ) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let a = fmt.read(&self.state.fpu, rs1);
        let b = fmt.read(&self.state.fpu, rs2);
        let mut fpu = Self::new_fpu();
        let result = match fmt {
            Fmt::Single => {
                let a = float32_t::from_bits(a as u32);
                let b = float32_t::from_bits(b as u32);
                match op {
                    ArithmeticOp::Add => fpu.add::<float32_t, _>(a, b, rounding),
                    ArithmeticOp::Sub => fpu.sub::<float32_t, _>(a, b, rounding),
                    ArithmeticOp::Mul => fpu.mul::<float32_t, _>(a, b, rounding),
                    ArithmeticOp::Div => fpu.div::<float32_t, _>(a, b, rounding),
                }
                .to_bits() as u64
            }
            Fmt::Double => {
                let a = float64_t::from_bits(a);
                let b = float64_t::from_bits(b);
                match op {
                    ArithmeticOp::Add => fpu.add::<float64_t, _>(a, b, rounding),
                    ArithmeticOp::Sub => fpu.sub::<float64_t, _>(a, b, rounding),
                    ArithmeticOp::Mul => fpu.mul::<float64_t, _>(a, b, rounding),
                    ArithmeticOp::Div => fpu.div::<float64_t, _>(a, b, rounding),
                }
                .to_bits()
            }
        };
        self.state.fpu.merge_flags(fpu.flags);
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    fn fp_sqrt(&mut self, fmt: Fmt, instr: u32, rd: u8, rs1: u8) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let a = fmt.read(&self.state.fpu, rs1);
        let mut fpu = Self::new_fpu();
        let result = match fmt {
            Fmt::Single => fpu
                .sqrt::<float32_t, _>(float32_t::from_bits(a as u32), rounding)
                .to_bits() as u64,
            Fmt::Double => fpu
                .sqrt::<float64_t, _>(float64_t::from_bits(a), rounding)
                .to_bits(),
        };
        self.state.fpu.merge_flags(fpu.flags);
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    fn fp_sign_inject(
        &mut self,
        fmt: Fmt,
        funct3: u8,
        rd: u8,
        rs1: u8,
        rs2: u8,
        instr: u32,
    ) -> Result<(), Trap> {
        if funct3 > 2 {
            return Err(illegal(instr));
        }
        let a = fmt.read(&self.state.fpu, rs1);
        let b = fmt.read(&self.state.fpu, rs2);
        let mask = fmt.sign_mask();
        let sign = match funct3 {
            0 => b & mask,
            1 => !b & mask,
            2 => (a ^ b) & mask,
            _ => unreachable!(),
        };
        let result = (a & !mask) | sign;
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    fn fp_minmax(
        &mut self,
        fmt: Fmt,
        funct3: u8,
        rd: u8,
        rs1: u8,
        rs2: u8,
        instr: u32,
    ) -> Result<(), Trap> {
        if funct3 > 1 {
            return Err(illegal(instr));
        }
        let is_max = funct3 == 1;
        let a = fmt.read(&self.state.fpu, rs1);
        let b = fmt.read(&self.state.fpu, rs2);
        let mut fpu = Self::new_fpu();
        let result = match fmt {
            Fmt::Single => {
                let a = float32_t::from_bits(a as u32);
                let b = float32_t::from_bits(b as u32);
                let both_nan = a.is_nan() && b.is_nan();
                let signaling = a.is_signaling_nan() || b.is_signaling_nan();
                let value = if is_max { fpu.max(a, b) } else { fpu.min(a, b) };
                if both_nan && signaling {
                    fpu.flags.merge(softfloat_flag_invalid);
                }
                value.to_bits() as u64
            }
            Fmt::Double => {
                let a = float64_t::from_bits(a);
                let b = float64_t::from_bits(b);
                let both_nan = a.is_nan() && b.is_nan();
                let signaling = a.is_signaling_nan() || b.is_signaling_nan();
                let value = if is_max { fpu.max(a, b) } else { fpu.min(a, b) };
                if both_nan && signaling {
                    fpu.flags.merge(softfloat_flag_invalid);
                }
                value.to_bits()
            }
        };
        self.state.fpu.merge_flags(fpu.flags);
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    fn fp_compare(
        &mut self,
        fmt: Fmt,
        funct3: u8,
        rd: u8,
        rs1: u8,
        rs2: u8,
        instr: u32,
    ) -> Result<(), Trap> {
        if funct3 > 2 {
            return Err(illegal(instr));
        }
        let a = fmt.read(&self.state.fpu, rs1);
        let b = fmt.read(&self.state.fpu, rs2);
        let mut fpu = Self::new_fpu();
        let result = match fmt {
            Fmt::Single => {
                let a = float32_t::from_bits(a as u32);
                let b = float32_t::from_bits(b as u32);
                match funct3 {
                    0 => fpu.le::<float32_t, _>(a, b),
                    1 => fpu.lt::<float32_t, _>(a, b),
                    _ => fpu.eq::<float32_t, _>(a, b),
                }
            }
            Fmt::Double => {
                let a = float64_t::from_bits(a);
                let b = float64_t::from_bits(b);
                match funct3 {
                    0 => fpu.le::<float64_t, _>(a, b),
                    1 => fpu.lt::<float64_t, _>(a, b),
                    _ => fpu.eq::<float64_t, _>(a, b),
                }
            }
        };
        self.state.fpu.merge_flags(fpu.flags);
        self.state.set_x(rd, result as u32);
        Ok(())
    }

    fn fp_cvt_s_d(&mut self, instr: u32, rd: u8, rs1: u8) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let a = self.state.fpu.f64(rs1);
        let mut fpu = Self::new_fpu();
        let result = fpu.to_f32::<float64_t, _>(a, rounding);
        self.state.fpu.merge_flags(fpu.flags);
        self.state.fpu.set_f32(rd, result);
        Ok(())
    }

    fn fp_cvt_d_s(&mut self, instr: u32, rd: u8, rs1: u8) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let a = self.state.fpu.f32(rs1);
        let mut fpu = Self::new_fpu();
        let result = fpu.to_f64::<float32_t, _>(a, rounding);
        self.state.fpu.merge_flags(fpu.flags);
        self.state.fpu.set_f64(rd, result);
        Ok(())
    }

    fn fp_to_int(&mut self, fmt: Fmt, rs2: u8, rd: u8, rs1: u8, instr: u32) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let a = fmt.read(&self.state.fpu, rs1);
        let mut fpu = Self::new_fpu();
        //L and LU conversions are RV64 only.
        let result = match (fmt, rs2) {
            (Fmt::Single, 0) => {
                fpu.to_i32::<float32_t, _>(float32_t::from_bits(a as u32), rounding, true) as u32
            }
            (Fmt::Single, 1) => {
                fpu.to_u32::<float32_t, _>(float32_t::from_bits(a as u32), rounding, true)
            }
            (Fmt::Double, 0) => {
                fpu.to_i32::<float64_t, _>(float64_t::from_bits(a), rounding, true) as u32
            }
            (Fmt::Double, 1) => fpu.to_u32::<float64_t, _>(float64_t::from_bits(a), rounding, true),
            _ => return Err(illegal(instr)),
        };
        self.state.fpu.merge_flags(fpu.flags);
        self.state.set_x(rd, result);
        Ok(())
    }

    fn fp_from_int(&mut self, fmt: Fmt, rs2: u8, rd: u8, rs1: u8, instr: u32) -> Result<(), Trap> {
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let value = self.state.get_x(rs1);
        let mut fpu = Self::new_fpu();
        //L and LU conversions are RV64 only.
        let result = match (fmt, rs2) {
            (Fmt::Single, 0) => fpu.f32_from_i32(value as i32, rounding).to_bits() as u64,
            (Fmt::Single, 1) => fpu.f32_from_u32(value, rounding).to_bits() as u64,
            (Fmt::Double, 0) => fpu.f64_from_i32(value as i32).to_bits(),
            (Fmt::Double, 1) => fpu.f64_from_u32(value).to_bits(),
            _ => return Err(illegal(instr)),
        };
        self.state.fpu.merge_flags(fpu.flags);
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    fn fp_mv_or_class(
        &mut self,
        fmt: Fmt,
        funct3: u8,
        rs2: u8,
        rd: u8,
        rs1: u8,
        instr: u32,
    ) -> Result<(), Trap> {
        if rs2 != 0 {
            return Err(illegal(instr));
        }
        match funct3 {
            0 => {
                //FMV.X.D is RV64 only.
                if fmt != Fmt::Single {
                    return Err(illegal(instr));
                }
                let bits = self.state.fpu.f32_raw(rs1);
                self.state.set_x(rd, bits);
                Ok(())
            }
            1 => {
                let value = fmt.read(&self.state.fpu, rs1);
                let class = match fmt {
                    Fmt::Single => classify_f32(value as u32),
                    Fmt::Double => classify_f64(value),
                };
                self.state.set_x(rd, class);
                Ok(())
            }
            _ => Err(illegal(instr)),
        }
    }

    fn fp_fma(&mut self, instr: u32) -> Result<(), Trap> {
        let funct7 = get_funct7(instr);
        let fmt = Fmt::from_code(funct7 & 0b11).ok_or_else(|| illegal(instr))?;
        //R4-type instructions encode the operation in the opcode; funct7's high bits are rs3.
        let (negate_a, negate_c) = match get_opcode(instr) {
            0b1000011 => (false, false), //FMADD
            0b1000111 => (false, true),  //FMSUB
            0b1001011 => (true, false),  //FNMSUB
            0b1001111 => (true, true),   //FNMADD
            _ => return Err(illegal(instr)),
        };
        let rounding = self.fp_rounding(get_funct3(instr), instr)?;
        let rd = get_rd(instr);
        let a = fmt.read(&self.state.fpu, get_rs1(instr));
        let b = fmt.read(&self.state.fpu, get_rs2(instr));
        let c = fmt.read(&self.state.fpu, get_rs3(instr));
        let mut fpu = Self::new_fpu();
        let result = match fmt {
            Fmt::Single => {
                let a = float32_t::from_bits(a as u32);
                let b = float32_t::from_bits(b as u32);
                let c = float32_t::from_bits(c as u32);
                let a = if negate_a { a.neg() } else { a };
                let c = if negate_c { c.neg() } else { c };
                fpu.mul_add::<float32_t, _>(a, b, c, rounding).to_bits() as u64
            }
            Fmt::Double => {
                let a = float64_t::from_bits(a);
                let b = float64_t::from_bits(b);
                let c = float64_t::from_bits(c);
                let a = if negate_a { a.neg() } else { a };
                let c = if negate_c { c.neg() } else { c };
                fpu.mul_add::<float64_t, _>(a, b, c, rounding).to_bits()
            }
        };
        self.state.fpu.merge_flags(fpu.flags);
        fmt.write(&mut self.state.fpu, rd, result);
        Ok(())
    }

    pub(crate) fn execute_fma(&mut self, instr: u32) -> Result<(), Trap> {
        self.fp_enabled(instr)?;
        self.fp_fma(instr)?;
        self.mark_fp_dirty();
        Ok(())
    }

    pub(crate) fn execute_fp_load(&mut self, instr: u32) -> Result<(), Trap> {
        self.fp_enabled(instr)?;
        let address = self
            .state
            .get_x(get_rs1(instr))
            .wrapping_add(get_imm_i_type(instr));
        let rd = get_rd(instr);
        match get_funct3(instr) {
            0b010 => {
                let value = self.mmu.read_word(address)?;
                self.state.fpu.set_f32(rd, float32_t::from_bits(value));
            }
            0b011 => {
                let value = self.mmu.read_dword(address)?;
                self.state.fpu.set_f64(rd, float64_t::from_bits(value));
            }
            _ => return Err(illegal(instr)),
        }
        self.mark_fp_dirty();
        Ok(())
    }

    pub(crate) fn execute_fp_store(&mut self, instr: u32) -> Result<(), Trap> {
        self.fp_enabled(instr)?;
        let address = self
            .state
            .get_x(get_rs1(instr))
            .wrapping_add(get_imm_s_type(instr));
        let rs2 = get_rs2(instr);
        match get_funct3(instr) {
            0b010 => self.mmu.write_word(address, self.state.fpu.f32_raw(rs2))?,
            0b011 => self
                .mmu
                .write_dword(address, self.state.fpu.f64(rs2).to_bits())?,
            _ => return Err(illegal(instr)),
        }
        self.mark_fp_dirty();
        Ok(())
    }

    pub(crate) fn execute_compressed_fp(&mut self, instr: u16) -> Result<(), Trap> {
        self.fp_enabled(instr as u32)?;
        self.dispatch_compressed_fp(instr)?;
        self.mark_fp_dirty();
        Ok(())
    }

    fn dispatch_compressed_fp(&mut self, instr: u16) -> Result<(), Trap> {
        let funct3 = get_compressed_func3(instr);
        match instr & 0b11 {
            0b00 => match funct3 {
                0b001 => {
                    let rd = get_compressed_rdc(instr) + 8;
                    let rs1 = get_compressed_rs1c(instr) + 8;
                    let address = self
                        .state
                        .get_x(rs1)
                        .wrapping_add(get_compressed_c_fld_fsd_offset(instr));
                    let value = self.mmu.read_dword(address)?;
                    self.state.fpu.set_f64(rd, float64_t::from_bits(value));
                    Ok(())
                }
                0b011 => {
                    let rd = get_compressed_rdc(instr) + 8;
                    let rs1 = get_compressed_rs1c(instr) + 8;
                    let address = self
                        .state
                        .get_x(rs1)
                        .wrapping_add(get_compressed_c_flw_fsw_offset(instr));
                    let value = self.mmu.read_word(address)?;
                    self.state.fpu.set_f32(rd, float32_t::from_bits(value));
                    Ok(())
                }
                0b101 => {
                    let rs2 = get_compressed_rdc(instr) + 8;
                    let rs1 = get_compressed_rs1c(instr) + 8;
                    let address = self
                        .state
                        .get_x(rs1)
                        .wrapping_add(get_compressed_c_fld_fsd_offset(instr));
                    self.mmu
                        .write_dword(address, self.state.fpu.f64(rs2).to_bits())
                }
                0b111 => {
                    let rs2 = get_compressed_rdc(instr) + 8;
                    let rs1 = get_compressed_rs1c(instr) + 8;
                    let address = self
                        .state
                        .get_x(rs1)
                        .wrapping_add(get_compressed_c_flw_fsw_offset(instr));
                    self.mmu.write_word(address, self.state.fpu.f32_raw(rs2))
                }
                _ => Err(illegal(instr as u32)),
            },
            0b10 => match funct3 {
                0b001 => {
                    let rd = get_compressed_rd(instr);
                    let address = self
                        .state
                        .get_x(2)
                        .wrapping_add(get_compressed_c_fldsp_offset(instr));
                    let value = self.mmu.read_dword(address)?;
                    self.state.fpu.set_f64(rd, float64_t::from_bits(value));
                    Ok(())
                }
                0b011 => {
                    let rd = get_compressed_rd(instr);
                    let address = self
                        .state
                        .get_x(2)
                        .wrapping_add(get_compressed_c_flwsp_offset(instr));
                    let value = self.mmu.read_word(address)?;
                    self.state.fpu.set_f32(rd, float32_t::from_bits(value));
                    Ok(())
                }
                0b101 => {
                    let rs2 = get_compressed_rs2(instr);
                    let address = self
                        .state
                        .get_x(2)
                        .wrapping_add(get_compressed_c_fsdsp_offset(instr));
                    self.mmu
                        .write_dword(address, self.state.fpu.f64(rs2).to_bits())
                }
                0b111 => {
                    let rs2 = get_compressed_rs2(instr);
                    let address = self
                        .state
                        .get_x(2)
                        .wrapping_add(get_compressed_c_fswsp_offset(instr));
                    self.mmu.write_word(address, self.state.fpu.f32_raw(rs2))
                }
                _ => Err(illegal(instr as u32)),
            },
            _ => Err(illegal(instr as u32)),
        }
    }
}

//RISC-V FCLASS result bits.
fn classify_f32(bits: u32) -> u32 {
    let negative = bits >> 31 != 0;
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    if exponent == 0xff {
        if fraction == 0 {
            if negative {
                1 << 0
            } else {
                1 << 7
            }
        } else if fraction & 0x40_0000 == 0 {
            1 << 8
        } else {
            1 << 9
        }
    } else if exponent == 0 {
        if fraction == 0 {
            if negative {
                1 << 3
            } else {
                1 << 4
            }
        } else if negative {
            1 << 2
        } else {
            1 << 5
        }
    } else if negative {
        1 << 1
    } else {
        1 << 6
    }
}

fn classify_f64(bits: u64) -> u32 {
    let negative = bits >> 63 != 0;
    let exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & 0xf_ffff_ffff_ffff;
    if exponent == 0x7ff {
        if fraction == 0 {
            if negative {
                1 << 0
            } else {
                1 << 7
            }
        } else if fraction & 0x8_0000_0000_0000 == 0 {
            1 << 8
        } else {
            1 << 9
        }
    } else if exponent == 0 {
        if fraction == 0 {
            if negative {
                1 << 3
            } else {
                1 << 4
            }
        } else if negative {
            1 << 2
        } else {
            1 << 5
        }
    } else if negative {
        1 << 1
    } else {
        1 << 6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bus::{MemoryBus, RAM_ADDRESS, RAM_ADDRESS_END},
        mmu::MMU,
        ops_decode::{encode_i_type, encode_r_type},
        traps::TrapType,
    };

    fn cpu_with_instruction(instr: u32) -> CPU {
        let (mut bus, _audio) = MemoryBus::new();
        for (offset, byte) in instr.to_le_bytes().iter().enumerate() {
            assert!(bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte));
        }
        let mut cpu = CPU::new(MMU::new(bus));
        cpu.state.pc = RAM_ADDRESS;
        cpu
    }

    fn enable_fp(cpu: &mut CPU) {
        cpu.state.mstatus |= MSTATUS_FS as u64;
    }

    #[test]
    fn fp_instruction_requires_fs() {
        //fadd.s f3, f1, f2
        let mut cpu = cpu_with_instruction(encode_r_type(0b1010011, 3, 0, 1, 2, 0));
        cpu.state.fpu.set_f32(1, float32_t::from_bits(0x3f80_0000));
        cpu.state.fpu.set_f32(2, float32_t::from_bits(0x4000_0000));

        let trap = cpu.execute_instruction().unwrap_err();
        assert_eq!(trap.tcause, TrapType::IllegalInstruction);

        enable_fp(&mut cpu);
        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.fpu.f32(3).to_bits(), 0x4040_0000);
        assert_eq!(cpu.state.mstatus & MSTATUS_FS as u64, MSTATUS_FS as u64);
    }

    #[test]
    fn single_precision_values_are_nan_boxed() {
        let mut cpu = cpu_with_instruction(0);
        cpu.state.fpu.set_f32(1, float32_t::from_bits(0x3f80_0000));
        assert_eq!(cpu.state.fpu.registers[1], 0xffff_ffff_3f80_0000);
        assert_eq!(cpu.state.fpu.f32(1).to_bits(), 0x3f80_0000);

        //A non-boxed register reads as the canonical NaN.
        cpu.state.fpu.registers[1] = 0x0000_0000_3f80_0000;
        assert_eq!(cpu.state.fpu.f32(1).to_bits(), defaultNaNF32UI);
        //The raw view used by FSW and FMV.X.W still sees the low bits.
        assert_eq!(cpu.state.fpu.f32_raw(1), 0x3f80_0000);
    }

    #[test]
    fn dynamic_rounding_and_flags() {
        //fcvt.w.s a3, f1 with rm=dyn
        let mut cpu = cpu_with_instruction(encode_r_type(0b1010011, 3, 0b111, 1, 0, 0x60));
        enable_fp(&mut cpu);
        cpu.state.fpu.set_f32(1, float32_t::from_bits(0x4060_0000)); //3.5
        cpu.state.fpu.set_frm(1); //round toward zero

        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.get_registers()[3], 3);
        //NX is sticky in fflags.
        assert_eq!(cpu.state.fpu.fcsr() & 0x1f, 0x01);
    }

    #[test]
    fn fp_csrs_require_fs_and_dirty_it() {
        //csrrs a0, fcsr, x0
        let read = encode_i_type(0b1110011, 10, 0b010, 0, 0x003);
        let mut cpu = cpu_with_instruction(read);
        let trap = cpu.execute_instruction().unwrap_err();
        assert_eq!(trap.tcause, TrapType::IllegalInstruction);

        enable_fp(&mut cpu);
        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.get_registers()[10], 0);

        //csrrw x0, fcsr, x1
        let write = encode_i_type(0b1110011, 0, 0b001, 1, 0x003);
        let mut cpu = cpu_with_instruction(write);
        enable_fp(&mut cpu);
        let mut regs = cpu.state.get_registers();
        regs[1] = 0x21;
        cpu.state.set_registers(regs);
        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.fpu.fcsr(), 0x21);
        assert_eq!(cpu.state.mstatus & MSTATUS_FS as u64, MSTATUS_FS as u64);
    }

    #[test]
    fn fsd_at_end_of_ram_does_not_partially_store() {
        //fsd f1, 0(x1)
        let fsd = encode_r_type(0b0100111, 0, 0b011, 1, 1, 0);
        let mut cpu = cpu_with_instruction(fsd);
        enable_fp(&mut cpu);
        cpu.state
            .fpu
            .set_f64(1, float64_t::from_bits(0x1122_3344_5566_7788));
        let mut regs = cpu.state.get_registers();
        regs[1] = RAM_ADDRESS_END - 3;
        cpu.state.set_registers(regs);

        let trap = cpu.execute_instruction().unwrap_err();
        assert_eq!(trap.tcause, TrapType::StoreAccessFault);
        for offset in 0..4 {
            assert_eq!(
                cpu.mmu.bus.read_raw_from_ram(RAM_ADDRESS_END - 3 + offset),
                Some(0)
            );
        }
    }

    #[test]
    fn double_precision_fma() {
        //fmadd.d f4, f1, f2, f3
        let funct7 = (3 << 2) | 1;
        let mut cpu = cpu_with_instruction(encode_r_type(0b1000011, 4, 0, 1, 2, funct7));
        enable_fp(&mut cpu);
        cpu.state
            .fpu
            .set_f64(1, float64_t::from_bits(0x4000_0000_0000_0000)); //2.0
        cpu.state
            .fpu
            .set_f64(2, float64_t::from_bits(0x4008_0000_0000_0000)); //3.0
        cpu.state
            .fpu
            .set_f64(3, float64_t::from_bits(0x4010_0000_0000_0000)); //4.0

        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.fpu.f64(4).to_bits(), 0x4024_0000_0000_0000);
    }

    #[test]
    fn double_precision_add() {
        //fadd.d f3, f1, f2
        let mut cpu = cpu_with_instruction(encode_r_type(0b1010011, 3, 0, 1, 2, 0x01));
        enable_fp(&mut cpu);
        cpu.state
            .fpu
            .set_f64(1, float64_t::from_bits(0x3ff8_0000_0000_0000)); //1.5
        cpu.state
            .fpu
            .set_f64(2, float64_t::from_bits(0x4004_0000_0000_0000)); //2.5

        cpu.execute_instruction().unwrap();
        assert_eq!(cpu.state.fpu.f64(3).to_bits(), 0x4010_0000_0000_0000);
    }
}
