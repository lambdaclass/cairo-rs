use crate::{
    types::{
        instruction::{Instruction, Op1Addr, Register},
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::errors::{
        memory_errors::MemoryError::AddressNotRelocatable, vm_errors::VirtualMachineError,
    },
};
use num_traits::abs;

pub struct RunContext {
    pub(crate) pc: Relocatable,
    pub(crate) ap: usize,
    pub(crate) fp: usize,
}

impl RunContext {
    pub fn get_ap(&self) -> Relocatable {
        Relocatable::from((1, self.ap))
    }
    pub fn get_fp(&self) -> Relocatable {
        Relocatable::from((1, self.fp))
    }
    pub fn get_pc(&self) -> Relocatable {
        self.pc
    }

    pub fn new(pc: Relocatable, ap: usize, fp: usize) -> Self {
        RunContext { pc, ap, fp }
    }

    pub fn compute_dst_addr(
        &self,
        instruction: Instruction,
    ) -> Result<Relocatable, VirtualMachineError> {
        let base_addr = match instruction.dst_register() {
            Register::AP => self.get_ap(),
            Register::FP => self.get_fp(),
        };
        let off0 = instruction.offset0();
        if off0 < 0 {
            Ok((base_addr - abs(off0) as usize)?)
        } else {
            Ok((base_addr + (off0 as usize))?)
        }
    }

    pub fn compute_op0_addr(
        &self,
        instruction: Instruction,
    ) -> Result<Relocatable, VirtualMachineError> {
        let base_addr = match instruction.op0_register() {
            Register::AP => self.get_ap(),
            Register::FP => self.get_fp(),
        };
        let off1 = instruction.offset1();
        if off1 < 0 {
            Ok((base_addr - abs(off1) as usize)?)
        } else {
            Ok((base_addr + (off1 as usize))?)
        }
    }

    pub fn compute_op1_addr(
        &self,
        instruction: Instruction,
        op0: Option<&MaybeRelocatable>,
    ) -> Result<Relocatable, VirtualMachineError> {
        let off2 = instruction.offset2();
        let base_addr = match instruction.op1_addr() {
            Op1Addr::FP => self.get_fp(),
            Op1Addr::AP => self.get_ap(),
            Op1Addr::Imm => match off2 == 1 {
                true => self.pc,
                false => return Err(VirtualMachineError::ImmShouldBe1),
            },
            Op1Addr::Op0 => match op0 {
                Some(MaybeRelocatable::RelocatableValue(addr)) => *addr,
                Some(_) => return Err(VirtualMachineError::Memory(AddressNotRelocatable)),
                None => return Err(VirtualMachineError::UnknownOp0),
            },
        };
        if off2 < 0 {
            Ok((base_addr - abs(off2) as usize)?)
        } else {
            Ok((base_addr + (off2 as usize))?)
        }
    }

    #[doc(hidden)]
    pub(crate) fn set_ap(&mut self, ap: usize) {
        self.ap = ap;
    }

    #[doc(hidden)]
    pub(crate) fn set_fp(&mut self, fp: usize) {
        self.fp = fp;
    }

    #[doc(hidden)]
    pub(crate) fn set_pc(&mut self, pc: Relocatable) {
        self.pc = pc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relocatable;
    use crate::stdlib::string::ToString;
    use crate::types::instruction::{ApUpdate, FpUpdate, Opcode, PcUpdate, Res};
    use crate::utils::test_utils::mayberelocatable;
    use crate::vm::errors::memory_errors::MemoryError;
    use crate::Felt252;
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_dst_addr_for_ap_register() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::AP,
            op0_register: Register::FP,
            op1_addr: Op1Addr::AP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_dst_addr(&instruction),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 6)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_dst_addr_for_fp_register() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };

        assert_matches!(
            run_context.compute_dst_addr(&instruction),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 7)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op0_addr_for_ap_register() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::AP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_op0_addr(&instruction),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 7)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op0_addr_for_fp_register() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::FP,
            op1_addr: Op1Addr::AP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_op0_addr(&instruction),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 8)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_fp_op1_addr() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::FP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_op1_addr(&instruction, None),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 9)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_ap_op1_addr() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_op1_addr(&instruction, None),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 8)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_imm_op1_addr_correct_off2() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 1,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::Imm,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };
        assert_matches!(
            run_context.compute_op1_addr(&instruction, None),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(0, 5)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_imm_op1_addr_incorrect_off2() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::Imm,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };

        let error = run_context.compute_op1_addr(&instruction, None);
        assert_matches!(error, Err(VirtualMachineError::ImmShouldBe1));
        assert_eq!(
            error.unwrap_err().to_string(),
            "In immediate mode, off2 should be 1"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_op0_op1_addr_with_op0() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 1,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::Op0,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };

        let op0 = mayberelocatable!(1, 7);
        assert_matches!(
            run_context.compute_op1_addr(&instruction, Some(&op0)),
            Ok::<Relocatable, VirtualMachineError>(x) if x == relocatable!(1, 8)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_with_no_relocatable_address() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 1,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::Op0,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };

        let op0 = MaybeRelocatable::from(Felt252::from(7));
        assert_matches!(
            run_context.compute_op1_addr(&instruction, Some(&op0)),
            Err::<Relocatable, VirtualMachineError>(VirtualMachineError::Memory(
                MemoryError::AddressNotRelocatable
            ))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn compute_op1_addr_for_op0_op1_addr_without_op0() {
        let instruction = Instruction {
            off0: 1,
            off1: 2,
            off2: 3,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::Op0,
            res: Res::Add,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        };

        let run_context = RunContext {
            pc: relocatable!(0, 4),
            ap: 5,
            fp: 6,
        };

        let error = run_context.compute_op1_addr(&instruction, None);
        assert_matches!(error, Err(VirtualMachineError::UnknownOp0));
        assert_eq!(
            error.unwrap_err().to_string(),
            "op0 must be known in double dereference"
        );
    }
}
