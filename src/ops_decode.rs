//Shift number by $shift bits and trim $mask amount of bits.
macro_rules! shift_and_trim {
    ($u:expr, $shift:expr, $mask:expr) => {
        ($u >> $shift & (u32::MAX >> (32 - $mask))) as _
    };
}
//Shift number by $shift bits and trim $mask amount of bits.
macro_rules! shift_and_trim16 {
    ($u:expr, $shift:expr, $mask:expr) => {
        ($u >> $shift & (u16::MAX >> (16 - $mask))) as _
    };
}
#[inline(always)]
fn shift_and_trim(data: u32, shift: u32, mask: u32) -> u32 {
    debug_assert!(shift <= 32);
    debug_assert!(mask <= 32);
    data >> shift & (u32::MAX >> (32 - mask))
}
#[inline(always)]
fn from_to_get(data: u32, begin: u32, end: u32, start: u32) -> u32 {
    debug_assert!(begin <= end);
    debug_assert!(start <= 32);
    shift_and_trim(data, begin, end - begin + 1) << start
}

#[inline]
fn sign_extend32(data: u32, size: u32) -> u32 {
    debug_assert!(size > 0 && size <= 32);
    (((data << (32 - size)) as i32) >> (32 - size)) as u32
}
#[inline]
fn sign_extend16(data: u16, size: u16) -> u16 {
    debug_assert!(size > 0 && size <= 16);
    (((data << (16 - size)) as i16) >> (16 - size)) as u16
}

#[inline]
pub fn get_opcode(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 0, 7)
}
#[inline]
pub fn get_rd(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 7, 5)
}
#[inline]
pub fn get_funct3(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 12, 3)
}
#[inline]
pub fn get_rs1(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 15, 5)
}
#[inline]
pub fn get_rs2(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 20, 5)
}
#[inline]
pub fn get_funct7(instruction: u32) -> u8 {
    shift_and_trim!(instruction, 25, 7)
}
#[inline]
pub fn get_imm_i_type(instruction: u32) -> u32 {
    sign_extend32(shift_and_trim(instruction, 20, 12), 12)
}
#[inline]
pub fn get_imm_s_type(instruction: u32) -> u32 {
    //TODO do faster
    sign_extend32(
        (get_funct7(instruction) as u32) << 5 | get_rd(instruction) as u32,
        12,
    )
}
#[inline]
pub fn get_imm_b_type(instruction: u32) -> u32 {
    //let instruction = u32::MAX;
    let i_11_8: u32 = from_to_get(instruction, 8, 11, 1);
    let i_30_25: u32 = from_to_get(instruction, 25, 30, 5);
    let i_31: u32 = from_to_get(instruction, 31, 31, 12);
    let i_7: u32 = from_to_get(instruction, 7, 7, 11);
    //println!("{i_11_8:00$b}", 32);
    //println!("{i_30_25:00$b}", 32);
    //println!("{i_31:00$b}", 32);
    //println!("{i_7:00$b}", 32);

    let t = sign_extend32(i_7 | i_11_8 | i_30_25 | i_31, 13);
    //println!("{t:00$b}", 32);
    t
}
#[inline]
pub fn get_imm_u_type(instruction: u32) -> u32 {
    instruction & !0b1111_1111_1111
}
pub fn get_rs3(instruction: u32) -> u8 {
    from_to_get(instruction, 27, 31, 0) as u8
}
pub fn get_csr_num(instruction: u32) -> u16 {
    (instruction >> 20) as u16
}

#[inline]
pub fn get_imm_j_type(instruction: u32) -> u32 {
    let i_24_21: u32 = from_to_get(instruction, 21, 24, 1);
    let i_30_25: u32 = from_to_get(instruction, 25, 30, 5);
    let i_20: u32 = from_to_get(instruction, 20, 20, 11);
    let i_19_12: u32 = from_to_get(instruction, 12, 19, 12);
    let i_31: u32 = from_to_get(instruction, 31, 31, 20);
    sign_extend32(i_24_21 | i_30_25 | i_20 | i_19_12 | i_31, 21)
}

pub fn get_compressed_func3(instruction: u16) -> u16 {
    instruction >> 13
}
pub fn get_compressed_func4(instruction: u16) -> u16 {
    instruction >> 12
}
pub fn get_compressed_func6(instruction: u16) -> u16 {
    instruction >> 10
}
pub fn get_compressed_func2(instruction: u16) -> u16 {
    shift_and_trim16!(instruction, 10, 2)
}
pub fn get_compressed_func(instruction: u16) -> u16 {
    shift_and_trim16!(instruction, 5, 2)
}
// rs1'
pub fn get_compressed_rs1c(instruction: u16) -> u8 {
    shift_and_trim16!(instruction, 7, 3)
}
// rs2'/rd'
pub fn get_compressed_rdc(instruction: u16) -> u8 {
    shift_and_trim16!(instruction, 2, 3)
}
pub fn get_compressed_rs2(instruction: u16) -> u8 {
    shift_and_trim16!(instruction, 2, 5)
}
pub fn get_compressed_rd(instruction: u16) -> u8 {
    shift_and_trim16!(instruction, 7, 5)
}
pub fn get_compressed_cj_jump_imm(instruction: u16) -> u32 {
    let raw_imm: u32 = shift_and_trim16!(instruction, 2, 11);
    let off5: u32 = shift_and_trim!(raw_imm, 0, 1);
    let off3_1: u32 = shift_and_trim!(raw_imm, 1, 3);
    let off7: u32 = shift_and_trim!(raw_imm, 4, 1);
    let off6: u32 = shift_and_trim!(raw_imm, 5, 1);
    let off10: u32 = shift_and_trim!(raw_imm, 6, 1);
    let off9_8: u32 = shift_and_trim!(raw_imm, 7, 2);
    let off4: u32 = shift_and_trim!(raw_imm, 9, 1);
    let off11: u32 = shift_and_trim!(raw_imm, 10, 1);
    sign_extend32(
        off3_1 << 1
            | off4 << 4
            | off5 << 5
            | off6 << 6
            | off7 << 7
            | off9_8 << 8
            | off10 << 10
            | off11 << 11,
        12,
    )
}
//Don't appliable for C.LDSP, C.LQSP, C.FLDSP, C.SDSP, C.SQSP, C.FSDSP
pub fn get_compressed_ci_stack_load_32_imm(instruction: u16) -> u32 {
    let off5: u32 = shift_and_trim16!(instruction, 12, 1);
    let off7_6: u32 = shift_and_trim16!(instruction, 2, 2);
    let off4_2: u32 = shift_and_trim16!(instruction, 4, 3);
    off7_6 << 6 | off5 << 5 | off4_2 << 2
}
//Don't appliable for C.LDSP, C.LQSP, C.FLDSP, C.SDSP, C.SQSP, C.FSDSP
pub fn get_compressed_css_stack_write_32_imm(instruction: u16) -> u32 {
    let off7_6: u32 = shift_and_trim16!(instruction, 6, 2);
    let off5_2: u32 = shift_and_trim16!(instruction, 8, 4);
    off5_2 << 2 | off7_6 << 6
}
pub fn get_compressed_cl_mem_load_32_imm(instruction: u16) -> u32 {
    let off5_3: u32 = shift_and_trim16!(instruction, 10, 3);
    let off6: u32 = shift_and_trim16!(instruction, 2, 2);
    let off2: u32 = shift_and_trim16!(instruction, 4, 3);
    off6 << 6 | off5_3 << 3 | off2 << 2
}
pub fn get_compressed_cs_mem_store_32_imm(instruction: u16) -> u32 {
    let off5_3: u32 = shift_and_trim16!(instruction, 10, 3);
    let off6: u32 = shift_and_trim16!(instruction, 2, 2);
    let off2: u32 = shift_and_trim16!(instruction, 4, 3);
    off6 << 6 | off5_3 << 3 | off2 << 2
}
pub fn get_compressed_cb_shift_imm(instruction: u16) -> u32 {
    let shamt5: u32 = shift_and_trim16!(instruction, 12, 1);
    let shamt4_0: u32 = shift_and_trim16!(instruction, 2, 5);
    shamt5 << 5 | shamt4_0
}
pub fn get_compressed_cb_and_imm(instruction: u16) -> u32 {
    let shamt5: u32 = shift_and_trim16!(instruction, 12, 1);
    let shamt4_0: u32 = shift_and_trim16!(instruction, 2, 5);
    sign_extend32(shamt5 << 5 | shamt4_0, 6)
}
pub fn get_compressed_cb_branch_imm(instruction: u16) -> u32 {
    let off5: u32 = shift_and_trim16!(instruction, 2, 1);
    let off2_1: u32 = shift_and_trim16!(instruction, 3, 2);
    let off7_6: u32 = shift_and_trim16!(instruction, 5, 2);
    let off4_3: u32 = shift_and_trim16!(instruction, 10, 2);
    let off8: u32 = shift_and_trim16!(instruction, 12, 1);

    sign_extend32(
        off8 << 8 | off7_6 << 6 | off5 << 5 | off4_3 << 3 | off2_1 << 1,
        9,
    )
}
pub fn get_compressed_ci_addi16sp_imm(instruction: u16) -> u32 {
    let nzimm9: u32 = shift_and_trim16!(instruction, 12, 1);
    let nzimm5: u32 = shift_and_trim16!(instruction, 2, 1);
    let nzimm8_7: u32 = shift_and_trim16!(instruction, 3, 2);
    let nzimm6: u32 = shift_and_trim16!(instruction, 5, 1);
    let nzimm4: u32 = shift_and_trim16!(instruction, 6, 1);
    sign_extend32(
        nzimm4 << 4 | nzimm5 << 5 | nzimm6 << 6 | nzimm8_7 << 7 | nzimm9 << 9,
        10,
    )
}
pub fn get_compressed_ci_li_addi_imm(instruction: u16) -> u32 {
    let imm5: u32 = shift_and_trim16!(instruction, 12, 1);
    let imm4_0: u32 = shift_and_trim16!(instruction, 2, 5);
    sign_extend32(imm5 << 5 | imm4_0, 6)
}
pub fn get_compressed_ci_lui_imm(instruction: u16) -> u32 {
    let imm17: u32 = shift_and_trim16!(instruction, 12, 1);
    let imm16_12: u32 = shift_and_trim16!(instruction, 2, 5);
    sign_extend32(imm17 << 17 | imm16_12 << 12, 18)
}
pub fn get_compressed_ciw_addi4spn_imm(instruction: u16) -> u32 {
    let nzimm3: u32 = shift_and_trim16!(instruction, 5, 1);
    let nzimm2: u32 = shift_and_trim16!(instruction, 6, 1);
    let nzimm9_6: u32 = shift_and_trim16!(instruction, 7, 4);
    let nzimm5_4: u32 = shift_and_trim16!(instruction, 11, 2);
    nzimm9_6 << 6 | nzimm5_4 << 4 | nzimm3 << 3 | nzimm2 << 2
}
pub fn encode_r_type(opcode: u8, rd: u8, funct3: u8, rs1: u8, rs2: u8, funct7: u8) -> u32 {
    let opcode = opcode as u32;
    let rd = (rd as u32) << 7;
    let funct3 = (funct3 as u32) << 12;
    let rs1 = (rs1 as u32) << 15;
    let rs2 = (rs2 as u32) << 20;
    let funct7 = (funct7 as u32) << 25;
    opcode | rd | funct3 | rs1 | rs2 | funct7
}
pub fn encode_i_type(opcode: u8, rd: u8, funct3: u8, rs1: u8, imm: i16) -> u32 {
    let opcode = opcode as u32;
    let rd = (rd as u32) << 7;
    let funct3 = (funct3 as u32) << 12;
    let rs1 = (rs1 as u32) << 15;
    let imm = ((imm & 0x0FFF) as u32) << 20;
    opcode | rd | funct3 | rs1 | imm
}
pub fn encode_u_type(opcode: u8, rd: u8, imm: i32) -> u32 {
    let opcode = opcode as u32;
    let rd = (rd as u32) << 7;
    let imm = imm as u32 & !0x0FFF;
    opcode | rd | imm
}

struct FormatB {
    rs1: u8,
    rs2: u8,
    imm: u32,
}

fn parse_format_b(word: u32) -> FormatB {
    FormatB {
        rs1: ((word >> 15) & 0x1f) as u8, // [19:15]
        rs2: ((word >> 20) & 0x1f) as u8, // [24:20]
        imm: (
            match word & 0x80000000 { // imm[31:12] = [31]
				0x80000000 => 0xfffff000,
				_ => 0
			} |
			((word << 4) & 0x00000800) | // imm[11] = [7]
			((word >> 20) & 0x000007e0) | // imm[10:5] = [30:25]
			((word >> 7) & 0x0000001e)
            // imm[4:1] = [11:8]
        ) as i32 as i64 as u32,
    }
}

struct FormatCSR {
    csr: u16,
    rs: u8,
    rd: u8,
}

fn parse_format_csr(word: u32) -> FormatCSR {
    FormatCSR {
        csr: ((word >> 20) & 0xfff) as u16, // [31:20]
        rs: ((word >> 15) & 0x1f) as u8,    // [19:15], also uimm
        rd: ((word >> 7) & 0x1f) as u8,     // [11:7]
    }
}

struct FormatI {
    rd: u8,
    rs1: u8,
    imm: u32,
}

fn parse_format_i(word: u32) -> FormatI {
    FormatI {
        rd: ((word >> 7) & 0x1f) as u8,   // [11:7]
        rs1: ((word >> 15) & 0x1f) as u8, // [19:15]
        imm: (
            match word & 0x80000000 {
                // imm[31:11] = [31]
                0x80000000 => 0xfffff800,
                _ => 0,
            } | ((word >> 20) & 0x000007ff)
            // imm[10:0] = [30:20]
        ) as i32 as i64 as u32,
    }
}

struct FormatJ {
    rd: u8,
    imm: u32,
}

fn parse_format_j(word: u32) -> FormatJ {
    FormatJ {
        rd: ((word >> 7) & 0x1f) as u8, // [11:7]
        imm: (
            match word & 0x80000000 { // imm[31:20] = [31]
				0x80000000 => 0xfff00000,
				_ => 0
			} |
			(word & 0x000ff000) | // imm[19:12] = [19:12]
			((word & 0x00100000) >> 9) | // imm[11] = [20]
			((word & 0x7fe00000) >> 20)
            // imm[10:1] = [30:21]
        ) as i32 as i64 as u32,
    }
}

struct FormatR {
    rd: u8,
    rs1: u8,
    rs2: u8,
}

fn parse_format_r(word: u32) -> FormatR {
    FormatR {
        rd: ((word >> 7) & 0x1f) as u8,   // [11:7]
        rs1: ((word >> 15) & 0x1f) as u8, // [19:15]
        rs2: ((word >> 20) & 0x1f) as u8, // [24:20]
    }
}

// has rs3
struct FormatR2 {
    rd: u8,
    rs1: u8,
    rs2: u8,
    rs3: u8,
}

fn parse_format_r2(word: u32) -> FormatR2 {
    FormatR2 {
        rd: ((word >> 7) & 0x1f) as u8,   // [11:7]
        rs1: ((word >> 15) & 0x1f) as u8, // [19:15]
        rs2: ((word >> 20) & 0x1f) as u8, // [24:20]
        rs3: ((word >> 27) & 0x1f) as u8, // [31:27]
    }
}

struct FormatS {
    rs1: u8,
    rs2: u8,
    imm: u32,
}

fn parse_format_s(word: u32) -> FormatS {
    FormatS {
        rs1: ((word >> 15) & 0x1f) as u8, // [19:15]
        rs2: ((word >> 20) & 0x1f) as u8, // [24:20]
        imm: (
            match word & 0x80000000 {
				0x80000000 => 0xfffff000,
				_ => 0
			} | // imm[31:12] = [31]
			((word >> 20) & 0xfe0) | // imm[11:5] = [31:25]
			((word >> 7) & 0x1f)
            // imm[4:0] = [11:7]
        ) as i32 as i64 as u32,
    }
}

struct FormatU {
    rd: usize,
    imm: u64,
}

fn parse_format_u(word: u32) -> FormatU {
    FormatU {
        rd: ((word >> 7) & 0x1f) as usize, // [11:7]
        imm: (
            match word & 0x80000000 {
				0x80000000 => 0xffffffff00000000,
				_ => 0
			} | // imm[63:32] = [31]
			((word as u64) & 0xfffff000)
            // imm[31:12] = [31:12]
        ) as u64,
    }
}

#[cfg(test)]
mod tests {
    use crate::ops_decode::{
        get_compressed_cj_jump_imm, get_funct3, get_funct7, get_imm_b_type, get_imm_i_type,
        get_imm_s_type, get_opcode, get_rd, get_rs1, get_rs2,
    };

    use super::{
        encode_i_type, get_imm_j_type, parse_format_b, parse_format_i, parse_format_j,
        parse_format_s,
    };
    #[test]
    fn super_test_b() {
        for word in 0..=u32::MAX {
            let r = parse_format_b(word);
            let l = get_imm_b_type(word);
            if r.imm != l {
                assert_eq!(l, r.imm);
            }
        }
    }
    #[test]
    fn super_test_i() {
        for word in 0..=u32::MAX {
            let r = parse_format_i(word);
            let l = get_imm_i_type(word);
            if r.imm != l {
                let imm = r.imm;
                println!("{imm:00$b}", 32);
                assert_eq!(l, r.imm);
            }
        }
    }
    #[test]
    fn super_test_s() {
        for word in 0..=u32::MAX {
            let r = parse_format_s(word);
            let l = get_imm_s_type(word);
            if r.imm != l {
                let imm = r.imm;
                println!("{imm:00$b}", 32);
                assert_eq!(l, r.imm);
            }
        }
    }
    #[test]
    fn super_test_j() {
        for word in 0..=u32::MAX {
            let r = parse_format_j(word);
            let l = get_imm_j_type(word);
            if r.imm != l {
                let imm = r.imm;
                println!("{imm:00$b}", 32);
                assert_eq!(l, r.imm);
            }
        }
    }

    #[test]
    fn test_r_type() {
        let n: u32 = 0b10101010_10101010_10101010_10101010;
        assert_eq!(get_opcode(n), 0b0101010);
        assert_eq!(get_rd(n), 0b10101);
        assert_eq!(get_funct3(n), 0b010);
        assert_eq!(get_rs1(n), 0b10101);
        assert_eq!(get_rs2(n), 0b1010);
        assert_eq!(get_funct7(n), 0b1010101);
    }
    #[test]
    fn test_i_type() {
        let n: u32 = 0b10101010_10101010_10101010_10101010;
        assert_eq!(get_opcode(n), 0b0101010);
        assert_eq!(get_rd(n), 0b10101);
        assert_eq!(get_funct3(n), 0b010);
        assert_eq!(get_rs1(n), 0b10101);
        assert_eq!(get_imm_i_type(n), 0b11111111_11111111_11111010_10101010);
    }
    #[test]
    fn test_s_type() {
        let n: u32 = 0b10101010_10101010_10101010_10101010;
        assert_eq!(get_opcode(n), 0b0101010);
        assert_eq!(get_rd(n), 0b10101);
        assert_eq!(get_funct3(n), 0b010);
        assert_eq!(get_rs1(n), 0b10101);
        assert_eq!(get_rs2(n), 0b1010);
        //println!("{:032b}", get_imm_s_type(n));
        assert_eq!(get_imm_s_type(n), 0b11111111_11111111_11111010_10110101);
    }
    #[test]
    fn test_b_imm() {
        let n: u32 = 0b10101010_10101010_10101010_10101010;
        let left = get_imm_b_type(n);
        let right = 0b1111_1111_1111_1111_1111_1010_1011_0100;
        assert_eq!(left, right)
    }
    #[test]
    fn test_cj_imm() {
        let n: u16 = 0b000_10000000000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b11111111111111111111100000000000;
        assert_eq!(left, right);
        let n: u16 = 0b000_01000000000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b10000;
        assert_eq!(left, right);
        let n: u16 = 0b000_00110000000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b1100000000;
        assert_eq!(left, right);
        let n: u16 = 0b000_00001000000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b10000000000;
        assert_eq!(left, right);
        let n: u16 = 0b000_00000100000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b1000000;
        assert_eq!(left, right);
        let n: u16 = 0b000_00000010000_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b10000000;
        assert_eq!(left, right);
        let n: u16 = 0b000_00000001110_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b1110;
        assert_eq!(left, right);
        let n: u16 = 0b000_00000000001_00;
        let left = get_compressed_cj_jump_imm(n);
        let right = 0b100000;
        assert_eq!(left, right);

        //println!("0b{left:00$b}", 16);
    }
    #[test]
    fn test_i_encoding() {
        let opcode = 0b0010011;
        let rd = 6;
        let funct3 = 0b101;
        let rs1 = 4;
        let imm = 10;
        let n = encode_i_type(opcode, rd, funct3, rs1, imm);
        // 000000001010_00100_101_00110_0010011
        assert_eq!(get_opcode(n), opcode);
        assert_eq!(get_rd(n), rd);
        assert_eq!(get_funct3(n), funct3);
        assert_eq!(get_rs1(n), rs1);
        assert_eq!(get_imm_i_type(n), imm as i32 as u32);
    }
}
