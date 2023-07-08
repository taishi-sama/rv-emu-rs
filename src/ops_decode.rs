macro_rules! shift_and_trim {
    ($u:expr, $shift:expr, $mask:expr) => {
        ($u >> $shift & (u32::MAX >> (32 - $mask))) as _
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
    sign_extend32((get_funct7(instruction) as u32) << 5 | get_rd(instruction) as u32, 12)
}
#[inline]
pub fn get_imm_b_type(instruction: u32) -> u32 {
    let i_11_8: u32 = from_to_get(instruction, 8, 11, 1);
    let i_30_25: u32 = from_to_get(instruction, 25, 30, 5);
    let i_31: u32 = from_to_get(instruction, 31, 31, 12);
    let i_7: u32 = from_to_get(instruction, 7, 7, 11);
    sign_extend32(i_7 | i_11_8 | i_30_25 | i_31, 12)
}
#[inline]
pub fn get_imm_u_type(instruction: u32) -> u32 {
    instruction & !0b1111_1111_1111
}
pub fn get_rs3(instruction: u32) -> u8 {
    from_to_get(instruction, 27, 31, 0) as u8
}
#[inline]
pub fn get_imm_j_type(instruction: u32) -> u32 {
    let i_24_21: u32 = from_to_get(instruction, 21, 24, 1);
    let i_30_25: u32 = from_to_get(instruction, 25, 30, 5);
    let i_20: u32 = from_to_get(instruction, 20, 20, 11);
    let i_19_12: u32 = from_to_get(instruction, 12, 19, 12);
    let i_31: u32 = from_to_get(instruction, 31, 31, 20);
    sign_extend32(i_24_21 | i_30_25 | i_20 | i_19_12 | i_31, 20)
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

#[cfg(test)]
mod tests {
    use crate::ops_decode::{
        get_funct3, get_funct7, get_imm_b_type, get_imm_i_type, get_imm_s_type, get_opcode, get_rd,
        get_rs1, get_rs2,
    };

    use super::encode_i_type;

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
