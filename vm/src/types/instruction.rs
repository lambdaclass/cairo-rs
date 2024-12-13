use crate::Felt252;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::vm::decoding::decoder::{decode_instruction, decode_offset};

#[cfg(feature = "test_utils")]
use arbitrary::Arbitrary;

#[cfg_attr(feature = "test_utils", derive(Arbitrary))]
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Register {
    AP,
    FP,
}

// FIXME: impl Debug as decoded
// TODO: possibly exploit NonZero or NonMax so `Result<Instruction, _>` and `Option<Instruction>`
// can use the niche. Note NonMax is guaranteed by the restriction of MSB being 0.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Instruction(u64);

impl Instruction {
    #[inline]
    pub fn offset0(self) -> isize {
        decode_offset(self.0 & 0xffff)
    }

    #[inline]
    pub fn offset1(self) -> isize {
        decode_offset((self.0 >> 16) & 0xffff)
    }

    #[inline]
    pub fn offset2(self) -> isize {
        decode_offset((self.0 >> 32) & 0xffff)
    }

    #[inline]
    pub fn dst_register(self) -> Register {
        const DST_REG_MSK: u64 = 1 << 48;
        const AP: u64 = 0;
        const FP: u64 = 1 << 48;
        match self.0 & DST_REG_MSK {
            AP => Register::AP,
            FP => Register::FP,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn op0_register(self) -> Register {
        const OP0_SRC_MSK: u64 = 1 << 49;
        const AP: u64 = 0;
        const FP: u64 = 1 << 49;
        match self.0 & OP0_SRC_MSK {
            AP => Register::AP,
            FP => Register::FP,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn op1_addr(self) -> Op1Addr {
        const OP1_SRC_MSK: u64 = 7 << 50;
        const OP0: u64 = 0;
        const IMM: u64 = 1 << 50;
        const FP: u64 = 2 << 50;
        const AP: u64 = 4 << 50;
        match self.0 & OP1_SRC_MSK {
            OP0 => Op1Addr::Op0,
            IMM => Op1Addr::Imm,
            FP => Op1Addr::FP,
            AP => Op1Addr::AP,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn res(self) -> Res {
        const RES_LOGIC_MSK: u64 = (3 << 53) | (1 << 57);
        const OP1: u64 = 0;
        const ADD: u64 = 1 << 53;
        const MUL: u64 = 2 << 53;
        const FREE: u64 = 1 << 57;
        match self.0 & RES_LOGIC_MSK {
            OP1 => Res::Op1,
            ADD => Res::Add,
            MUL => Res::Mul,
            FREE => Res::Unconstrained,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn pc_update(self) -> PcUpdate {
        const PC_UPD_MSK: u64 = 7 << 55;
        const REG: u64 = 0;
        const JMP: u64 = 1 << 55;
        const JRL: u64 = 2 << 55;
        const JNZ: u64 = 4 << 55;
        match self.0 & PC_UPD_MSK {
            REG => PcUpdate::Regular,
            JMP => PcUpdate::Jump,
            JRL => PcUpdate::JumpRel,
            JNZ => PcUpdate::Jnz,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn ap_update(self) -> ApUpdate {
        const AP_UPD_MSK: u64 = 7 << 58;
        const REG: u64 = 0;
        const ADD: u64 = 1 << 58;
        const ADD1: u64 = 2 << 58;
        const ADD2: u64 = 4 << 58;
        match self.0 & AP_UPD_MSK {
            REG => ApUpdate::Regular,
            ADD => ApUpdate::Add,
            ADD1 => ApUpdate::Add1,
            ADD2 => ApUpdate::Add2,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn fp_update(self) -> FpUpdate {
        const OPCODE_MSK: u64 = 7 << 60;
        const NOP: u64 = 0;
        const CALL: u64 = 1 << 60;
        const RET: u64 = 2 << 60;
        const ASSERT_EQ: u64 = 4 << 60;
        match self.0 & OPCODE_MSK {
            CALL => FpUpdate::APPlus2,
            RET => FpUpdate::Dst,
            NOP | ASSERT_EQ => FpUpdate::Regular,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn opcode(self) -> Opcode {
        const OPCODE_MSK: u64 = 7 << 60;
        const NOP: u64 = 0;
        const CALL: u64 = 1 << 60;
        const RET: u64 = 2 << 60;
        const ASSERT_EQ: u64 = 4 << 60;
        match self.0 & OPCODE_MSK {
            NOP => Opcode::NOp,
            CALL => Opcode::Call,
            RET => Opcode::Ret,
            ASSERT_EQ => Opcode::AssertEq,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn size(self) -> usize {
        // 2 if immediate operand, 1 otherwise
        (((self.0 >> 50) & 1) + 1) as usize
    }

    #[inline]
    pub fn is_call(self) -> bool {
        let flags = (self.0 >> 48) & 0x7fe0;
        flags == 0b0001000100000000 || flags == 0b0001001000000000
        // Res::Op1
        // PcUpdate::Jump || PcUpdate::JumpRel
        // ApUpdate::Add2
        // FpUpdate::APPlus2
        // Opcode::Call
    }
}

impl TryFrom<u64> for Instruction {
    type Error = ();

    fn try_from(v: u64) -> Result<Self, ()> {
        let flags = v >> 48;
        let dstreg_bits = (flags & 0x0001).count_ones();
        let op0reg_bits = (flags & 0x0002).count_ones();
        let op1src_bits = (flags & 0x001c).count_ones();
        let reslog_bits = (flags & 0x0260).count_ones();
        let pcupd_bits = (flags & 0x0380).count_ones();
        let apupd_bits = (flags & 0x1c00).count_ones();
        let opcode_bits = (flags & 0x7000).count_ones();
        let fpupd_bits = (flags & 0x7000).count_ones();
        let high_bit = flags as u32 & 0x8000;

        if (high_bit
            | opcode_bits
            | fpupd_bits
            | apupd_bits
            | pcupd_bits
            | reslog_bits
            | dstreg_bits
            | op0reg_bits
            | op1src_bits)
            > 1
        {
            return Err(());
        }

        Ok(Self(v))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Op1Addr {
    Imm,
    AP,
    FP,
    Op0,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Res {
    Op1,
    Add,
    Mul,
    Unconstrained,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PcUpdate {
    Regular,
    Jump,
    JumpRel,
    Jnz,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApUpdate {
    Regular,
    Add,
    Add1,
    Add2,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FpUpdate {
    Regular,
    APPlus2,
    Dst,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Opcode {
    NOp,
    AssertEq,
    Call,
    Ret,
}

// Returns True if the given instruction looks like a call instruction
pub(crate) fn is_call_instruction(encoded_instruction: &Felt252) -> bool {
    let encoded_i64_instruction = match encoded_instruction.to_u64() {
        Some(num) => num,
        None => return false,
    };
    let instruction = match decode_instruction(encoded_i64_instruction) {
        Ok(inst) => inst,
        Err(_) => return false,
    };
    instruction.is_call()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_call_instruction_true() {
        let encoded_instruction = Felt252::from(1226245742482522112_i64);
        assert!(is_call_instruction(&encoded_instruction));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_call_instruction_false() {
        let encoded_instruction = Felt252::from(4612671187288031229_i64);
        assert!(!is_call_instruction(&encoded_instruction));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_call_instruction_invalid() {
        let encoded_instruction = Felt252::from(1u64 << 63);
        assert!(!is_call_instruction(&encoded_instruction));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn instruction_size() {
        let encoded_instruction = Felt252::from(1226245742482522112_i64);
        let instruction = decode_instruction(encoded_instruction.to_u64().unwrap()).unwrap();
        assert_eq!(instruction.size(), 2);
    }
}
