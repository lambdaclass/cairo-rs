use crate::{types::instruction::Instruction, vm::errors::vm_errors::VirtualMachineError};

//  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
// 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0

/// Decodes an instruction. The encoding is little endian, so flags go from bit 63 to 48.
pub fn decode_instruction(encoded_instr: u64) -> Result<Instruction, VirtualMachineError> {
    // FIXME: replace the function and use the extended decoding for error when relevant
    encoded_instr
        .try_into()
        .map_err(|_| VirtualMachineError::InstructionNonZeroHighBit)
    /*
    const HIGH_BIT: u64 = 1u64 << 63;
    const DST_REG_MASK: u64 = 0x0001;
    const DST_REG_OFF: u64 = 0;
    const OP0_REG_MASK: u64 = 0x0002;
    const OP0_REG_OFF: u64 = 1;
    const OP1_SRC_MASK: u64 = 0x001C;
    const OP1_SRC_OFF: u64 = 2;
    const RES_LOGIC_MASK: u64 = 0x0060;
    const RES_LOGIC_OFF: u64 = 5;
    const PC_UPDATE_MASK: u64 = 0x0380;
    const PC_UPDATE_OFF: u64 = 7;
    const AP_UPDATE_MASK: u64 = 0x0C00;
    const AP_UPDATE_OFF: u64 = 10;
    const OPCODE_MASK: u64 = 0x7000;
    const OPCODE_OFF: u64 = 12;

    // Flags start on the 48th bit.
    const FLAGS_OFFSET: u64 = 48;
    const OFF0_OFF: u64 = 0;
    const OFF1_OFF: u64 = 16;
    const OFF2_OFF: u64 = 32;
    const OFFX_MASK: u64 = 0xFFFF;

    if encoded_instr & HIGH_BIT != 0 {
        return Err(VirtualMachineError::InstructionNonZeroHighBit);
    }

    // Grab offsets and convert them from little endian format.
    let off0 = decode_offset(encoded_instr >> OFF0_OFF & OFFX_MASK);
    let off1 = decode_offset(encoded_instr >> OFF1_OFF & OFFX_MASK);
    let off2 = decode_offset(encoded_instr >> OFF2_OFF & OFFX_MASK);

    // Grab flags
    let flags = encoded_instr >> FLAGS_OFFSET;
    // Grab individual flags
    let dst_reg_num = (flags & DST_REG_MASK) >> DST_REG_OFF;
    let op0_reg_num = (flags & OP0_REG_MASK) >> OP0_REG_OFF;
    let op1_src_num = (flags & OP1_SRC_MASK) >> OP1_SRC_OFF;
    let res_logic_num = (flags & RES_LOGIC_MASK) >> RES_LOGIC_OFF;
    let pc_update_num = (flags & PC_UPDATE_MASK) >> PC_UPDATE_OFF;
    let ap_update_num = (flags & AP_UPDATE_MASK) >> AP_UPDATE_OFF;
    let opcode_num = (flags & OPCODE_MASK) >> OPCODE_OFF;

    // Match each flag to its corresponding enum value
    let dst_register = if dst_reg_num == 1 {
        Register::FP
    } else {
        Register::AP
    };

    let op0_register = if op0_reg_num == 1 {
        Register::FP
    } else {
        Register::AP
    };

    let op1_addr = match op1_src_num {
        0 => Op1Addr::Op0,
        1 => Op1Addr::Imm,
        2 => Op1Addr::FP,
        4 => Op1Addr::AP,
        _ => return Err(VirtualMachineError::InvalidOp1Reg(op1_src_num)),
    };

    let pc_update = match pc_update_num {
        0 => PcUpdate::Regular,
        1 => PcUpdate::Jump,
        2 => PcUpdate::JumpRel,
        4 => PcUpdate::Jnz,
        _ => return Err(VirtualMachineError::InvalidPcUpdate(pc_update_num)),
    };

    let res = match res_logic_num {
        0 if matches!(pc_update, PcUpdate::Jnz) => Res::Unconstrained,
        0 => Res::Op1,
        1 => Res::Add,
        2 => Res::Mul,
        _ => return Err(VirtualMachineError::InvalidRes(res_logic_num)),
    };

    let opcode = match opcode_num {
        0 => Opcode::NOp,
        1 => Opcode::Call,
        2 => Opcode::Ret,
        4 => Opcode::AssertEq,
        _ => return Err(VirtualMachineError::InvalidOpcode(opcode_num)),
    };

    let ap_update = match ap_update_num {
        0 if matches!(opcode, Opcode::Call) => ApUpdate::Add2,
        0 => ApUpdate::Regular,
        1 => ApUpdate::Add,
        2 => ApUpdate::Add1,
        _ => return Err(VirtualMachineError::InvalidApUpdate(ap_update_num)),
    };

    let fp_update = match opcode {
        Opcode::Call => FpUpdate::APPlus2,
        Opcode::Ret => FpUpdate::Dst,
        _ => FpUpdate::Regular,
    };

    Ok(Instruction {
        off0,
        off1,
        off2,
        dst_register,
        op0_register,
        op1_addr,
        res,
        pc_update,
        ap_update,
        fp_update,
        opcode,
    })
    */
}

pub(crate) fn decode_offset(offset: u64) -> isize {
    let vectorized_offset: [u8; 8] = offset.to_le_bytes();
    let offset_16b_encoded = u16::from_le_bytes([vectorized_offset[0], vectorized_offset[1]]);
    let complement_const = 0x8000u16;
    let (offset_16b, _) = offset_16b_encoded.overflowing_sub(complement_const);
    isize::from(offset_16b as i16)
}

#[cfg(test)]
mod decoder_test {
    use super::*;
    use crate::stdlib::string::ToString;
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn non_zero_high_bit() {
        let error = decode_instruction(0x94A7800080008000);
        assert_eq!(
            error.unwrap_err().to_string(),
            "Instruction MSB should be 0",
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn invalid_op1_reg() {
        let error = decode_instruction(0x294F800080008000);
        assert_matches!(error, Err(VirtualMachineError::InvalidOp1Reg(3)));
        assert_eq!(
            error.unwrap_err().to_string(),
            "Invalid op1_register value: 3"
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn invalid_pc_update() {
        let error = decode_instruction(0x29A8800080008000);
        assert_matches!(error, Err(VirtualMachineError::InvalidPcUpdate(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid pc_update value: 3")
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn invalid_res_logic() {
        let error = decode_instruction(0x2968800080008000);
        assert_matches!(error, Err(VirtualMachineError::InvalidRes(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid res value: 3")
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn invalid_opcode() {
        let error = decode_instruction(0x3948800080008000);
        assert_matches!(error, Err(VirtualMachineError::InvalidOpcode(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid opcode value: 3")
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn invalid_ap_update() {
        let error = decode_instruction(0x2D48800080008000);
        assert_matches!(error, Err(VirtualMachineError::InvalidApUpdate(3)));
        assert_eq!(error.unwrap_err().to_string(), "Invalid ap_update value: 3")
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_flags_call_add_jmp_add_imm_fp_fp() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |    CALL|      ADD|     JUMP|      ADD|    IMM|     FP|     FP
        //  0  0  0  1      0  1   0  0  1      0  1 0  0  1       1       1
        //  0001 0100 1010 0111 = 0x14A7; offx = 0
        let inst = decode_instruction(0x14A7800080008000).unwrap();
        assert_matches!(inst.dst_register, Register::FP);
        assert_matches!(inst.op0_register, Register::FP);
        assert_matches!(inst.op1_addr, Op1Addr::Imm);
        assert_matches!(inst.res, Res::Add);
        assert_matches!(inst.pc_update, PcUpdate::Jump);
        assert_matches!(inst.ap_update, ApUpdate::Add);
        assert_matches!(inst.opcode, Opcode::Call);
        assert_matches!(inst.fp_update, FpUpdate::APPlus2);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_flags_ret_add1_jmp_rel_mul_fp_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     RET|     ADD1| JUMP_REL|      MUL|     FP|     AP|     AP
        //  0  0  1  0      1  0   0  1  0      1  0 0  1  0       0       0
        //  0010 1001 0100 1000 = 0x2948; offx = 0
        let inst = decode_instruction(0x2948800080008000).unwrap();
        assert_matches!(inst.dst_register, Register::AP);
        assert_matches!(inst.op0_register, Register::AP);
        assert_matches!(inst.op1_addr, Op1Addr::FP);
        assert_matches!(inst.res, Res::Mul);
        assert_matches!(inst.pc_update, PcUpdate::JumpRel);
        assert_matches!(inst.ap_update, ApUpdate::Add1);
        assert_matches!(inst.opcode, Opcode::Ret);
        assert_matches!(inst.fp_update, FpUpdate::Dst);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_flags_assrt_add_jnz_mul_ap_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |ASSRT_EQ|      ADD|      JNZ|      MUL|     AP|     AP|     AP
        //  0  1  0  0      1  0   1  0  0      1  0 1  0  0       0       0
        //  0100 1010 0101 0000 = 0x4A50; offx = 0
        let inst = decode_instruction(0x4A50800080008000).unwrap();
        assert_matches!(inst.dst_register, Register::AP);
        assert_matches!(inst.op0_register, Register::AP);
        assert_matches!(inst.op1_addr, Op1Addr::AP);
        assert_matches!(inst.res, Res::Mul);
        assert_matches!(inst.pc_update, PcUpdate::Jnz);
        assert_matches!(inst.ap_update, ApUpdate::Add1);
        assert_matches!(inst.opcode, Opcode::AssertEq);
        assert_matches!(inst.fp_update, FpUpdate::Regular);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_flags_assrt_add2_jnz_uncon_op0_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |ASSRT_EQ|     ADD2|      JNZ|UNCONSTRD|    OP0|     AP|     AP
        //  0  1  0  0      0  0   1  0  0      0  0 0  0  0       0       0
        //  0100 0010 0000 0000 = 0x4200; offx = 0
        let inst = decode_instruction(0x4200800080008000).unwrap();
        assert_matches!(inst.dst_register, Register::AP);
        assert_matches!(inst.op0_register, Register::AP);
        assert_matches!(inst.op1_addr, Op1Addr::Op0);
        assert_matches!(inst.res, Res::Unconstrained);
        assert_matches!(inst.pc_update, PcUpdate::Jnz);
        assert_matches!(inst.ap_update, ApUpdate::Regular);
        assert_matches!(inst.opcode, Opcode::AssertEq);
        assert_matches!(inst.fp_update, FpUpdate::Regular);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_flags_nop_regu_regu_op1_op0_ap_ap() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     NOP|  REGULAR|  REGULAR|      OP1|    OP0|     AP|     AP
        //  0  0  0  0      0  0   0  0  0      0  0 0  0  0       0       0
        //  0000 0000 0000 0000 = 0x0000; offx = 0
        let inst = decode_instruction(0x0000800080008000).unwrap();
        assert_matches!(inst.dst_register, Register::AP);
        assert_matches!(inst.op0_register, Register::AP);
        assert_matches!(inst.op1_addr, Op1Addr::Op0);
        assert_matches!(inst.res, Res::Op1);
        assert_matches!(inst.pc_update, PcUpdate::Regular);
        assert_matches!(inst.ap_update, ApUpdate::Regular);
        assert_matches!(inst.opcode, Opcode::NOp);
        assert_matches!(inst.fp_update, FpUpdate::Regular);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn decode_offset_negative() {
        //  0|  opcode|ap_update|pc_update|res_logic|op1_src|op0_reg|dst_reg
        // 15|14 13 12|    11 10|  9  8  7|     6  5|4  3  2|      1|      0
        //   |     NOP|  REGULAR|  REGULAR|      OP1|    OP0|     AP|     AP
        //  0  0  0  0      0  0   0  0  0      0  0 0  0  0       0       0
        //  0000 0000 0000 0000 = 0x0000; offx = 0
        let inst = decode_instruction(0x0000800180007FFF).unwrap();
        assert_eq!(inst.off0, -1);
        assert_eq!(inst.off1, 0);
        assert_eq!(inst.off2, 1);
    }
}
