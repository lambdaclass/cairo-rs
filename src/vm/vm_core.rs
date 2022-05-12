use crate::vm::instruction::{ApUpdate, FpUpdate, Instruction, Opcode, PcUpdate, Res};
use crate::vm::relocatable::MaybeRelocatable;
use crate::vm::run_context::RunContext;
use crate::vm::trace_entry::TraceEntry;
use crate::vm::validated_memory_dict::ValidatedMemoryDict;
use num_bigint::BigInt;
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::fmt;

macro_rules! bigint {
    ($val : expr) => {
        BigInt::from_i32($val).unwrap()
    };
}

struct Operands {
    dst: MaybeRelocatable,
    res: Option<MaybeRelocatable>,
    op0: MaybeRelocatable,
    op1: MaybeRelocatable,
}

struct Rule {
    func: fn(&VirtualMachine, &MaybeRelocatable, &()) -> Option<MaybeRelocatable>,
}

pub struct VirtualMachine {
    run_context: RunContext,
    prime: BigInt,
    //builtin_runners: Option<HashMap<String, BuiltinRunner>>,
    //exec_scopes: Vec<HashMap<..., ...>>,
    //enter_scope: ,
    //hints: HashMap<MaybeRelocatable, Vec<CompiledHint>>,
    //hint_locals: HashMap<..., ...>,
    //hint_pc_and_index: HashMap<i64, (MaybeRelocatable, i64)>,
    //static_locals: Option<HashMap<..., ...>>,
    //intruction_debug_info: HashMap<MaybeRelocatable, InstructionLocation>,
    //debug_file_contents: HashMap<String, String>,
    //error_message_attributes: Vec<VmAttributeScope>,
    //program: ProgramBase,
    program_base: Option<MaybeRelocatable>,
    validated_memory: ValidatedMemoryDict,
    auto_deduction: HashMap<BigInt, Vec<(Rule, ())>>,
    accessesed_addresses: Vec<MaybeRelocatable>,
    trace: Vec<TraceEntry>,
    current_step: BigInt,
    skip_instruction_execution: bool,
}

impl VirtualMachine {
    fn update_fp(&mut self, instruction: &Instruction, operands: &Operands) {
        let new_fp: MaybeRelocatable = match instruction.fp_update {
            FpUpdate::AP_PLUS2 => self.run_context.ap.add_num_addr(bigint!(2), None),
            FpUpdate::DST => operands.dst.clone(),
            FpUpdate::REGULAR => return,
        };
        self.run_context.fp = new_fp;
    }

    fn update_ap(
        &mut self,
        instruction: &Instruction,
        operands: &Operands,
    ) -> Result<(), VirtualMachineError> {
        let new_ap: MaybeRelocatable = match instruction.ap_update {
            ApUpdate::ADD => match operands.res.clone() {
                Some(res) => self
                    .run_context
                    .ap
                    .add_addr(res, Some(self.prime.clone()))?,

                None => return Err(VirtualMachineError::UnconstrainedResAddError),
            },
            ApUpdate::ADD1 => self.run_context.ap.add_num_addr(bigint!(1), None),
            ApUpdate::ADD2 => self.run_context.ap.add_num_addr(bigint!(2), None),
            ApUpdate::REGULAR => return Ok(()),
        };
        self.run_context.ap = new_ap % self.prime.clone();
        Ok(())
    }

    fn update_pc(
        &mut self,
        instruction: &Instruction,
        operands: &Operands,
    ) -> Result<(), VirtualMachineError> {
        let new_pc: MaybeRelocatable = match instruction.pc_update {
            PcUpdate::REGULAR => self
                .run_context
                .pc
                .add_num_addr(bigint!(Instruction::size(&instruction)), None),
            PcUpdate::JUMP => match operands.res.clone() {
                Some(res) => res,
                None => return Err(VirtualMachineError::UnconstrainedResJumpError),
            },
            PcUpdate::JUMP_REL => match operands.res.clone() {
                Some(res) => match res {
                    MaybeRelocatable::Int(num_res) => {
                        self.run_context.pc.add_num_addr(num_res, None)
                    }

                    _ => return Err(VirtualMachineError::PureValueError),
                },
                None => return Err(VirtualMachineError::UnconstrainedResJumpRelError),
            },
            PcUpdate::JNZ => match VirtualMachine::is_zero(operands.res.clone())? {
                true => self
                    .run_context
                    .pc
                    .add_num_addr(bigint!(Instruction::size(&instruction)), None),
                false => (self.run_context.pc.add_addr(operands.op1.clone(), None))?,
            },
        };
        self.run_context.pc = new_pc % self.prime.clone();
        Ok(())
    }

    fn update_registers(
        &mut self,
        instruction: Instruction,
        operands: Operands,
    ) -> Result<(), VirtualMachineError> {
        self.update_fp(&instruction, &operands);
        self.update_ap(&instruction, &operands)?;
        self.update_pc(&instruction, &operands)?;
        Ok(())
    }

    /// Returns true if the value is zero
    /// Used for JNZ instructions
    fn is_zero(addr: Option<MaybeRelocatable>) -> Result<bool, VirtualMachineError> {
        if let Some(value) = addr {
            match value {
                MaybeRelocatable::Int(num) => return Ok(num == bigint!(0)),
                MaybeRelocatable::RelocatableValue(rel_value) => {
                    if rel_value.offset >= bigint!(0) {
                        return Ok(false);
                    } else {
                        return Err(VirtualMachineError::PureValueError);
                    }
                }
            };
        }
        return Err(VirtualMachineError::NotImplementedError);
    }

    ///Returns a tuple (deduced_op0, deduced_res).
    ///Deduces the value of op0 if possible (based on dst and op1). Otherwise, returns None.
    ///If res was already deduced, returns its deduced value as well.
    fn deduce_op0(
        &self,
        instruction: &Instruction,
        dst: Option<&MaybeRelocatable>,
        op1: Option<&MaybeRelocatable>,
    ) -> Result<(Option<MaybeRelocatable>, Option<MaybeRelocatable>), VirtualMachineError> {
        match instruction.opcode {
            Opcode::CALL => {
                return Ok((
                    Some(
                        self.run_context
                            .pc
                            .add_num_addr(bigint!(Instruction::size(&instruction)), None),
                    ),
                    None,
                ))
            }
            Opcode::ASSERT_EQ => {
                match instruction.res {
                    Res::ADD => {
                        if let (Some(dst_addr), Some(op1_addr)) = (dst, op1) {
                            return Ok((
                                Some((dst_addr.sub_addr(op1_addr))? % self.prime.clone()),
                                Some(dst_addr.clone()),
                            ));
                        }
                    }
                    Res::MUL => {
                        if let (Some(dst_addr), Some(op1_addr)) = (dst, op1) {
                            if let (
                                MaybeRelocatable::Int(num_dst),
                                MaybeRelocatable::Int(ref num_op1_ref),
                            ) = (dst_addr, op1_addr)
                            {
                                let num_op1 = Clone::clone(num_op1_ref);
                                if num_op1 != bigint!(0) {
                                    return Ok((
                                        Some(
                                            MaybeRelocatable::Int(num_dst / num_op1)
                                                % self.prime.clone(),
                                        ),
                                        Some(dst_addr.clone()),
                                    ));
                                }
                            }
                        }
                    }
                    _ => (),
                };
            }
            _ => (),
        };
        Ok((None, None))
    }

    /// Returns a tuple (deduced_op1, deduced_res).
    ///Deduces the value of op1 if possible (based on dst and op0). Otherwise, returns None.
    ///If res was already deduced, returns its deduced value as well.
    fn deduce_op1(
        &self,
        instruction: &Instruction,
        dst: Option<&MaybeRelocatable>,
        op0: Option<MaybeRelocatable>,
    ) -> Result<(Option<MaybeRelocatable>, Option<MaybeRelocatable>), VirtualMachineError> {
        if let Opcode::ASSERT_EQ = instruction.opcode {
            match instruction.res {
                Res::OP1 => {
                    if let Some(dst_addr) = dst {
                        return Ok((Some(dst_addr.clone()), Some(dst_addr.clone())));
                    }
                }
                Res::ADD => {
                    if let (Some(dst_addr), Some(op0_addr)) = (dst, op0) {
                        return Ok((
                            Some((dst_addr.sub_addr(&op0_addr))?),
                            Some(dst_addr.clone()),
                        ));
                    }
                }
                Res::MUL => {
                    if let (Some(dst_addr), Some(op0_addr)) = (dst, op0) {
                        if let (MaybeRelocatable::Int(num_dst), MaybeRelocatable::Int(num_op0)) =
                            (dst_addr, op0_addr)
                        {
                            if num_op0 != bigint!(0) {
                                return Ok((
                                    Some(
                                        MaybeRelocatable::Int(num_dst / num_op0)
                                            % self.prime.clone(),
                                    ),
                                    Some(dst_addr.clone()),
                                ));
                            }
                        }
                    }
                }
                _ => (),
            };
        };
        return Ok((None, None));
    }

    ///Computes the value of res if possible
    fn compute_res(
        &self,
        instruction: &Instruction,
        op0: &MaybeRelocatable,
        op1: &MaybeRelocatable,
    ) -> Result<Option<MaybeRelocatable>, VirtualMachineError> {
        match instruction.res {
            Res::OP1 => return Ok(Some(op1.clone())),
            Res::ADD => return Ok(Some(op0.add_addr(op1.clone(), Some(self.prime.clone()))?)),
            Res::MUL => {
                if let (MaybeRelocatable::Int(num_op0), MaybeRelocatable::Int(num_op1)) = (op0, op1)
                {
                    return Ok(Some(
                        MaybeRelocatable::Int(num_op0 * num_op1) % self.prime.clone(),
                    ));
                }
                return Err(VirtualMachineError::PureValueError);
            }
            Res::UNCONSTRAINED => return Ok(None),
        };
    }

    fn deduce_dst(
        &self,
        instruction: &Instruction,
        res: Option<&MaybeRelocatable>,
    ) -> Option<MaybeRelocatable> {
        match instruction.opcode {
            Opcode::ASSERT_EQ => {
                if let Some(res_addr) = res {
                    return Some(res_addr.clone());
                }
            }
            Opcode::CALL => return Some(self.run_context.fp.clone()),
            _ => (),
        };
        return None;
    }

    pub fn deduce_memory_cell(&mut self, addr: MaybeRelocatable) -> Option<MaybeRelocatable> {
        match addr {
            MaybeRelocatable::Int(_) => (),
            MaybeRelocatable::RelocatableValue(ref addr_val) => {
                match self.auto_deduction.get(&addr_val.segment_index) {
                    Some(rules) => {
                        for (rule, args) in rules.iter() {
                            match (rule.func)(self, &addr, args) {
                                Some(value) => {
                                    self.validated_memory.memory.insert(&addr, &value);
                                    return Some(value);
                                },
                                None => (),
                            };
                        }
                    },
                    None => (),
                };
            }
        }
        None
    }
    
    fn opcode_assertions(&self, instruction: &Instruction, operands: &Operands) {
        match instruction.opcode {
            Opcode::ASSERT_EQ => {
                match &operands.res {
                    None => panic!("Res.UNCONSTRAINED cannot be used with Opcode.ASSERT_EQ"),
                    Some(res) => {
                        if let (MaybeRelocatable::Int(res_num), MaybeRelocatable::Int(dst_num)) =
                            (res, &operands.dst)
                        {
                            if res_num != dst_num {
                                panic!(
                                    "An ASSERT_EQ instruction failed: {} != {}",
                                    res_num, dst_num
                                );
                            };
                        };
                    }
                };
            }
            Opcode::CALL => {
                if let (MaybeRelocatable::Int(op0_num), MaybeRelocatable::Int(run_pc)) =
                    (&operands.op0, &self.run_context.pc)
                {
                    let return_pc = run_pc + bigint!(instruction.size());
                    if op0_num != &return_pc {
                        panic!("Call failed to write return-pc (inconsistent op0): {} != {}. Did you forget to increment ap?", op0_num, return_pc);
                    };
                };

                if let (MaybeRelocatable::Int(return_fp), MaybeRelocatable::Int(dst_num)) =
                    (&self.run_context.fp, &operands.dst)
                {
                    if dst_num != return_fp {
                        panic!("Call failed to write return-fp (inconsistent dst): fp->{} != dst->{}. Did you forget to increment ap?",dst_num,dst_num);
                    };
                };
            }
            _ => {}
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum VirtualMachineError {
    //InvalidInstructionEncodingError(MaybeRelocatable), Impl fmt for MaybeRelocatable
    InvalidInstructionEncodingError,
    InvalidDstRegError,
    InvalidOp0RegError,
    InvalidOp1RegError,
    ImmShouldBe1Error,
    UnknownOp0Error,
    InvalidFpUpdateError,
    InvalidApUpdateError,
    InvalidPcUpdateError,
    UnconstrainedResAddError,
    UnconstrainedResJumpError,
    UnconstrainedResJumpRelError,
    PureValueError,
    InvalidResError,
    RelocatableAddError,
    NotImplementedError,
    DiffIndexSubError,
}

impl fmt::Display for VirtualMachineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            //VirtualMachineError::InvalidInstructionEncodingError(arg) => write!(f, "Instruction should be an int. Found: {}", arg),
            VirtualMachineError::InvalidInstructionEncodingError => {
                write!(f, "Instruction should be an int. Found:")
            }
            VirtualMachineError::InvalidDstRegError => write!(f, "Invalid dst_register value"),
            VirtualMachineError::InvalidOp0RegError => write!(f, "Invalid op0_register value"),
            VirtualMachineError::InvalidOp1RegError => write!(f, "Invalid op1_register value"),
            VirtualMachineError::ImmShouldBe1Error => {
                write!(f, "In immediate mode, off2 should be 1")
            }
            VirtualMachineError::UnknownOp0Error => {
                write!(f, "op0 must be known in double dereference")
            }
            VirtualMachineError::InvalidFpUpdateError => write!(f, "Invalid fp_update value"),
            VirtualMachineError::InvalidApUpdateError => write!(f, "Invalid ap_update value"),
            VirtualMachineError::InvalidPcUpdateError => write!(f, "Invalid pc_update value"),
            VirtualMachineError::UnconstrainedResAddError => {
                write!(f, "Res.UNCONSTRAINED cannot be used with ApUpdate.ADD")
            }
            VirtualMachineError::UnconstrainedResJumpError => {
                write!(f, "Res.UNCONSTRAINED cannot be used with PcUpdate.JUMP")
            }
            VirtualMachineError::UnconstrainedResJumpRelError => {
                write!(f, "Res.UNCONSTRAINED cannot be used with PcUpdate.JUMP_REL")
            }
            VirtualMachineError::InvalidResError => write!(f, "Invalid res value"),
            VirtualMachineError::RelocatableAddError => {
                write!(f, "Cannot add two relocatable values")
            }
            VirtualMachineError::NotImplementedError => write!(f, "This is not implemented"),
            VirtualMachineError::PureValueError => Ok(()), //TODO
            VirtualMachineError::DiffIndexSubError => write!(
                f,
                "Can only subtract two relocatable values of the same segment"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::instruction::{ApUpdate, FpUpdate, Op1Addr, Opcode, PcUpdate, Register, Res};
    use crate::vm::memory::Memory;
    use crate::vm::relocatable::Relocatable;

    #[test]
    fn update_fp_ap_plus2() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::AP_PLUS2,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        vm.update_fp(&instruction, &operands);
        assert_eq!(vm.run_context.fp, MaybeRelocatable::Int(bigint!(7)))
    }

    #[test]
    fn update_fp_dst() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::DST,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        vm.update_fp(&instruction, &operands);
        assert_eq!(vm.run_context.fp, MaybeRelocatable::Int(bigint!(11)))
    }

    #[test]
    fn update_fp_regular() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        vm.update_fp(&instruction, &operands);
        assert_eq!(vm.run_context.fp, MaybeRelocatable::Int(bigint!(6)))
    }

    #[test]
    fn update_ap_add_with_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::ADD,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_ap(&instruction, &operands));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(13)));
    }

    #[test]
    fn update_ap_add_without_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::ADD,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: None,
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(
            Err(VirtualMachineError::UnconstrainedResAddError),
            vm.update_ap(&instruction, &operands)
        );
    }

    #[test]
    fn update_ap_add1() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::ADD1,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_ap(&instruction, &operands));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(6)));
    }

    #[test]
    fn update_ap_add2() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::ADD2,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_ap(&instruction, &operands));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(7)));
    }

    #[test]
    fn update_ap_regular() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_ap(&instruction, &operands));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(5)));
    }

    #[test]
    fn update_pc_regular_instruction_no_imm() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(5)));
    }

    #[test]
    fn update_pc_regular_instruction_has_imm() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: Some(bigint!(5)),
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(6)));
    }

    #[test]
    fn update_pc_jump_with_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(8)));
    }

    #[test]
    fn update_pc_jump_without_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: None,
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(
            Err(VirtualMachineError::UnconstrainedResJumpError),
            vm.update_pc(&instruction, &operands)
        );
    }

    #[test]
    fn update_pc_jump_rel_with_int_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP_REL,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(12)));
    }

    #[test]
    fn update_pc_jump_rel_without_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP_REL,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: None,
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(
            Err(VirtualMachineError::UnconstrainedResJumpRelError),
            vm.update_pc(&instruction, &operands)
        );
    }

    #[test]
    fn update_pc_jump_rel_with_non_int_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP_REL,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::RelocatableValue(Relocatable {
                segment_index: bigint!(1),
                offset: bigint!(4),
            })),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(
            Err(VirtualMachineError::PureValueError),
            vm.update_pc(&instruction, &operands)
        );
    }

    #[test]
    fn update_pc_jnz_res_is_zero() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JNZ,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(0))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(5)));
    }

    #[test]
    fn update_pc_jnz_res_is_not_zero() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JNZ,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_pc(&instruction, &operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(14)));
    }

    #[test]
    fn update_registers_all_regular() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::REGULAR,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_registers(instruction, operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(5)));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(5)));
        assert_eq!(vm.run_context.fp, MaybeRelocatable::Int(bigint!(6)));
    }

    #[test]
    fn update_registers_mixed_types() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP_REL,
            ap_update: ApUpdate::ADD2,
            fp_update: FpUpdate::DST,
            opcode: Opcode::NOP,
        };

        let operands = Operands {
            dst: MaybeRelocatable::Int(bigint!(11)),
            res: Some(MaybeRelocatable::Int(bigint!(8))),
            op0: MaybeRelocatable::Int(bigint!(9)),
            op1: MaybeRelocatable::Int(bigint!(10)),
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok(()), vm.update_registers(instruction, operands));
        assert_eq!(vm.run_context.pc, MaybeRelocatable::Int(bigint!(12)));
        assert_eq!(vm.run_context.ap, MaybeRelocatable::Int(bigint!(7)));
        assert_eq!(vm.run_context.fp, MaybeRelocatable::Int(bigint!(11)));
    }

    #[test]
    fn is_zero_int_value() {
        let value = MaybeRelocatable::Int(bigint!(1));
        assert_eq!(Ok(false), VirtualMachine::is_zero(Some(value)));
    }

    #[test]
    fn is_zero_relocatable_value() {
        let value = MaybeRelocatable::RelocatableValue(Relocatable {
            segment_index: bigint!(1),
            offset: bigint!(2),
        });
        assert_eq!(Ok(false), VirtualMachine::is_zero(Some(value)));
    }

    #[test]
    fn is_zero_relocatable_value_negative() {
        let value = MaybeRelocatable::RelocatableValue(Relocatable {
            segment_index: bigint!(1),
            offset: bigint!(-1),
        });
        assert_eq!(
            Err(VirtualMachineError::PureValueError),
            VirtualMachine::is_zero(Some(value))
        );
    }

    #[test]
    fn deduce_op0_opcode_call() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::CALL,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(
            Ok((Some(MaybeRelocatable::Int(bigint!(5))), None)),
            vm.deduce_op0(&instruction, None, None)
        );
    }

    #[test]
    fn deduce_op0_opcode_assert_eq_res_add_with_optionals() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(3));
        let op1 = MaybeRelocatable::Int(bigint!(2));
        assert_eq!(
            Ok((
                Some(MaybeRelocatable::Int(bigint!(1))),
                Some(MaybeRelocatable::Int(bigint!(3)))
            )),
            vm.deduce_op0(&instruction, Some(&dst), Some(&op1))
        );
    }

    #[test]
    fn deduce_op0_opcode_assert_eq_res_add_without_optionals() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        assert_eq!(Ok((None, None)), vm.deduce_op0(&instruction, None, None));
    }

    #[test]
    fn deduce_op0_opcode_assert_eq_res_mul_non_zero_op1() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op1 = MaybeRelocatable::Int(bigint!(2));
        assert_eq!(
            Ok((
                Some(MaybeRelocatable::Int(bigint!(2))),
                Some(MaybeRelocatable::Int(bigint!(4)))
            )),
            vm.deduce_op0(&instruction, Some(&dst), Some(&op1))
        );
    }

    #[test]
    fn deduce_op0_opcode_assert_eq_res_mul_zero_op1() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op1 = MaybeRelocatable::Int(bigint!(0));
        assert_eq!(
            Ok((None, None)),
            vm.deduce_op0(&instruction, Some(&dst), Some(&op1))
        );
    }

    #[test]
    fn deduce_op0_opcode_assert_eq_res_op1() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::OP1,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op1 = MaybeRelocatable::Int(bigint!(0));
        assert_eq!(
            Ok((None, None)),
            vm.deduce_op0(&instruction, Some(&dst), Some(&op1))
        );
    }

    #[test]
    fn deduce_op0_opcode_ret() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::RET,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op1 = MaybeRelocatable::Int(bigint!(0));
        assert_eq!(
            Ok((None, None)),
            vm.deduce_op0(&instruction, Some(&dst), Some(&op1))
        );
    }

    #[test]
    fn deduce_op1_opcode_call() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::CALL,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };

        assert_eq!(Ok((None, None)), vm.deduce_op1(&instruction, None, None));
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_add_with_optionals() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(3));
        let op0 = MaybeRelocatable::Int(bigint!(2));
        assert_eq!(
            Ok((
                Some(MaybeRelocatable::Int(bigint!(1))),
                Some(MaybeRelocatable::Int(bigint!(3)))
            )),
            vm.deduce_op1(&instruction, Some(&dst), Some(op0))
        );
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_add_without_optionals() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        assert_eq!(Ok((None, None)), vm.deduce_op1(&instruction, None, None));
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_mul_non_zero_op0() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op0 = MaybeRelocatable::Int(bigint!(2));
        assert_eq!(
            Ok((
                Some(MaybeRelocatable::Int(bigint!(2))),
                Some(MaybeRelocatable::Int(bigint!(4)))
            )),
            vm.deduce_op1(&instruction, Some(&dst), Some(op0))
        );
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_mul_zero_op0() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(4));
        let op0 = MaybeRelocatable::Int(bigint!(0));
        assert_eq!(
            Ok((None, None)),
            vm.deduce_op1(&instruction, Some(&dst), Some(op0))
        );
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_op1_without_dst() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::OP1,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op0 = MaybeRelocatable::Int(bigint!(0));
        assert_eq!(
            Ok((None, None)),
            vm.deduce_op1(&instruction, None, Some(op0))
        );
    }

    #[test]
    fn deduce_op1_opcode_assert_eq_res_op1_with_dst() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::OP1,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let dst = MaybeRelocatable::Int(bigint!(7));
        assert_eq!(
            Ok((
                Some(MaybeRelocatable::Int(bigint!(7))),
                Some(MaybeRelocatable::Int(bigint!(7)))
            )),
            vm.deduce_op1(&instruction, Some(&dst), None)
        );
    }

    #[test]
    fn compute_res_op1() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::OP1,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op1 = MaybeRelocatable::Int(bigint!(7));
        let op0 = MaybeRelocatable::Int(bigint!(9));
        assert_eq!(
            Ok((Some(MaybeRelocatable::Int(bigint!(7))))),
            vm.compute_res(&instruction, &op0, &op1)
        );
    }

    #[test]
    fn compute_res_add() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::ADD,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op1 = MaybeRelocatable::Int(bigint!(7));
        let op0 = MaybeRelocatable::Int(bigint!(9));
        assert_eq!(
            Ok((Some(MaybeRelocatable::Int(bigint!(16))))),
            vm.compute_res(&instruction, &op0, &op1)
        );
    }

    #[test]
    fn compute_res_mul_int_operands() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op1 = MaybeRelocatable::Int(bigint!(7));
        let op0 = MaybeRelocatable::Int(bigint!(9));
        assert_eq!(
            Ok((Some(MaybeRelocatable::Int(bigint!(63))))),
            vm.compute_res(&instruction, &op0, &op1)
        );
    }

    #[test]
    fn compute_res_mul_relocatable_values() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::MUL,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op1 = MaybeRelocatable::RelocatableValue(Relocatable {
            segment_index: bigint!(2),
            offset: bigint!(3),
        });
        let op0 = MaybeRelocatable::RelocatableValue(Relocatable {
            segment_index: bigint!(2),
            offset: bigint!(6),
        });
        assert_eq!(
            Err(VirtualMachineError::PureValueError),
            vm.compute_res(&instruction, &op0, &op1)
        );
    }

    #[test]
    fn compute_res_unconstrained() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::UNCONSTRAINED,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let op1 = MaybeRelocatable::Int(bigint!(7));
        let op0 = MaybeRelocatable::Int(bigint!(9));
        assert_eq!(Ok(None), vm.compute_res(&instruction, &op0, &op1));
    }

    #[test]
    fn deduce_dst_opcode_assert_eq_with_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::UNCONSTRAINED,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        let res = MaybeRelocatable::Int(bigint!(7));
        assert_eq!(Some(MaybeRelocatable::Int(bigint!(7))), vm.deduce_dst(&instruction, Some(&res)));
    }

    #[test]
    fn deduce_dst_opcode_assert_eq_without_res() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::UNCONSTRAINED,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::ASSERT_EQ,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        assert_eq!(None, vm.deduce_dst(&instruction, None));
    }

    #[test]
    fn deduce_dst_opcode_call() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::UNCONSTRAINED,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::CALL,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        assert_eq!(Some(MaybeRelocatable::Int(bigint!(6))), vm.deduce_dst(&instruction, None));
    }

    #[test]
    fn deduce_dst_opcode_ret() {
        let instruction = Instruction {
            off0: bigint!(1),
            off1: bigint!(2),
            off2: bigint!(3),
            imm: None,
            dst_register: Register::FP,
            op0_register: Register::AP,
            op1_addr: Op1Addr::AP,
            res: Res::UNCONSTRAINED,
            pc_update: PcUpdate::JUMP,
            ap_update: ApUpdate::REGULAR,
            fp_update: FpUpdate::REGULAR,
            opcode: Opcode::RET,
        };

        let mut run_context = RunContext {
            memory: Memory::new(),
            pc: MaybeRelocatable::Int(bigint!(4)),
            ap: MaybeRelocatable::Int(bigint!(5)),
            fp: MaybeRelocatable::Int(bigint!(6)),
            prime: bigint!(127),
        };

        let mut vm = VirtualMachine {
            run_context: run_context,
            prime: bigint!(127),
            program_base: None,
            validated_memory: ValidatedMemoryDict::new(),
            accessesed_addresses: Vec::<MaybeRelocatable>::new(),
            trace: Vec::<TraceEntry>::new(),
            current_step: bigint!(1),
            skip_instruction_execution: false,
        };
        assert_eq!(None, vm.deduce_dst(&instruction, None));
    }
}
